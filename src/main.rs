use libc::{self, pid_t, CPU_SET, CPU_ZERO, sched_setaffinity};
use std::fs;
use std::io::{self, Write};
use std::process::exit;
use std::process::{Command};
use std::env;
use affinity::{get_core_num, set_thread_affinity, get_thread_affinity};

struct ProcessInfo {
    pid: pid_t,
    ppid: pid_t,
    name: String,
    state: char,
    memory: u64,
    cpu: usize,
}

// Function to bind a process to a set of CPUs
fn bind_to_cpu_set(pid: pid_t, cpu_ids: &Vec<usize>) -> io::Result<()> {
    let mut cpuset: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe {
        CPU_ZERO(&mut cpuset); // Initialize to empty set
        for &cpu_id in cpu_ids {
            CPU_SET(cpu_id, &mut cpuset); // Add each CPU in the set to the cpuset
        }
    }

    let result = unsafe {
        sched_setaffinity(pid, std::mem::size_of::<libc::cpu_set_t>(), &cpuset)
    };

    if result == 0 {
        println!("Process {} bound to CPUs {:?}", pid, cpu_ids);
        Ok(())
    } else {
        eprintln!("Failed to set CPU affinity for process {}", pid);
        Err(io::Error::last_os_error())
    }
}

fn is_root() -> bool {
    unsafe { libc::getuid() == 0 }
}

fn bind_others_to_other_cpus(selected_pids: &Vec<pid_t>, selected_cpu_ids: &Vec<usize>) -> io::Result<()> {
    let num_cpus = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) } as usize;
    if num_cpus < 2 {
        return Err(io::Error::new(io::ErrorKind::Other, "Insufficient CPUs"));
    }

    // Create a CPU set for the complementary CPUs (the ones not selected)
    let mut complementary_cpu_ids: Vec<usize> = (0..num_cpus).collect();
    complementary_cpu_ids.retain(|&cpu_id| !selected_cpu_ids.contains(&cpu_id)); // Keep only CPUs that are not selected

    // For each process, bind it to the complementary CPUs using `taskset` command
    for pid in selected_pids {
        // Skip processes that are already selected
        let pid_str = pid.to_string();
        
        // Create a mask string for the complementary CPUs
        let mut mask = String::new();
        for cpu in complementary_cpu_ids.iter() {
            mask.push_str(&format!("{:b}", 1 << cpu));
        }

        // Run taskset command to bind the process to complementary CPUs
        let status = Command::new("sudo")
            .arg("taskset")
            .arg("-p")
            .arg(mask)  // The CPU affinity mask
            .arg(&pid_str)  // The target process PID
            .status()?;

        if !status.success() {
            eprintln!("Error binding process {} to complementary CPUs", pid);
        }
    }

    Ok(())
}


fn reset_cpu_affinity(pid: pid_t) -> io::Result<()> {
    let mut cpuset: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe {
        CPU_ZERO(&mut cpuset); // Initialize to empty set
        let num_cpus = libc::sysconf(libc::_SC_NPROCESSORS_ONLN);
        if num_cpus < 1 {
            return Err(io::Error::new(io::ErrorKind::Other, "Failed to get number of CPUs"));
        }
        for cpu_id in 0..num_cpus {
            CPU_SET(cpu_id as usize, &mut cpuset); // Add all CPUs to the set
        }
    }

    let result = unsafe {
        sched_setaffinity(pid, std::mem::size_of::<libc::cpu_set_t>(), &cpuset)
    };

    if result == 0 {
        println!("CPU affinity for process {} reset to all CPUs", pid);
        Ok(())
    } else {
        eprintln!("Failed to reset CPU affinity for process {}", pid);
        Err(io::Error::last_os_error())
    }
}

// Read process info from /proc/<pid>/status and /proc/<pid>/stat
fn read_process_info(pid: pid_t) -> io::Result<ProcessInfo> {
    let status_path = format!("/proc/{}/status", pid);
    let stat_path = format!("/proc/{}/stat", pid);

    let status_content = fs::read_to_string(status_path)?;
    let stat_content = fs::read_to_string(stat_path)?;

    let mut name = String::new();
    let mut ppid = 0;
    let mut state = '?';
    let mut memory = 0;
    let mut cpu = 0;

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

    if let Some(last_cpu_field) = stat_content.split_whitespace().nth(38) {
        cpu = last_cpu_field.parse::<usize>().unwrap_or(0);
    }

    Ok(ProcessInfo {
        pid,
        ppid,
        name,
        state,
        memory,
        cpu,
    })
}

// List processes from /proc
fn list_processes() -> io::Result<Vec<ProcessInfo>> {
    let mut processes = Vec::new();

    for entry in fs::read_dir("/proc")? {
        let entry = entry?; // Loop through /proc directory
        let path = entry.path();
        if let Some(name) = path.file_name() {
            if let Some(name_str) = name.to_str() {
                if let Ok(pid) = name_str.parse::<pid_t>() {
                    match read_process_info(pid) {
                        Ok(info) => processes.push(info),
                        Err(_) => continue, // Skip processes that couldn't be read
                    }
                }
            }
        }
    }

    Ok(processes)
}

fn main() -> io::Result<()> {
    println!("PID\tPPID\tSTATE\tMEM(KB)\tNAME\tCPU");
    println!("{}", "-".repeat(60));

    match list_processes() {
        Ok(mut processes) => {
            processes.sort_by_key(|p| p.pid); // Sort processes by PID
            for process in &processes {
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    process.pid, process.ppid, process.state, process.memory, process.name, process.cpu
                );
            }

            print!("Enter the PIDs of the processes to bind to a set of CPUs (comma-separated): ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let pids: Vec<pid_t> = input
                .trim()
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();

            print!("Enter the CPU IDs to bind the processes to (comma-separated): ");
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            let cpu_ids: Vec<usize> = input
                .trim()
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();

            // Bind the selected processes to the chosen CPUs
            for pid in &pids {
                if let Err(e) = bind_to_cpu_set(*pid, &cpu_ids) {
                    eprintln!("Error binding process {} to CPUs {:?}: {}", pid, cpu_ids, e);
                    exit(1);
                }
            }

            // Bind all other processes to other CPUs
            if let Err(e) = bind_others_to_other_cpus(&pids, &cpu_ids) {
                eprintln!("Error binding other processes: {}", e);
                exit(1);
            }

            print!("Do you want to reset the CPU affinity and restore default? (y/n): ");
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            if input.trim().eq_ignore_ascii_case("y") {
                for pid in &pids {
                    if let Err(e) = reset_cpu_affinity(*pid) {
                        eprintln!("Error resetting CPU affinity: {}", e);
                    }
                }
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("Error listing processes: {}", e);
            Err(e)
        }
    }
}
