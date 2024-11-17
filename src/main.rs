use core::panic;
use getopts::Options;
use libc::{self, pid_t, sysinfo,sched_setaffinity, CPU_SET, CPU_ZERO, cpu_set_t};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::time::Duration;
use std::{thread};
use std::io::Write;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{self, Clear, ClearType},
};

struct ProcessInfo {
    pid: pid_t,
    ppid: pid_t,
    name: String,
    state: char,
    memory: u64,
    exe_path: String,
    thread_count: u64,
    virtual_memory: u64,
    user_time: u64,
    system_time: u64,
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
    let result = unsafe {
        sched_setaffinity(pid, std::mem::size_of::<cpu_set_t>(), &cpuset as *const _)
    };

    if result == 0 {
        println!("Process {} bound to CPUs {:?}", pid, cpu_ids);
        Ok(())
    } else {
        eprintln!("Failed to set CPU affinity for process {}. Error: {:?}", pid, io::Error::last_os_error());
        Err(io::Error::last_os_error())
    }
}

fn is_root() -> bool {
    unsafe { libc::getuid() == 0 }
}

// Read process info from /proc/<pid>/status and /proc/<pid>/stat
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

    let stat_content1 = fs::read_to_string("/proc/stat")?;
    let stats1 = parse_cpu_stats(&stat_content1);
    thread::sleep(Duration::from_secs(1));
    let stat_content2 = fs::read_to_string("/proc/stat")?;
    let stats2 = parse_cpu_stats(&stat_content2);

    let mut cpu_usage = Vec::new();
    for (stat1, stat2) in stats1.iter().zip(stats2.iter()) {
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
                // Listen for keyboard events to bind processes
                if event::poll(Duration::from_millis(1000)).unwrap() {
                    if let Event::Key(event) = event::read().unwrap() {
                        match event.code {
                            KeyCode::Char('p') => {
                                // Get PID and CPU IDs from the user
                                println!("'p' key pressed");
                                let mut input = String::new();
                                println!("Enter the PID of the process to bind:");
                                io::stdin().read_line(&mut input).unwrap();
                                let pid: pid_t = match input.trim().parse() {
                                    Ok(pid) => pid,
                                    Err(_) => {
                                        eprintln!("Invalid PID. Returning to main menu.");
                                        continue; // Skip this iteration
                                    }
                                };
                            
                                input.clear();
                                println!("Enter the CPU IDs to bind the process to (comma-separated):");
                                io::stdin().read_line(&mut input).unwrap();
                                let cpu_ids: Vec<usize> = input
                                    .trim()
                                    .split(',')
                                    .filter_map(|s| s.trim().parse().ok())
                                    .map(|id: usize| id - 1)  // Subtract 1 from each CPU ID as the CPU are zero based
                                    .collect();
                            
                                if cpu_ids.is_empty() {
                                    eprintln!("No valid CPU IDs provided. Returning to main menu.");
                                    continue;
                                }
                            
                                match bind_to_cpu_set(pid, &cpu_ids) {
                                    Ok(_) => println!("Successfully bound process {} to CPUs {:?}", pid, cpu_ids),
                                    Err(e) => eprintln!("Failed to bind process: {}", e),
                                }
                            }
                            _ => {
                                // Handle all other keys
                                println!("Unhandled key pressed: {:?}", event.code);
                            }
                        } 
                        
                    }
                }
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

        output.push_str(&format!(
            "{:<6}\t{:<6}\t{:<6}\t{:<8}\t{:<8}\t{:<12}\t{:<10}\t{:<10}\t{}\n",
            "PID", "PPID", "STATE", "MEM(KB)", "THREADS", "VIRT_MEM(KB)", "USER_TIME", "SYS_TIME", "EXE PATH"
        ));
        output.push_str(&format!("{}\n", "-".repeat(100)));

        match list_processes() {
            Ok(mut processes) => {
                processes.sort_by_key(|p| p.pid);
                for process in processes {
                    output.push_str(&format!(
                        "{:<6}\t{:<6}\t{:<6}\t{:<8}\t{:<8}\t{:<12}\t{:<10}\t{:<10}\t{}\n",
                        process.pid,
                        process.ppid,
                        process.state,
                        process.memory,
                        process.thread_count,
                        process.virtual_memory,
                        process.user_time,
                        process.system_time,
                        process.exe_path
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
    }
}