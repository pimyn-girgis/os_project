use core::panic;
use getopts::Options;
use libc::{self, cpu_set_t, pid_t, sched_setaffinity, sysinfo, CPU_SET, CPU_ZERO};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};

#[derive(Clone)]
pub struct ProcessInfo {
  pub user: String,
  pub pid: pid_t,
  pub ppid: pid_t,
  pub name: String,
  pub state: char,
  pub memory: u64,
  pub thread_count: u64,
  pub virtual_memory: u64,
  pub user_time: u64,
  pub system_time: u64,
  pub priority: i32,
}

fn get_username_from_uid(target_uid: u32) -> Option<String> {
  // cache the usernames to avoid opening the file multiple times
  static mut CACHE: BTreeMap<u32, String> = BTreeMap::new();
  if let Some(username) = unsafe { CACHE.get(&target_uid) } {
    return Some(username.clone());
  }
  let file = File::open("/etc/passwd").ok()?;
  let reader = BufReader::new(file);

  for line in reader.lines().map_while(Result::ok) {
    let fields: Vec<&str> = line.split(':').collect();
    if fields.len() >= 3 {
      if let Ok(uid) = fields[2].parse::<u32>() {
        if uid == target_uid {
          unsafe {
            CACHE.insert(uid, fields[0].to_string());
          }
          return Some(fields[0].to_string());
        }
      }
    }
  }

  None
}

pub fn parse_status_line(line: &str) -> io::Result<(String, Vec<String>)> {
  let line_parts: Vec<&str> = line.split(':').collect();
  match line_parts.len() {
    2 => {
      let key = line_parts[0].trim().to_string();
      let values: Vec<String> = line_parts[1].split_whitespace().map(|s| s.to_string()).collect();
      Ok((key, values))
    }
    _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid status line")),
  }
}

