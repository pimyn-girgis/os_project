use core::panic;
use getopts::Options;
use libc::{self, cpu_set_t, pid_t, sched_setaffinity, sysinfo, CPU_SET, CPU_ZERO};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    static ref CACHE: Mutex<BTreeMap<u32, String>> = Mutex::new(BTreeMap::new());
    static ref PREV_STATS: Mutex<Vec<(u64, u64)>> = Mutex::new(Vec::new());
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
    pub priority: i32,
}

fn get_username_from_uid(target_uid: u32) -> Option<String> {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(username) = cache.get(&target_uid) {
        return Some(username.clone());
    }

    let file = File::open("/etc/passwd").ok()?;
    for line in BufReader::new(file).lines().flatten() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() >= 3 {
            if let Ok(uid) = fields[2].parse::<u32>() {
                if uid == target_uid {
                    let username = fields[0].to_string();
                    cache.insert(uid, username.clone());
                    return Some(username);
                }
            }
        }
    }
    None
}

pub fn bind_to_cpu_set(pid: pid_t, cpu_ids: &[usize]) -> io::Result<()> {
    let mut cpuset: cpu_set_t = unsafe { std::mem::zeroed() };

    unsafe {
        CPU_ZERO(&mut cpuset);
        for &cpu_id in cpu_ids {
            CPU_SET(cpu_id, &mut cpuset);
        }
    }

    let result = unsafe { sched_setaffinity(pid, std::mem::size_of::<cpu_set_t>(), &cpuset as *const _) };
    if result == 0 {
        println!("Process {} bound to CPUs {:?}", pid, cpu_ids);
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn parse_status_line(line: &str) -> io::Result<(String, Vec<String>)> {
    let line_parts: Vec<&str> = line.split(':').collect();
    if line_parts.len() == 2 {
        let key = line_parts[0].trim().to_string();
        let values = line_parts[1].split_whitespace().map(|s| s.to_string()).collect();
        Ok((key, values))
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid status line format"))
    }
}

pub fn read_process_info(pid: pid_t) -> io::Result<ProcessInfo> {
    let status_path = format!("/proc/{}/status", pid);
    let status_content = fs::read_to_string(&status_path)?;
    let mut status_map = HashMap::new();

    for line in status_content.lines() {
        if let Ok((key, values)) = parse_status_line(line) {
            status_map.insert(key, values);
        }
    }

    Ok(ProcessInfo {
        user: get_username_from_uid(
            status_map.get("Uid").and_then(|v| v.get(0)).unwrap_or(&"0".to_string()).parse().unwrap_or(0)
        ).unwrap_or_default(),
        pid,
        ppid: status_map.get("PPid").and_then(|v| v.get(0)).unwrap_or(&"0".to_string()).parse().unwrap_or(0),
        name: status_map.get("Name").and_then(|v| v.get(0)).cloned().unwrap_or_default(),
        state: status_map.get("State").and_then(|v| v.get(0)).and_then(|s| s.chars().next()).unwrap_or_default(),
        memory: status_map.get("VmRSS").and_then(|v| v.get(0)).unwrap_or(&"0".to_string()).parse().unwrap_or(0),
        thread_count: status_map.get("Threads").and_then(|v| v.get(0)).unwrap_or(&"0".to_string()).parse().unwrap_or(0),
        virtual_memory: status_map.get("VmSize").and_then(|v| v.get(0)).unwrap_or(&"0".to_string()).parse().unwrap_or(0),
        priority: 0, // Priority fetching logic can be added here.
    })
}