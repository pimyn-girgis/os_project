use core::panic;
use getopts::Options;
use libc::{self, cpu_set_t, pid_t, sched_setaffinity, sysinfo, CPU_SET, CPU_ZERO};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::sync::mpsc::Sender;

pub struct OutputMessage {
  pub message: String,
  pub is_error: bool,
}

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

pub struct ProcessInfoIntoIterator {
  process_info: ProcessInfo,
  index: usize,
}

impl IntoIterator for ProcessInfo {
  type Item = String;
  type IntoIter = ProcessInfoIntoIterator;
  fn into_iter(self) -> Self::IntoIter {
    ProcessInfoIntoIterator {
      process_info: self,
      index: 0,
    }
  }
}

impl Iterator for ProcessInfoIntoIterator {
  type Item = String;
  fn next(&mut self) -> Option<Self::Item> {
    let result = match self.index {
      0 => Some(self.process_info.user.clone()),
      1 => Some(self.process_info.pid.to_string()),
      2 => Some(self.process_info.ppid.to_string()),
      3 => Some(self.process_info.state.to_string()),
      4 => Some((self.process_info.memory / 1000).to_string()),
      5 => Some(self.process_info.thread_count.to_string()),
      6 => Some((self.process_info.virtual_memory / 1000).to_string()),
      7 => Some(self.process_info.user_time.to_string()),
      8 => Some(self.process_info.system_time.to_string()),
      9 => Some(self.process_info.priority.to_string()),
      10 => Some(self.process_info.name.clone()),
      _ => None,
    };
    self.index += 1;
    result
  }
}

impl Display for ProcessInfo {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(
      f,
      "{:<10}\t{:<6}\t{:<6}\t{:<1}\t{:<6}\t{:<2}\t{:<6}\t{:<10}\t{:<10}\t{:<3}\t{:<40}",
      self.user,
      self.pid,
      self.ppid,
      self.state,
      self.memory / 1000,
      self.thread_count,
      self.virtual_memory / 1000,
      self.user_time,
      self.system_time,
      self.priority,
      self.name
    )
  }
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

pub fn bind_to_cpu_set(pid: pid_t, cpu_ids: &Vec<usize>, sender: Option<&Sender<OutputMessage>>) -> io::Result<()> {
  let mut cpuset: cpu_set_t = unsafe { std::mem::zeroed() };

  unsafe {
    CPU_ZERO(&mut cpuset);
    for &cpu_id in cpu_ids {
      CPU_SET(cpu_id, &mut cpuset);
    }
  }

  let result = unsafe { sched_setaffinity(pid, std::mem::size_of::<cpu_set_t>(), &cpuset as *const _) };

  if result == 0 {
    let msg = format!("Process {} bound to CPUs {:?}", pid, cpu_ids);
    if let Some(tx) = sender {
      let _ = tx.send(OutputMessage {
        message: msg,
        is_error: false,
      });
    } else {
      println!("{}", msg);
    }
    Ok(())
  } else {
    let error = io::Error::last_os_error();
    let msg = format!("Failed to set CPU affinity for process {}. Error: {:?}", pid, error);
    if let Some(tx) = sender {
      let _ = tx.send(OutputMessage {
        message: msg,
        is_error: true,
      });
    } else {
      eprintln!("{}", msg);
    }
    Err(error)
  }
}

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
        "pid" => p.pid.to_string(),
        "ppid" => p.ppid.to_string(),
        "state" => p.state.to_string(),
        "any" => p.to_string(),
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
        let total: u64 = values[1..].iter().map(|&s| s.parse::<u64>().unwrap_or(0)).sum();
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

pub fn show_stats(
  nprocs: usize,
  sort_by: &str,
  descending: bool,
  filter_by: &str,
  pattern: &str,
  exact_match: bool,
) -> String {
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

  output.push_str(&format!(
    "{:<6}\t{:<6}\t{:<6}\t{:<6}\t{:<8}\t{:<8}\t{:<12}\t{:<10}\t{:<10}\t{:<8}\t{:<20}\n",
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
        output.push_str(&process.to_string());
        output.push('\n');
      }
    }
    Err(e) => {
      output.push_str(&format!("Error listing processes: {}\n", e));
    }
  }
  output
}

