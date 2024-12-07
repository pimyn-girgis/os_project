// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// Don't forget to put this function after demo: get_sysinfo, print_usage
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![get_disk_stats, get_network_stats,greet, get_sysinfo_serialized, get_username_from_uid, show_stats, get_priority, set_priority, kill_process, build_tree, get_cpu_usage, parse_status_line, bind_to_cpu_set, read_process_info, filter_processes, read_processes, list_processes])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use core::panic;
use getopts::Options;
use libc::{self, cpu_set_t, pid_t, sched_setaffinity, sysinfo, CPU_SET, CPU_ZERO};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};

use serde::{Serialize, Deserialize};
use tauri::ipc::IpcResponse;

#[derive(Serialize)] // Deriving Serialize to make the struct serializable
struct SystemInfo {
    total_ram: u64,
    shared_ram: u64,
    free_ram: u64,
    buffer_ram: u64,
    total_swap: u64,
    free_swap: u64,
    uptime: i64,
    load_averages: [u64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[tauri::command]
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
#[tauri::command]
fn parse_status_line(line: &str) -> Result<(String, Vec<String>), String> {
    let line_parts: Vec<&str> = line.split(':').collect();
    match line_parts.len() {
        2 => {
            let key = line_parts[0].trim().to_string();
            let values: Vec<String> = line_parts[1].split_whitespace().map(|s| s.to_string()).collect();
            Ok((key, values))
        }
        _ => Err("Invalid status line".to_string()),
    }
}


// Function to bind a process to a set of CPUs
#[tauri::command] 
fn bind_to_cpu_set(pid: pid_t, cpu_ids: Vec<usize>) -> Result<(), String> {
    // Existing bind_to_cpu_set implementation
    // Convert io::Error to String for serialization
    let mut cpuset: libc::cpu_set_t = unsafe { std::mem::zeroed() };

    unsafe {
        libc::CPU_ZERO(&mut cpuset);
        for &cpu_id in &cpu_ids {
            libc::CPU_SET(cpu_id, &mut cpuset);
        }
    }

    let result = unsafe { 
        libc::sched_setaffinity(pid, std::mem::size_of::<libc::cpu_set_t>(), &cpuset as *const _) 
    };

    if result == 0 {
        Ok(())
    } else {
        Err(format!("Failed to set CPU affinity for process {}", pid))
    }
}

// Read process info from /proc/<pid>/status and /proc/<pid>/stat
#[tauri::command]
fn read_process_info(pid: pid_t) -> Result<ProcessInfo, String> {
    // Existing read_process_info implementation
    // Convert io::Error to String for serialization
    let status_path = format!("/proc/{}/status", pid);
    let status_content = std::fs::read_to_string(&status_path)
        .map_err(|e| format!("Failed to read status file: {}", e))?;

    let mut status_map = std::collections::HashMap::new();
    for line in status_content.lines() {
        if !line.is_empty() {
            let (key, values) = parse_status_line(line)
                .map_err(|e| format!("Failed to parse status line: {}", e))?;
            status_map.insert(key, values);
        }
    }

    Ok(ProcessInfo {
        user: get_username_from_uid(status_map["Uid"][0].parse().unwrap_or_default())
            .unwrap_or_default(),
        pid,
        ppid: status_map["PPid"][0].parse().unwrap_or_default(),
        state: status_map["State"][0].chars().next().unwrap_or_default(),
        memory: status_map.get("VmRSS")
            .and_then(|vm_rss| vm_rss[0].parse::<u64>().ok())
            .unwrap_or_default(),
        name: status_map["Name"][0].clone(),
        thread_count: status_map.get("Threads")
            .and_then(|v| v[0].parse().ok())
            .unwrap_or(0),
        virtual_memory: status_map.get("VmSize")
            .and_then(|v| v[0].parse().ok())
            .unwrap_or(0),
        user_time: status_map.get("Utime")
            .and_then(|v| v[0].parse().ok())
            .unwrap_or(0),
        system_time: status_map.get("Stime")
            .and_then(|v| v[0].parse().ok())
            .unwrap_or(0),
        priority: get_priority(pid),
    })
}

#[tauri::command]
fn filter_processes(
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

#[tauri::command]
fn read_processes() -> Result<Vec<ProcessInfo>, String> {
    let mut processes = Vec::new();
    for entry in std::fs::read_dir("/proc")
        .map_err(|e| format!("Failed to read /proc directory: {}", e))? {
        let path = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?.path();
        if let Some(name) = path.file_name() {
            if let Some(name_str) = name.to_str() {
                if let Ok(pid) = name_str.parse::<pid_t>() {
                    match read_process_info(pid) {
                        Ok(info) => processes.push(info),
                        Err(_) => continue,
                    }
                }
            }
        }
    }
    Ok(processes)
}

// List processes from /proc
#[tauri::command]
fn list_processes(
    mut processes: Vec<ProcessInfo>,
    mut from: usize,
    mut nprocs: usize,
    sort_by: &str,
    ascending: bool,
    filter_by: &str,
    pattern: &str,
    exact_match: bool,
) -> Result<Vec<ProcessInfo>, String> {
    // Existing list_processes implementation with error handling
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
        _ => return Err("Invalid sort_by value".to_string()),
    }

    let filtered_processes = if !filter_by.is_empty() {
        processes
            .into_iter()
            .filter(|p| {
                let field = match filter_by {
                    "name" => p.name.clone(),
                    "user" => p.user.clone(),
                    "ppid" => p.ppid.to_string(),
                    "state" => p.state.to_string(),
                    _ => return false,
                };
                if exact_match {
                    field == pattern
                } else {
                    field.contains(pattern)
                }
            })
            .collect()
    } else {
        processes
    };

    if nprocs > filtered_processes.len() {
        nprocs = filtered_processes.len();
    }

    if ascending {
        if from + nprocs > filtered_processes.len() {
            from = filtered_processes.len() - nprocs;
        }
        Ok(filtered_processes[from..from + nprocs].to_vec())
    } else {
        let len = filtered_processes.len();
        if from + nprocs > len {
            from = len - nprocs;
        }
        Ok(
            filtered_processes[len - from - nprocs..len - from]
                .iter()
                .cloned()
                .rev()
                .collect(),
        )
    }
}

#[derive(Serialize, Deserialize)]
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

#[tauri::command]
fn build_tree(processes: Vec<ProcessInfo>, pid: pid_t) -> Tree {
    let children: Vec<Tree> = processes
        .iter()
        .filter(|p| p.ppid == pid)
        .map(|p| build_tree(processes.clone(), p.pid))
        .collect();
    Tree { children, pid }
}

#[tauri::command]
fn get_cpu_usage() -> Result<Vec<f64>, String> {
    // Existing get_cpu_usage implementation with error handling
    let stat_content = std::fs::read_to_string("/proc/stat")
        .map_err(|e| format!("Failed to read /proc/stat: {}", e))?;
    
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

fn parse_cpu_stats(content: &str) -> Vec<(u64, u64)> {
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

#[tauri::command]
fn show_stats(
  nprocs: usize,
  sort_by: &str,
  descending: bool,
  filter_by: &str,
  pattern: &str,
  exact_match: bool,
) -> String {
  // Use buffering to accumulate and write output in one go
  let mut output = String::new();

  let system_info = get_sysinfo();
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

  macro_rules! FORMAT_STR {
    () => {
      "{:<6}\t{:<6}\t{:<6}\t{:<6}\t{:<8}\t{:<8}\t{:<12}\t{:<10}\t{:<10}\t{:<8}\t{:<20}\n"
    };
  }

  output.push_str(&format!(
    FORMAT_STR!(),
    "UID", "PID", "PPID", "STATE", "MEM(MB)", "THREADS", "VIRT_MEM(MB)", "USER_TIME", "SYS_TIME", "Priority", "Name",
  ));
  output.push_str(&format!("{}\n", "-".repeat(150)));

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
    Ok(processes) => {
      for process in processes {
        output.push_str(&format!(
          FORMAT_STR!(),
          process.user,
          process.pid,
          process.ppid,
          process.state,
          process.memory / 1000,
          process.thread_count,
          process.virtual_memory / 1000,
          process.user_time,
          process.system_time,
          process.priority,
          process.name
        ));
      }
    }
    Err(e) => {
      output.push_str(&format!("Error listing processes: {}\n", e));
    }
  }
  output
}

#[tauri::command]
fn kill_process(pid: pid_t, signal: i32) {
  println!("Killing process {} with signal {}", pid, signal);
  unsafe {
    libc::kill(pid, signal);
  }
}

#[tauri::command]
fn print_usage(program: &str, opts: Options) {
  let brief = format!("Usage: {} [options]", program);
  print!("{}", opts.usage(&brief));
}

#[tauri::command]
fn set_priority(pid: pid_t, priority: i32) {
  unsafe {
    if libc::setpriority(libc::PRIO_PROCESS, pid.try_into().unwrap(), priority) == -1 {
      eprintln!("Failed to set priority: {}", std::io::Error::last_os_error());
    }
  }
}

#[tauri::command]
fn get_priority(pid: pid_t) -> i32 {
  unsafe { libc::getpriority(libc::PRIO_PROCESS, pid.try_into().unwrap()) }
}

#[tauri::command]
fn get_sysinfo_serialized() -> SystemInfo {
  let mut system_info = get_sysinfo();
  // Extract the necessary information from sysinfo
  let total_ram = system_info.totalram;
  let shared_ram = system_info.sharedram;
  let free_ram = system_info.freeram;
  let buffer_ram = system_info.bufferram;
  let total_swap = system_info.totalswap;
  let free_swap = system_info.freeswap;
  let uptime = system_info.uptime;
  let load_averages = system_info.loads;

  // Return the system info as a serializable struct
  SystemInfo {
      total_ram,
      shared_ram,
      free_ram,
      buffer_ram,
      total_swap,
      free_swap,
      uptime,
      load_averages,
  }
}

#[tauri::command]
fn get_sysinfo() -> sysinfo {
  let mut system_info: sysinfo = unsafe { std::mem::zeroed() };
  unsafe {
    sysinfo(&mut system_info as *mut sysinfo);
  }
  system_info
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskStats {
    pub device: String,
    pub reads_completed: u64,
    pub reads_merged: u64,
    pub sectors_read: u64,
    pub time_reading: u64,
    pub writes_completed: u64,
    pub writes_merged: u64,
    pub sectors_written: u64,
    pub time_writing: u64,
    pub io_in_progress: u64,
    pub time_io: u64,
    pub weighted_time_io: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkStats {
    pub interface: String,
    pub bytes_received: u64,
    pub packets_received: u64,
    pub errors_received: u64,
    pub drops_received: u64,
    pub bytes_transmitted: u64,
    pub packets_transmitted: u64,
    pub errors_transmitted: u64,
    pub drops_transmitted: u64,
}

#[tauri::command]
fn get_disk_stats() -> Result<Vec<DiskStats>, String> {
    let content = fs::read_to_string("/proc/diskstats").map_err(|e| e.to_string())?;
    let mut stats = Vec::new();

    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 14 {
            continue;
        }

        if !fields[2].starts_with("sd") && !fields[2].starts_with("hd") && !fields[2].starts_with("nvme") {
            continue;
        }

        stats.push(DiskStats {
            device: fields[2].to_string(),
            reads_completed: fields[3].parse().unwrap_or(0),
            reads_merged: fields[4].parse().unwrap_or(0),
            sectors_read: fields[5].parse().unwrap_or(0),
            time_reading: fields[6].parse().unwrap_or(0),
            writes_completed: fields[7].parse().unwrap_or(0),
            writes_merged: fields[8].parse().unwrap_or(0),
            sectors_written: fields[9].parse().unwrap_or(0),
            time_writing: fields[10].parse().unwrap_or(0),
            io_in_progress: fields[11].parse().unwrap_or(0),
            time_io: fields[12].parse().unwrap_or(0),
            weighted_time_io: fields[13].parse().unwrap_or(0),
        });
    }

    Ok(stats)
}

#[tauri::command]
fn get_network_stats() -> Result<Vec<NetworkStats>, String> {
    let content = fs::read_to_string("/proc/net/dev").map_err(|e| e.to_string())?;
    let mut stats = Vec::new();

    for line in content.lines().skip(2) {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() != 2 {
            continue;
        }

        let interface = parts[0].trim();
        let values: Vec<u64> = parts[1].split_whitespace().map(|s| s.parse().unwrap_or(0)).collect();

        if values.len() < 16 || interface == "lo" {
            continue;
        }

        stats.push(NetworkStats {
            interface: interface.to_string(),
            bytes_received: values[0],
            packets_received: values[1],
            errors_received: values[2],
            drops_received: values[3],
            bytes_transmitted: values[8],
            packets_transmitted: values[9],
            errors_transmitted: values[10],
            drops_transmitted: values[11],
        });
    }

    Ok(stats)
}