use libc::{self, pid_t, sysinfo};
use std::fs;
use std::io;

struct ProcessInfo {
  pid: pid_t,
  ppid: pid_t,
  name: String,
  state: char,
  memory: u64,
}

fn read_process_info(pid: pid_t) -> io::Result<ProcessInfo> {
  // Read process status file
  let status_path = format!("/proc/{}/status", pid);
  let status_content = fs::read_to_string(status_path)?;

  // Initialize default values
  let mut name = String::new();
  let mut ppid = 0;
  let mut state = '?';
  let mut memory = 0;

  // Parse status file
  // Place holder code, should be a general function
  for line in status_content.lines() {
    if line.starts_with("Name:") {
      name = line.split_whitespace().nth(1).unwrap_or("").to_string();
    } else if line.starts_with("PPid:") {
      ppid = line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    } else if line.starts_with("State:") {
      state = line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.chars().next())
        .unwrap_or('?');
    } else if line.starts_with("VmRSS:") {
      memory = line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    }
  }

  Ok(ProcessInfo {
    pid,
    ppid,
    name,
    state,
    memory,
  })
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
  println!("PID\tPPID\tSTATE\tMEM(KB)\tNAME");
  println!("{}", "-".repeat(50));

  unsafe {
    let mut system_info: sysinfo = std::mem::zeroed();
    sysinfo(&mut system_info as *mut sysinfo);
    let mem_unit = 1_000_000 / system_info.mem_unit as u64;
    println!("totalram: {}", system_info.totalram/mem_unit);
    println!("sharedram: {}", system_info.sharedram/mem_unit);
    println!("freeram: {}", system_info.freeram/mem_unit);
    println!("bufferram: {}", system_info.bufferram/mem_unit);
    println!("totalswap: {}", system_info.totalswap/mem_unit);
    println!("freeswap: {}", system_info.freeswap/mem_unit);
    println!("uptime: {}", system_info.uptime);
    println!("loads: {:?}", system_info.loads);
  };

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