pub fn kill_process(pid: pid_t, signal: i32, sender: Option<&Sender<OutputMessage>>) {
  let msg = format!("Killing process {} with signal {}", pid, signal);
  if let Some(tx) = sender {
    let _ = tx.send(OutputMessage {
      message: msg,
      is_error: false,
    });
  } else {
    println!("{}", msg);
  }
  unsafe {
    libc::kill(pid, signal);
  }
}

pub fn print_usage(program: &str, opts: Options) {
  let brief = format!("Usage: {} [options]", program);
  print!("{}", opts.usage(&brief));
}

pub fn set_priority(pid: pid_t, priority: i32, sender: Option<&Sender<OutputMessage>>) {
  unsafe {
    if libc::setpriority(libc::PRIO_PROCESS, pid.try_into().unwrap(), priority) == -1 {
      let msg = format!("Failed to set priority: {}", std::io::Error::last_os_error());
      if let Some(tx) = sender {
        let _ = tx.send(OutputMessage {
          message: msg,
          is_error: true,
        });
      } else {
        eprintln!("{}", msg);
      }
    } else {
      let msg = format!("Successfully set priority of process {} to {}", pid, priority);
      if let Some(tx) = sender {
        let _ = tx.send(OutputMessage {
          message: msg,
          is_error: false,
        });
      } else {
        println!("{}", msg);
      }
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

pub fn execute_on_with_arg<T: std::marker::Copy>(
  pids: Vec<pid_t>,
  arg: T,
  fn_ptr: fn(pid_t, T, Option<&Sender<OutputMessage>>),
  sender: Option<&Sender<OutputMessage>>,
) {
  for pid in pids {
    fn_ptr(pid, arg, sender);
  }
}

pub fn execute_on_with_args<T: std::marker::Copy>(
  pids: Vec<pid_t>,
  args: &Vec<T>,
  fn_ptr: fn(pid_t, &Vec<T>, Option<&Sender<OutputMessage>>) -> io::Result<()>,
  sender: Option<&Sender<OutputMessage>>,
) {
  for pid in pids {
    let _ = fn_ptr(pid, args, sender);
  }
}

pub fn execute_on(pids: Vec<pid_t>, fn_ptr: fn(pid_t)) {
  for pid in pids {
    fn_ptr(pid);
  }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

pub fn get_disk_stats() -> io::Result<Vec<DiskStats>> {
  let content = fs::read_to_string("/proc/diskstats")?;
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

pub fn get_disk_rates(previous: &[DiskStats], current: &[DiskStats], elapsed_seconds: f64) -> Vec<(String, f64, f64)> {
  let mut rates = Vec::new();

  for curr in current {
    if let Some(prev) = previous.iter().find(|p| p.device == curr.device) {
      let read_bytes = (curr.sectors_read - prev.sectors_read) * 512;
      let write_bytes = (curr.sectors_written - prev.sectors_written) * 512;

      let read_rate = read_bytes as f64 / elapsed_seconds;
      let write_rate = write_bytes as f64 / elapsed_seconds;

      rates.push((curr.device.clone(), read_rate, write_rate));
    }
  }

  rates
}

pub fn get_network_stats() -> io::Result<Vec<NetworkStats>> {
  let content = fs::read_to_string("/proc/net/dev")?;
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

pub fn get_network_rates(
  previous: &[NetworkStats],
  current: &[NetworkStats],
  elapsed_seconds: f64,
) -> Vec<(String, f64, f64)> {
  let mut rates = Vec::new();

  for curr in current {
    if let Some(prev) = previous.iter().find(|p| p.interface == curr.interface) {
      let rx_rate = (curr.bytes_received - prev.bytes_received) as f64 / elapsed_seconds;
      let tx_rate = (curr.bytes_transmitted - prev.bytes_transmitted) as f64 / elapsed_seconds;

      rates.push((curr.interface.clone(), rx_rate, tx_rate));
    }
  }

  rates
}

pub fn format_rate(bytes_per_sec: f64) -> String {
  const KB: f64 = 1024.0;
  const MB: f64 = KB * 1024.0;
  const GB: f64 = MB * 1024.0;

  if bytes_per_sec >= GB {
    format!("{:.2} GB/s", bytes_per_sec / GB)
  } else if bytes_per_sec >= MB {
    format!("{:.2} MB/s", bytes_per_sec / MB)
  } else if bytes_per_sec >= KB {
    format!("{:.2} KB/s", bytes_per_sec / KB)
  } else {
    format!("{:.0} B/s", bytes_per_sec)
  }
}
