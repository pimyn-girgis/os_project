use core::panic;
use getopts::Options;
use libc::{self, cpu_set_t, pid_t, sched_setaffinity, sysinfo, CPU_SET, CPU_ZERO};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::Write;
use std::time::Duration;

struct ProcessInfo {
  user: String,
  pid: pid_t,
  ppid: pid_t,
  name: String,
  state: char,
  memory: u64,
  thread_count: u64,
  virtual_memory: u64,
  user_time: u64,
  system_time: u64,
  priority: i32,
}

fn parse_status_line(line: &str) -> io::Result<(String, Vec<String>)> {
  let line_parts: Vec<&str> = line.split(':').collect();
  match line_parts.len() {
    2 => {
      let key = line_parts[0].trim().to_string();
      let values: Vec<String> = line_parts[1]
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
      Ok((key, values))
    }
    _ => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "Invalid status line",
    )),
  }
}

// Function to bind a process to a set of CPUs
fn bind_to_cpu_set(pid: pid_t, cpu_ids: &Vec<usize>) -> io::Result<()> {
    // Initialize an empty CPU set
    let mut cpuset: cpu_set_t = unsafe { std::mem::zeroed() };

    unsafe {
        // Clear all CPUs from the set
        CPU_ZERO(&mut cpuset);
        // Add each CPU ID to the set
        for &cpu_id in cpu_ids {
            CPU_SET(cpu_id, &mut cpuset);
        }
    }

    // Set the CPU affinity for the given process
    let result =
        unsafe { sched_setaffinity(pid, std::mem::size_of::<cpu_set_t>(), &cpuset as *const _) };

    if result == 0 {
        println!("Process {} bound to CPUs {:?}", pid, cpu_ids);
        Ok(())
    } else {
        eprintln!(
            "Failed to set CPU affinity for process {}. Error: {:?}",
            pid,
            io::Error::last_os_error()
        );
        Err(io::Error::last_os_error())
    }
}

// Read process info from /proc/<pid>/status and /proc/<pid>/stat
fn read_process_info(pid: pid_t) -> io::Result<ProcessInfo> {
  fn parse_status_file(status_path: &str) -> io::Result<HashMap<String, Vec<String>>> {
    let status_content = fs::read_to_string(status_path)?;
    let mut status_map = HashMap::new();
    for line in status_content.lines() {
      if line.is_empty() {
        continue;
      }
      let (key, values) = parse_status_line(line)?;
      if !key.is_empty() {
        status_map.insert(key, values);
      }
    }
    Ok(status_map)
  }
  // Read process status file
  let status_path = format!("/proc/{}/status", pid);
  let status_map = parse_status_file(&status_path)?;

  let process_info = ProcessInfo {
    user: status_map["Uid"][0].clone(),
    pid,
    ppid: status_map["PPid"][0].parse().unwrap_or_default(),
    state: status_map["State"][0].chars().next().unwrap_or_default(),
    memory: {
      if let Some(vm_rss) = status_map.get("VmRSS") {
        vm_rss[0].parse::<u64>().unwrap_or_default()
      } else {
        0
      }
    },
    name: status_map["Name"][0].clone(),
    thread_count: status_map
      .get("Threads")
      .and_then(|v| v[0].parse().ok())
      .unwrap_or(0),
    virtual_memory: status_map
      .get("VmSize")
      .and_then(|v| v[0].parse().ok())
      .unwrap_or(0),
    user_time: status_map
      .get("Utime")
      .and_then(|v| v[0].parse().ok())
      .unwrap_or(0),
    system_time: status_map
      .get("Stime")
      .and_then(|v| v[0].parse().ok())
      .unwrap_or(0),
    priority: get_priority(pid),
  };

  Ok(process_info)
}

// List processes from /proc
fn list_processes() -> io::Result<Vec<ProcessInfo>> {
  let mut processes = Vec::new();

  for entry in fs::read_dir("/proc")? {
    let entry = entry?;
    let path = entry.path();
    if let Some(name) = path.file_name() {
      if let Some(name_str) = name.to_str() {
        if let Ok(pid) = name_str.parse::<pid_t>() {
          match read_process_info(pid) {
            Ok(info) => processes.push(info),
            Err(_) => continue, // Skip processes we can't read
          }
        }
      }
    }
  }

  Ok(processes)
}

