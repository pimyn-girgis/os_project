use std::io;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    buffer::Buffer,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{palette::tailwind, Color, Style, Stylize},
    text::{Span, Text, Line},
    widgets::{Block, Borders, Gauge, Padding, Paragraph, Widget},
    Terminal,
};
use core::panic;
use getopts::Options;
<<<<<<< Updated upstream
use libc::{self, pid_t, sysinfo};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::time::Duration;
use std::{thread};
use std::io::Write;

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
<<<<<<< Updated upstream
}
=======
    if matches.opt_present("k") {
      let kill_signal = matches
        .opt_get_default::<i32>("k", libc::SIGKILL)
        .expect("Invalid signal value");
      // pro::kill_process(pid, kill_signal);
      pro::execute_on_with_arg(pids, kill_signal, pro::kill_process);
    } else if matches.opt_present("p") {
      let priority = matches.opt_get_default::<i32>("p", 0).expect("Invalid priority value");
      pro::execute_on_with_arg(pids, priority, pro::set_priority);
    } else if matches.opt_present("c") {
      let cpu_list: Vec<usize> = matches
        .opt_get_default::<String>("c", "".to_string())
        .iter()
        .map(|arg| arg.parse::<usize>().expect("Invalid CPU value"))
        .collect();
      pro::execute_on_with_args::<usize>(pids, &cpu_list, pro::bind_to_cpu_set);
    }
    return;
  }

  if matches.opt_present("t") {
    pro::build_tree(
      &pro::list_processes(
        pro::read_processes().unwrap(),
        0,
        nprocs,
        &sort_by,
        !descending,
        &filter_by,
        &pattern,
        exact_match,
      ).unwrap(),
      0,
    ).print(0);

    return;
  }

  while iterations == 0 || current_iteration != iterations {
    let stats = pro::show_stats(nprocs, &sort_by, descending, &filter_by, &pattern, exact_match);
    current_iteration += 1;

    if current_iteration == 1 {
        continue;
    }

    // Combine the components into a single output string
    let mut output = String::new();

    // Add memory info
    output.push_str(&stats.memory_info);
    output.push('\n');

    // Add CPU usage
    for line in &stats.cpu_usage {
        output.push_str(line);
        output.push('\n');
    }

    // Prepare process listing header
    output.push_str(&format!(
        "{:<6}\t{:<6}\t{:<6}\t{:<6}\t{:<8}\t{:<8}\t{:<12}\t{:<10}\t{:<10}\t{:<5}\t{:<20}",
        "UID", "PID", "PPID", "STATE", "MEM(MB)", "THREADS", "VIRT_MEM(MB)", "USER_TIME", "SYS_TIME", "Priority", "Name"
    ));
    output.push('\n');
    output.push_str(&"-".repeat(150));  // Use push_str here to append the dashed line
    output.push('\n');
    // Add processes (format each ProcessInfo as a string)
    for process in &stats.processes {
        output.push_str(&format!(
            "{:<6}\t{:<6}\t{:<6}\t{:<6}\t{:<8}\t{:<8}\t{:<12}\t{:<10}\t{:<10}\t{:<5}\t{:<20}",
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
        output.push('\n');
    }

    // Clear screen and display all at once
    print!("{esc}[2J{esc}[1;1H{}", output, esc = 27 as char);
    std::io::stdout().flush().unwrap();

    std::thread::sleep(Duration::from_secs(refresh_rate));
    writeln!(log_file, "{}", output).expect("Failed to write to log file");
}


}
>>>>>>> Stashed changes
=======
use libc::{self, cpu_set_t, pid_t, sched_setaffinity, sysinfo, CPU_SET, CPU_ZERO};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
mod pro;


const GAUGE1_COLOR: Color = tailwind::RED.c800;
const GAUGE2_COLOR: Color = tailwind::GREEN.c800;
const GAUGE3_COLOR: Color = tailwind::BLUE.c800;
const GAUGE4_COLOR: Color = tailwind::ORANGE.c800;
const CUSTOM_LABEL_COLOR: Color = tailwind::SLATE.c200;


#[derive(Debug, Default)]
pub struct App {
    exit: bool,
}

impl App {
    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| {
                self.draw_layout(frame);
            })?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw_layout(&self, frame: &mut ratatui::Frame) {
      let area = frame.area();
  
      let layout = Layout::default()
          .direction(Direction::Vertical)
          .constraints([
              Constraint::Percentage(30), // Reduced CPU usage section
              Constraint::Percentage(65), // Expanded process list section
              Constraint::Percentage(5),  // Buttons section
          ])
          .split(area);
  
      let titles = [
          "USER", "PID", "PPID", "STATE", "MEM(MB)", "THREADS", "VIRT_MEM(MB)",
          "USER_TIME", "SYS_TIME", "PRIORITY", "NAME",
      ];
  
      // Properly handle the Result
      let processes = match pro::read_processes() {
          Ok(procs) => procs,
          Err(e) => {
              let error_paragraph = Paragraph::new(format!("Error reading processes: {}", e))
                  .style(Style::default().fg(Color::Red));
              frame.render_widget(error_paragraph, layout[1]);
              return;
          }
      };
  
      // Create a layout for titles and process list
      let process_layout = Layout::default()
          .direction(Direction::Vertical)
          .constraints([
              Constraint::Length(1), // Space for titles
              Constraint::Min(0),    // Remaining space for processes
          ])
          .split(layout[1]);
  
      // Create columns for titles
      let columns = Layout::default()
          .direction(Direction::Horizontal)
          .constraints(
              titles
                  .iter()
                  .map(|_| Constraint::Percentage(100 / titles.len() as u16))
                  .collect::<Vec<_>>(),
          )
          .split(process_layout[0]);
  
      // Render titles
      for (i, title) in titles.iter().enumerate() {
          let paragraph = Paragraph::new(Line::from(title.green()))
              .alignment(Alignment::Center);
          frame.render_widget(paragraph, columns[i]);
      }
  
      let process_rows = Layout::default()
          .direction(Direction::Vertical)
          .constraints(
              processes.iter().map(|_| Constraint::Length(1)).collect::<Vec<_>>(),
          )
          .split(process_layout[1]);
  
      for (i, process) in processes.iter().enumerate() {
          let process_data: Vec<String> = vec![
              process.user.clone(),
              process.pid.to_string(),
              process.ppid.to_string(),
              process.state.to_string(),
              format!("{:.2} MB", process.memory as f64 / 1024.0 / 1024.0),
              process.thread_count.to_string(),
              format!("{:.2} MB", process.virtual_memory as f64 / 1024.0 / 1024.0),
              process.user_time.to_string(),
              process.system_time.to_string(),
              process.priority.to_string(),
              process.name.clone(),
          ];
  
          let process_columns = Layout::default()
              .direction(Direction::Horizontal)
              .constraints(
                  titles
                      .iter()
                      .map(|_| Constraint::Percentage(100 / titles.len() as u16))
                      .collect::<Vec<_>>(),
              )
              .split(process_rows[i]);
  
          for (j, data) in process_data.iter().enumerate() {
              let paragraph = Paragraph::new(Line::from(data.clone())) 
                  .alignment(Alignment::Center)
                  .style(Style::default().fg(Color::White));
              frame.render_widget(paragraph, process_columns[j]);
          }
      }


      let cpus = Layout::default()
      .direction(Direction::Vertical)
      .constraints(
          [0, 1, 2, 3, 4, 5, 6, 7] // change with the actual list of cpus
          .iter()
          .map(|_| Constraint::Percentage(100 / titles.len() as u16))
          .collect::<Vec<_>>(),
      ).split(layout[0]);

        for (i, cpu) in cpus.iter().enumerate() {
            let label = format!("CPU {i}: {:.1}%", 20); // change with the actual cpu usage
            let usage = Gauge::default()
                .gauge_style(GAUGE3_COLOR)
                .ratio(0.2)
                .label(label);
            frame.render_widget(usage, cpus[i]);
        }

        let buttons = ["Help", "Tree", "Search", "SortBy", "Nice", "Kill", "Quit"];
        let button_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                buttons
                    .iter()
                    .map(|_| Constraint::Percentage(50 / buttons.len() as u16))
                    .collect::<Vec<_>>(),
            )
            .split(layout[2]);

        for (i, label) in buttons.iter().enumerate() {
            let paragraph = Paragraph::new(Line::from(label.green())).alignment(Alignment::Center);
            frame.render_widget(paragraph, button_layout[i]);
        }
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

fn main() -> io::Result<()> {
  
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let app_result = App::default().run(&mut terminal);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    app_result
}
>>>>>>> Stashed changes
