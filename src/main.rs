use libc::{self, pid_t, sysinfo};
use std::collections::HashMap;
use std::fs;
use std::io;

struct ProcessInfo {
  pid: pid_t,
  ppid: pid_t,
  name: String,
  state: char,
  memory: u64,
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

  let info = ProcessInfo {
    pid,
    ppid: status_map["PPid"][0].parse().unwrap_or_default(),
    name: status_map["Name"][0].clone(),
    state: status_map["State"][0].chars().next().unwrap_or_default(),
    memory: {
      if let Some(vm_rss) = status_map.get("VmRSS") {
        vm_rss[0].parse::<u64>().unwrap()
      } else {
        0
      }
    },
  };

  Ok(info)
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

fn main() -> io::Result<()> {
  unsafe {
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

  println!("PID\tPPID\tSTATE\tMEM(KB)\tNAME");
  println!("{}", "-".repeat(50));
  match list_processes() {
    Ok(mut processes) => {
      processes.sort_by_key(|p| p.pid);
      for process in processes {
        println!(
          "{}\t{}\t{}\t{}\t{}",
          process.pid, process.ppid, process.state, process.memory, process.name
        );
      }
      Ok(())
    }
    Err(e) => {
      eprintln!("Error listing processes: {}", e);
      Err(e)
    }
  }
}
