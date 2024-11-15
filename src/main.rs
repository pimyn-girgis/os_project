use core::panic;
use getopts::Options;
use libc::{self, pid_t, sysinfo};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::time::Duration;

struct ProcessInfo {
  pid: pid_t,
  ppid: pid_t,
  name: String,
  state: char,
  memory: u64, // VmRSS in KB
  exe_path: String,
  thread_count: u64,
  virtual_memory: u64, // VmSize in KB
  user_time: u64, // User CPU time
  system_time: u64, // System CPU time
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

fn read_process_info(pid: pid_t) -> io::Result<ProcessInfo> {
  // Read process status file
  let status_path = format!("/proc/{}/status", pid);
  let status_map = parse_status_file(&status_path)?;

  let exe_path = format!("/proc/{}/exe", pid);
  let exe_path_str = fs::read_link(exe_path).unwrap_or_else(|_| "N/A".into());

  let process_info = ProcessInfo {
      pid,
      ppid: status_map["PPid"][0].parse().unwrap_or_default(),
      name: status_map["Name"][0].clone(),
      state: status_map["State"][0].chars().next().unwrap_or_default(),
      memory: {
          if let Some(vm_rss) = status_map.get("VmRSS") {
              vm_rss[0].parse::<u64>().unwrap_or_default()
          } else {
              0
          }
      },
      exe_path: exe_path_str.to_string_lossy().to_string(),
      thread_count: status_map.get("Threads").and_then(|v| v[0].parse().ok()).unwrap_or(0),
      virtual_memory: status_map.get("VmSize").and_then(|v| v[0].parse().ok()).unwrap_or(0),
      user_time: status_map.get("Utime").and_then(|v| v[0].parse().ok()).unwrap_or(0),
      system_time: status_map.get("Stime").and_then(|v| v[0].parse().ok()).unwrap_or(0),
  };

  Ok(process_info)
}


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
  let mut cpu_usage = Vec::new();

  let stat_content = fs::read_to_string("/proc/stat")?;
  for line in stat_content.lines() {
    if line.starts_with("cpu") {
      let values: Vec<&str> = line.split_whitespace().collect();
      if values[0] == "cpu" {
        let total: u64 = values[1..].iter().take(7).map(|&s| s.parse::<u64>().unwrap_or(0)).sum();
        let idle: u64 = values[4].parse().unwrap_or(0);
        let usage = (total - idle) as f64 / total as f64 * 100.0;
        cpu_usage.push(usage);
      } else {
        let core_usage: u64 = values[1..].iter().take(7).map(|&s| s.parse::<u64>().unwrap_or(0)).sum();
        let core_idle: u64 = values[4].parse().unwrap_or(0);
        let core_percent = (core_usage - core_idle) as f64 / core_usage as f64 * 100.0;
        cpu_usage.push(core_percent);
      }
    }
  }

  Ok(cpu_usage)
}

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let mut opts = Options::new();
  opts.optopt("r", "refresh_rate", "How often to update the output", "NUM");
  let matches = match opts.parse(&args[1..]) {
    Ok(m) => m,
    Err(f) => {
      panic!("{}", f);
    }
  };

  let refresh_rate = match matches.opt_get_default::<u64>("r", 1) {
    Ok(r) => r,
    Err(f) => {
      panic!("{}", f);
    }
  };

  loop {
    unsafe {
      print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
      let mut system_info: sysinfo = std::mem::zeroed();
      sysinfo(&mut system_info as *mut sysinfo);
      let mem_unit = 1_000_000 / system_info.mem_unit as u64;
      println!("totalram: {}", system_info.totalram / mem_unit);
      println!("sharedram: {}", system_info.sharedram / mem_unit);
      println!("freeram: {}", system_info.freeram / mem_unit);
      println!("bufferram: {}", system_info.bufferram / mem_unit);
      println!("totalswap: {}", system_info.totalswap / mem_unit);
      println!("freeswap: {}", system_info.freeswap / mem_unit);
      println!("uptime: {}", system_info.uptime);
      println!("loads: {:?}", system_info.loads);
    };

    match get_cpu_usage() {
      Ok(cpu_usage) => {
          println!("CPU Usage:");
          for (i, usage) in cpu_usage.iter().enumerate() {
              if i == 0 {
                  println!("Total CPU: {:.2}%", usage); 
              } else {
                  println!("Core {}: {:.2}%", i, usage);
              }
          }
      }
      Err(e) => eprintln!("Error retrieving CPU usage: {}", e),
  }  


// Print the table header with aligned labels
println!("{:<6}\t{:<6}\t{:<6}\t{:<8}\t{:<8}\t{:<12}\t{:<10}\t{:<10}\t{}", 
         "PID", "PPID", "STATE", "MEM(KB)", "THREADS", "VIRT_MEM(KB)", "USER_TIME", "SYS_TIME", "EXE PATH");
println!("{}", "-".repeat(90)); // Adjusted width

match list_processes() {
    Ok(mut processes) => {
        processes.sort_by_key(|p| p.pid); // Sort processes by PID
        for process in processes {
            // Format the table rows and print them
            println!(
                "{:<6}\t{:<6}\t{:<6}\t{:<8}\t{:<8}\t{:<12}\t{:<10}\t{:<10}\t{}",
                process.pid,                            // PID
                process.ppid,                           // PPID
                process.state,                          // Process state
                process.memory,                         // Memory usage
                process.thread_count,                   // Thread count
                process.virtual_memory,                 // Virtual memory
                process.user_time,                      // User time
                process.system_time,                    // System time
                process.exe_path                       // Executable path
            );
        }
    }
    Err(e) => {
        eprintln!("Error listing processes: {}", e);
    }
}
  
  std::thread::sleep(Duration::from_secs(refresh_rate));
  }
}