// Function to bind a process to a set of CPUs
pub fn bind_to_cpu_set(pid: pid_t, cpu_ids: &Vec<usize>) -> io::Result<()> {
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
  let result = unsafe { sched_setaffinity(pid, std::mem::size_of::<cpu_set_t>(), &cpuset as *const _) };

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
pub fn read_process_info(pid: pid_t) -> io::Result<ProcessInfo> {
  pub fn parse_status_file(status_path: &str) -> io::Result<HashMap<String, Vec<String>>> {
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
    user: get_username_from_uid(status_map["Uid"][0].parse().unwrap_or_default()).unwrap_or_default(),
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
    thread_count: status_map.get("Threads").and_then(|v| v[0].parse().ok()).unwrap_or(0),
    virtual_memory: status_map.get("VmSize").and_then(|v| v[0].parse().ok()).unwrap_or(0),
    user_time: status_map.get("Utime").and_then(|v| v[0].parse().ok()).unwrap_or(0),
    system_time: status_map.get("Stime").and_then(|v| v[0].parse().ok()).unwrap_or(0),
    priority: get_priority(pid),
  };

  Ok(process_info)
}

pub fn filter_processes(
  processes: Vec<ProcessInfo>,
  filter_by: &str,
  pattern: &str,
  exact_match: bool,
) -> Vec<ProcessInfo> {
  processes
    .into_iter()
    .filter(|p| {
      let field = match filter_by {
        "name" => p.name.clone(),
        "user" => p.user.clone(),
        "ppid" => p.ppid.to_string(),
        "state" => p.state.to_string(),
        _ => panic!("Invalid filter_by value"),
      };
      if exact_match {
        field == pattern
      } else {
        field.contains(pattern)
      }
    })
    .collect()
}

pub fn read_processes() -> io::Result<Vec<ProcessInfo>> {
  let mut processes = Vec::new();
  for entry in fs::read_dir("/proc")? {
    let path = entry?.path();
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

// List processes from /proc
pub fn list_processes(
  mut processes: Vec<ProcessInfo>,
  mut from: usize,
  mut nprocs: usize,
  sort_by: &str,
  ascending: bool,
  filter_by: &str,
  pattern: &str,
  exact_match: bool,
) -> io::Result<Vec<ProcessInfo>> {
  match sort_by {
    "name" => processes.sort_by_key(|p| p.name.clone()),
    "pid" => processes.sort_by_key(|p| p.pid),
    "memory" => processes.sort_by_key(|p| p.memory),
    "priority" => processes.sort_by_key(|p| p.priority),
    "user" => processes.sort_by_key(|p| p.user.clone()),
    "state" => processes.sort_by_key(|p| p.state),
    "threads" => processes.sort_by_key(|p| p.thread_count),
    "vmsize" => processes.sort_by_key(|p| p.virtual_memory),
    "utime" => processes.sort_by_key(|p| p.user_time),
    "stime" => processes.sort_by_key(|p| p.system_time),
    _ => panic!("Invalid sort_by value"),
  }

  if !filter_by.is_empty() {
    processes = filter_processes(processes, filter_by, pattern, exact_match);
  }

  if nprocs > processes.len() {
    nprocs = processes.len();
  }

  if ascending {
    if from + nprocs > processes.len() {
      from = processes.len() - nprocs;
    }
    Ok(processes[from..from + nprocs].to_vec())
  } else {
    let len = processes.len();
    if from + nprocs > len {
      from = len - nprocs;
    }
    Ok(
      processes[len - from - nprocs..len - from]
        .iter()
        .cloned()
        .rev()
        .collect(),
    )
  }
}

pub struct Tree {
  children: Vec<Tree>,
  pid: pid_t,
}

impl Tree {
  pub fn print(&self, indent: usize) {
    if indent == 0 {
      println!("{}", self.pid);
    } else {
      let prefix = "│   ".repeat(indent / 4 - 1);
      println!("{}├── {}", prefix, self.pid);
    }
    for child in &self.children {
      child.print(indent + 4);
    }
  }
}

pub fn build_tree(processes: &Vec<ProcessInfo>, pid: pid_t) -> Tree {
  let children: Vec<Tree> = processes
    .iter()
    .filter(|p| p.ppid == pid)
    .map(|p| build_tree(processes, p.pid))
    .collect();
  Tree { children, pid }
}

pub fn get_cpu_usage() -> io::Result<Vec<f64>> {
  pub fn parse_cpu_stats(content: &str) -> Vec<(u64, u64)> {
    let mut stats = Vec::new();
    for line in content.lines() {
      if line.starts_with("cpu") {
        let values: Vec<&str> = line.split_whitespace().collect();
        let total: u64 = values[1..].iter().take(7).map(|&s| s.parse::<u64>().unwrap_or(0)).sum();
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


pub struct SystemStats {
  pub memory_info: String,
  pub cpu_usage: Vec<String>,
  pub processes: Vec<ProcessInfo>,
}

pub fn show_stats(
  nprocs: usize,
  sort_by: &str,
  descending: bool,
  filter_by: &str,
  pattern: &str,
  exact_match: bool,
) -> SystemStats {
  let mut memory_info = String::new();
  let mut cpu_usage = Vec::new();
  let mut processes = Vec::new();

  // Collect system memory stats
  let system_info = get_sysinfo();
  let mem_unit = 1_000_000 / system_info.mem_unit as u64;

  memory_info.push_str(&format!(
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

  // Collect CPU usage stats
  match get_cpu_usage() {
      Ok(cpu_usages) => {
          cpu_usage.push("CPU Usage:".to_string());
          for (i, usage) in cpu_usages.iter().enumerate() {
              if i == 0 {
                  cpu_usage.push(format!("Total CPU: {:.2}%", usage));
              } else {
                  cpu_usage.push(format!("Core {}: {:.2}%", i, usage));
              }
          }
      }
      Err(e) => cpu_usage.push(format!("Error retrieving CPU usage: {}", e)),
  }

  // Collect process details
  match list_processes(
    read_processes().unwrap(),
    0,
    nprocs,
    sort_by,
    !descending,
    filter_by,
    pattern,
    exact_match,
  ) {
    Ok(proc_list) => {
        for process in proc_list {
            // Instead of formatting into a string, push the ProcessInfo directly
            processes.push(process); 
        }
    }
    Err(e) => {
        // You can either return the error directly or add an error message in processes
        eprintln!("Error listing processes: {}", e);
    }
  }


  // Return structured output
  SystemStats {
      memory_info,
      cpu_usage,
      processes,
  }
}


pub fn kill_process(pid: pid_t, signal: i32) {
  println!("Killing process {} with signal {}", pid, signal);
  unsafe {
    libc::kill(pid, signal);
  }
}

pub fn print_usage(program: &str, opts: Options) {
  let brief = format!("Usage: {} [options]", program);
  print!("{}", opts.usage(&brief));
}

pub fn set_priority(pid: pid_t, priority: i32) {
  unsafe {
    if libc::setpriority(libc::PRIO_PROCESS, pid.try_into().unwrap(), priority) == -1 {
      eprintln!("Failed to set priority: {}", std::io::Error::last_os_error());
    }
  }
}

pub fn get_priority(pid: pid_t) -> i32 {
  unsafe { libc::getpriority(libc::PRIO_PROCESS, pid.try_into().unwrap()) }
}

pub fn get_sysinfo() -> sysinfo {
  let mut system_info: sysinfo = unsafe { std::mem::zeroed() };
  unsafe {
    sysinfo(&mut system_info as *mut sysinfo);
  }
  system_info
}

pub fn execute_on_with_arg<T: std::marker::Copy>(pids: Vec<pid_t>, arg: T, fn_ptr: fn(pid_t, T)) {
  for pid in pids {
    fn_ptr(pid, arg);
  }
}

pub fn execute_on_with_args<T: std::marker::Copy>(
  pids: Vec<pid_t>,
  args: &Vec<T>,
  fn_ptr: fn(pid_t, &Vec<T>) -> io::Result<()>,
) {
  for pid in pids {
    let _ = fn_ptr(pid, args);
  }
}

pub fn execute_on(pids: Vec<pid_t>, fn_ptr: fn(pid_t)) {
  for pid in pids {
    fn_ptr(pid);
  }
}