fn get_cpu_usage() -> io::Result<Vec<f64>> {
  fn parse_cpu_stats(content: &str) -> Vec<(u64, u64)> {
    let mut stats = Vec::new();
    for line in content.lines() {
      if line.starts_with("cpu") {
        let values: Vec<&str> = line.split_whitespace().collect();
        let total: u64 = values[1..]
          .iter()
          .take(7)
          .map(|&s| s.parse::<u64>().unwrap_or(0))
          .sum();
        let idle: u64 = values[4].parse().unwrap_or(0);
        stats.push((total, idle));
      }
    }
    stats
  }

  let stat_content = fs::read_to_string("/proc/stat")?;
  let stats = parse_cpu_stats(&stat_content);
  static mut PREV_STATS: Vec<(u64, u64)> = Vec::new();
  let mut cpu_usage = Vec::new();

  unsafe {
    for (stat1, stat2) in PREV_STATS.iter().zip(stats.iter()) {
      let (total1, idle1) = stat1;
      let (total2, idle2) = stat2;

      let total_diff = total2 - total1;
      let idle_diff = idle2 - idle1;

      let usage = if total_diff > 0 {
        (total_diff - idle_diff) as f64 / total_diff as f64 * 100.0
      } else {
        0.0
      };

      cpu_usage.push(usage);
    }

    PREV_STATS = stats;
  }

  Ok(cpu_usage)
}

fn show_stats(
  refresh_rate: u64,
  nprocs: usize,
  iterations: u32,
  sort_by: String,
  log_file: String,
) {
  let mut log_file = fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(log_file)
    .expect("Failed to open log file");

  let mut current_iteration = 0;
  while iterations == 0 || current_iteration != iterations {
    // Use buffering to accumulate and write output in one go
    let mut output = String::new();

    unsafe {
      let mut system_info: sysinfo = std::mem::zeroed();
      sysinfo(&mut system_info as *mut sysinfo);
      let mem_unit = 1_000_000 / system_info.mem_unit as u64;

      output.push_str(&format!(
                "totalram: {}\nsharedram: {}\nfreeram: {}\nbufferram: {}\ntotalswap: {}\nfreeswap: {}\nuptime: {}\nloads: {:?}\n",
                system_info.totalram / mem_unit,
                system_info.sharedram / mem_unit,
                system_info.freeram / mem_unit,
                system_info.bufferram / mem_unit,
                system_info.totalswap / mem_unit,
                system_info.freeswap / mem_unit,
                system_info.uptime,
                system_info.loads
            ));
    }

    match get_cpu_usage() {
      Ok(cpu_usage) => {
        output.push_str("CPU Usage:\n");
        for (i, usage) in cpu_usage.iter().enumerate() {
          if i == 0 {
            output.push_str(&format!("Total CPU: {:.2}%\n", usage));
          } else {
            output.push_str(&format!("Core {}: {:.2}%\n", i, usage));
          }
        }
      }
      Err(e) => output.push_str(&format!("Error retrieving CPU usage: {}\n", e)),
    }

    macro_rules! FORMAT_STR { () => { "{:<6}\t{:<6}\t{:<6}\t{:<6}\t{:<8}\t{:<8}\t{:<12}\t{:<10}\t{:<10}\t{:<8}\t{:<20}\n" }; }

    output.push_str(&format!(
      FORMAT_STR!(),
      "UID",
      "PID",
      "PPID",
      "STATE",
      "MEM(KB)",
      "THREADS",
      "VIRT_MEM(KB)",
      "USER_TIME",
      "SYS_TIME",
      "Priority",
      "Name",
    ));
    output.push_str(&format!("{}\n", "-".repeat(150)));

    match list_processes() {
      Ok(mut processes) => {
        match sort_by.as_str() {
          "name" => processes.sort_by_key(|p| p.name.clone()),
          "pid" => processes.sort_by_key(|p| p.pid),
          "memory" => processes.sort_by_key(|p| p.memory),
          "priority" => processes.sort_by_key(|p| p.priority),
          _ => panic!("Invalid sort_by value"),
        }
        for (i, process) in processes.iter().enumerate() {
          if i >= nprocs {
            break;
          }
          output.push_str(&format!(
            FORMAT_STR!(),
            process.user,
            process.pid,
            process.ppid,
            process.state,
            process.memory,
            process.thread_count,
            process.virtual_memory,
            process.user_time,
            process.system_time,
            process.priority,
            process.name,
          ));
        }
      }
      Err(e) => {
        output.push_str(&format!("Error listing processes: {}\n", e));
      }
    }

    // Clear screen and display all at once
    print!("{esc}[2J{esc}[1;1H{}", output, esc = 27 as char);
    std::io::stdout().flush().unwrap();

    std::thread::sleep(Duration::from_secs(refresh_rate));
    current_iteration += 1;
    writeln!(log_file, "Iteration: {}\n{}", current_iteration, output)
      .expect("Failed to write to log file");
  }
}

fn kill_process(pid: pid_t, signal: i32) {
  println!("Killing process {} with signal {}", pid, signal);
  unsafe {
    libc::kill(pid, signal);
  }
}

fn print_usage(program: &str, opts: Options) {
  let brief = format!("Usage: {} [options]", program);
  print!("{}", opts.usage(&brief));
}

fn set_priority(pid: pid_t, priority: i32) {
  unsafe {
    if libc::setpriority(libc::PRIO_PROCESS, pid.try_into().unwrap(), priority) == -1 {
      eprintln!(
        "Failed to set priority: {}",
        std::io::Error::last_os_error()
      );
    }
  }
}

fn get_priority(pid: pid_t) -> i32 {
  unsafe { libc::getpriority(libc::PRIO_PROCESS, pid.try_into().unwrap()) }
}

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let mut opts = Options::new();
  opts.optopt("r", "refresh_rate", "Stats refresh rate", "[NUM]");
  opts.optopt("n", "nprocs", "Max number of processes to show", "[NUM]");
  opts.optopt("k", "kill", "Kill process with signal", "[SIG]");
  opts.optopt("i", "iters", "Number of iterations", "[NUM]");
  opts.optflag("h", "help", "Print help message");
  opts.optopt("p", "priority", "Priority of process", "[PRIO]");
  opts.optopt("o", "output", "output file (logs)", "[FILE]");
  opts.optopt("", "pid", "pid of process", "[PID]");
  opts.optopt(
    "s",
    "sort_by",
    "How to sort the processes",
    "[name|pid|memory|priority]",
  );
  opts.optopt("c", "cpu_affinity", "pid of process", "[PID,NUM]");

  let matches = match opts.parse(&args[1..]) {
    Ok(m) => m,
    Err(f) => {
      panic!("{}", f);
    }
  };

  if matches.opt_present("h") {
    print_usage(&args[0], opts);
    return;
  }

  let pid = matches
    .opt_get_default::<pid_t>("pid", 0)
    .expect("Invalid pid value");
  if matches.opt_present("pid") {
    if matches.opt_present("k") {
      let kill_signal = matches
        .opt_get_default::<i32>("k", libc::SIGKILL)
        .expect("Invalid signal value");
      kill_process(pid, kill_signal);
    }
    else if matches.opt_present("p") {
      let priority = matches
        .opt_get_default::<i32>("p", 0)
        .expect("Invalid priority value");
      set_priority(pid, priority)
    }
    else if matches.opt_present("c") {
      let cpu_list: Vec<usize> = args[2..].iter()
      .map(|arg| arg.parse::<usize>().expect("Invalid CPU value"))
      .collect();
      let _ = bind_to_cpu_set(pid, &cpu_list);
  }
    return;
  }

  let refresh_rate = matches
    .opt_get_default::<u64>("r", 1)
    .expect("Invalid refresh rate value");

  let nprocs = matches
    .opt_get_default::<usize>("n", usize::MAX)
    .expect("Invalid nprocs value");

  let iterations = matches
    .opt_get_default::<u32>("i", 0)
    .expect("Invalid iterations value");

  let sort_by = matches
    .opt_get_default::<String>("s", "pid".to_string())
    .expect("Invalid sort_by value");

  let output_file = matches
    .opt_get_default::<String>("o", "/tmp/procstat.log".to_string())
    .expect("Invalid output file value");
  show_stats(refresh_rate, nprocs, iterations, sort_by, output_file);
}
