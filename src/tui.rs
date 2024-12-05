use crate::pro::{self, list_processes, OutputMessage};
use core::fmt;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use humansize::{format_size, BINARY};
use libc::sysinfo;
use ratatui::{
  layout::{Constraint, Layout},
  prelude::Backend,
  style::{Color, Style, Stylize},
  symbols::{self, Marker},
  widgets::{Block, Chart, Dataset, Gauge, GraphType, Paragraph, Row, Table, TableState, Tabs},
  Frame, Terminal,
};
use std::{
  collections::HashMap,
  fmt::Debug,
  io,
  sync::mpsc::{self, Receiver, Sender},
  thread, time,
};

#[derive(Debug)]
enum InputMessage {
  KeyPress(KeyEvent),
  SearchInput(char),
  ModifySort(String),
  ClearSearch,
  Backspace,
  SearchEnd,
  ModifySearch,
  Quit,
}

#[derive(PartialEq)]
enum Mode {
  Search,
  Normal,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurrentTab {
  Processes,
  Cpu,
  System,
  Disk,
  Network,
  Max,
}

impl TryFrom<u8> for CurrentTab {
  type Error = ();
  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(Self::Processes),
      1 => Ok(Self::Cpu),
      2 => Ok(Self::System),
      3 => Ok(Self::Disk),
      4 => Ok(Self::Network),
      _ => Err(()),
    }
  }
}

impl fmt::Display for CurrentTab {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

pub struct App {
  processes: Vec<pro::ProcessInfo>,
  accessible_processes: Vec<pro::ProcessInfo>,
  from: usize,
  nprocs: usize,
  sort_by: String,
  ascending: bool,
  filter_by: String,
  pattern: String,
  exact_match: bool,
  exit: bool,
  refresh_rate: std::time::Duration,
  time: std::time::Instant,
  current_tab: CurrentTab,
  mode: Mode,
  table_state: TableState,
  input_rx: Receiver<InputMessage>,
  sysinfo: Option<sysinfo>,
  load_history: Vec<(f64, f64)>,
  memory_history: Vec<(f64, f64)>,
  cpu_history: Vec<(f64, f64)>,
  cpu_usage: Vec<f64>,
  disk_stats: Vec<pro::DiskStats>,
  prev_disk_stats: Vec<pro::DiskStats>,
  disk_history: HashMap<String, Vec<(f64, f64, f64)>>,
  network_stats: Vec<pro::NetworkStats>,
  prev_network_stats: Vec<pro::NetworkStats>,
  network_history: HashMap<String, Vec<(f64, f64, f64)>>,
  status_message: Option<String>,
  status_message_error: bool,
  status_message_time: Option<std::time::Instant>,
  output_tx: Sender<OutputMessage>,
  output_rx: Receiver<OutputMessage>,
}

fn spawn_input_handler(tx: Sender<InputMessage>) {
  thread::spawn(move || loop {
    if let Ok(event) = event::read() {
      match event {
        Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
          let msg = match key_event.code {
            KeyCode::Char('s') => {
              let mut sort_by = "pid";
              if let Ok(Event::Key(key_event)) = event::read() {
                sort_by = match key_event.code {
                  KeyCode::Char('n') => "name",
                  KeyCode::Char('p') => "pid",
                  KeyCode::Char('u') => "user",
                  KeyCode::Char('m') => "memory",
                  _ => "pid",
                };
              }
              InputMessage::ModifySort(sort_by.to_string())
            }
            KeyCode::Char('q') => InputMessage::Quit,
            KeyCode::Char('/') => {
              let _ = tx.send(InputMessage::KeyPress(key_event));
              loop {
                if let Ok(Event::Key(key_event)) = event::read() {
                  if key_event.kind == KeyEventKind::Press {
                    match key_event.code {
                      KeyCode::Enter => {
                        break;
                      }
                      KeyCode::Esc => {
                        let _ = tx.send(InputMessage::ClearSearch);
                        break;
                      }
                      KeyCode::Char(c) => {
                        let _ = tx.send(InputMessage::SearchInput(c));
                      }
                      KeyCode::Backspace => {
                        let _ = tx.send(InputMessage::Backspace);
                      }
                      _ => {}
                    }
                    let _ = tx.send(InputMessage::ModifySearch);
                  }
                }
              }
              InputMessage::SearchEnd
            }
            _ => InputMessage::KeyPress(key_event),
          };
          if tx.send(msg).is_err() {
            break;
          }
        }
        _ => {}
      }
    }
  });
}

impl App {
  pub fn new() -> Self {
    let (tx, rx) = mpsc::channel();
    let (output_tx, output_rx) = mpsc::channel();
    spawn_input_handler(tx);

    Self {
      processes: pro::read_processes().unwrap(),
      accessible_processes: Vec::new(),
      from: 0,
      nprocs: usize::MAX,
      sort_by: String::from("pid"),
      ascending: true,
      filter_by: String::from("any"),
      pattern: String::from(""),
      exact_match: false,
      exit: false,
      refresh_rate: std::time::Duration::from_secs(1),
      time: std::time::Instant::now() - std::time::Duration::from_secs(1),
      current_tab: CurrentTab::Processes,
      table_state: TableState::default(),
      input_rx: rx,
      mode: Mode::Normal,
      sysinfo: None,
      load_history: Vec::with_capacity(100),
      memory_history: Vec::with_capacity(100),
      cpu_history: Vec::with_capacity(100),
      cpu_usage: Vec::new(),
      disk_stats: Vec::new(),
      prev_disk_stats: Vec::new(),
      disk_history: HashMap::new(),
      network_stats: Vec::new(),
      prev_network_stats: Vec::new(),
      network_history: HashMap::new(),
      status_message: None,
      status_message_error: false,
      status_message_time: None,
      output_tx,
      output_rx,
    }
  }

  fn update_network_info(&mut self) {
    if let Ok(stats) = pro::get_network_stats() {
      let rates = pro::get_network_rates(&self.prev_network_stats, &stats, self.refresh_rate.as_secs_f64());

      for (interface, rx_rate, tx_rate) in rates {
        let history = self.network_history.entry(interface).or_default();
        if history.len() >= 100 {
          history.remove(0);
        }
        history.push((history.len() as f64, rx_rate, tx_rate));
      }

      self.prev_network_stats = self.network_stats.clone();
      self.network_stats = stats;
    }
  }

  fn update_cpu_info(&mut self) {
    if let Ok(usage) = pro::get_cpu_usage() {
      self.cpu_usage = usage;

      if self.cpu_history.len() >= 100 {
        self.cpu_history.remove(0);
      }

      let avg_usage = self.cpu_usage.iter().sum::<f64>() / self.cpu_usage.len() as f64;
      self.cpu_history.push((self.cpu_history.len() as f64, avg_usage));
    }
  }

  fn update_disk_info(&mut self) {
    if let Ok(stats) = pro::get_disk_stats() {
      let rates = pro::get_disk_rates(&self.prev_disk_stats, &stats, self.refresh_rate.as_secs_f64());

      for (device, read_rate, write_rate) in rates {
        let history = self.disk_history.entry(device).or_default();
        if history.len() >= 100 {
          history.remove(0);
        }
        history.push((history.len() as f64, read_rate, write_rate));
      }

      self.prev_disk_stats = self.disk_stats.clone();
      self.disk_stats = stats;
    }
  }

  fn format_uptime(seconds: i64) -> String {
    let days = seconds / (24 * 3600);
    let hours = (seconds % (24 * 3600)) / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    format!("{}d {}h {}m {}s", days, hours, minutes, seconds)
  }

  pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
    while !self.exit {
      let time = time::Instant::now();
      if (time - self.time) > self.refresh_rate {
        self.update_processes();
        self.update_sysinfo();
        self.update_cpu_info();
        self.update_disk_info();
        self.update_network_info();
        self.clear_status_after_delay();
        self.time = time;
      }

      // Check for output messages
      if let Ok(output_msg) = self.output_rx.try_recv() {
        self.status_message = Some(output_msg.message);
        self.status_message_error = output_msg.is_error;
        self.status_message_time = Some(std::time::Instant::now());
      }

      let _ = self.list_processes();
      terminal.draw(|frame| self.draw(frame))?;

      if let Ok(message) = self.input_rx.try_recv() {
        self.handle_input_message(message);
      }
    }
    Ok(())
  }

  fn clear_status_after_delay(&mut self) {
    if let Some(time) = self.status_message_time {
      if time.elapsed() > std::time::Duration::from_secs(3) {
        self.status_message = None;
        self.status_message_time = None;
      }
    }
  }

  fn update_processes(&mut self) {
    self.processes = pro::read_processes().unwrap();
  }

  fn update_sysinfo(&mut self) {
    let info = pro::get_sysinfo();
    if self.load_history.len() >= 100 {
      self.load_history.remove(0);
    }
    let load_avg = info.loads[0] as f64 / 65536.0;
    self.load_history.push((self.load_history.len() as f64, load_avg));

    if self.memory_history.len() >= 100 {
      self.memory_history.remove(0);
    }
    let used_mem = (info.totalram - info.freeram) as f64 / info.totalram as f64 * 100.0;
    self.memory_history.push((self.memory_history.len() as f64, used_mem));

    self.sysinfo = Some(info);
  }

  fn handle_input_message(&mut self, message: InputMessage) {
    match message {
      InputMessage::ModifySort(sort_by) => {
        self.status_message = Some(format!("Sorting by {}", sort_by));
        self.sort_by = sort_by;
        self.ascending = true;
      }
      InputMessage::ClearSearch => {
        self.pattern = String::new();
        self.mode = Mode::Normal;
      }
      InputMessage::KeyPress(key_event) => self.handle_key_event(key_event),
      InputMessage::SearchInput(c) => {
        if self.mode == Mode::Normal {
          self.pattern = String::new();
          self.mode = Mode::Search;
        }
        self.pattern.push(c);
      }
      InputMessage::Backspace => {
        self.pattern.pop();
      }
      InputMessage::SearchEnd => self.mode = Mode::Normal,
      InputMessage::ModifySearch => self.filter_by = "any".to_string(),
      InputMessage::Quit => self.exit = true,
    }
  }

  fn draw(&mut self, frame: &mut Frame) {
    use Constraint::{Length, Min, Percentage};

    let vertical = Layout::vertical([Length(1), Min(0)]);
    let [title_area, main_area] = vertical.areas(frame.area());
    let tab_area = Layout::horizontal([Percentage(15), Percentage(85)]).split(title_area);
    let main_area = Layout::vertical([Percentage(100), Min(1)]).split(main_area);

    frame.render_widget(Block::bordered().title("AMR KADI Pro"), tab_area[0]);
    let mut tabs: Vec<String> = Vec::new();
    for i in 0..CurrentTab::Max as usize {
      tabs.push(CurrentTab::try_from(i as u8).expect("").to_string());
    }
    frame.render_widget(
      Tabs::new(tabs)
        .select(self.current_tab as usize)
        .style(Style::default().white())
        .highlight_style(Style::default().yellow())
        .divider(symbols::DOT),
      tab_area[1],
    );

    match self.current_tab {
      CurrentTab::Processes => {
        let header = Row::new([
          "UID",
          "PID",
          "PPID",
          "STATE",
          "MEM(MB)",
          "THREADS",
          "VIRT_MEM(MB)",
          "USER_TIME",
          "SYS_TIME",
          "Priority",
          "Name",
        ]);

        frame.render_stateful_widget(
          Table::new(
            self.accessible_processes.iter().map(|f| Row::new(f.clone())),
            [6, 6, 6, 5, 7, 7, 12, 9, 9, 9, 30],
          )
          .block(Block::bordered().title("Processes"))
          .highlight_symbol(">>")
          .row_highlight_style(Style::default().bg(Color::DarkGray))
          .header(header),
          main_area[0],
          &mut self.table_state,
        );
      }
      CurrentTab::System => {
        if let Some(info) = &self.sysinfo {
          let chunks = Layout::vertical([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
          ])
          .split(main_area[0]);

          let ram_used = (info.totalram - info.freeram) as f64 / info.totalram as f64 * 100.0;
          let swap_used = (info.totalswap - info.freeswap) as f64 / info.totalswap as f64 * 100.0;

          let memory_chunks =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(chunks[0]);

          frame.render_widget(
            Gauge::default()
              .block(Block::bordered().title("RAM Usage"))
              .gauge_style(Style::default().fg(Color::Green))
              .percent(ram_used as u16)
              .label(format!(
                "{}/{} ({:.1}%)",
                format_size((info.totalram - info.freeram) * info.mem_unit as u64, BINARY),
                format_size(info.totalram * info.mem_unit as u64, BINARY),
                ram_used
              )),
            memory_chunks[0],
          );

          frame.render_widget(
            Gauge::default()
              .block(Block::bordered().title("Swap Usage"))
              .gauge_style(Style::default().fg(Color::Yellow))
              .percent(swap_used as u16)
              .label(format!(
                "{}/{} ({:.1}%)",
                format_size((info.totalswap - info.freeswap) * info.mem_unit as u64, BINARY),
                format_size(info.totalswap * info.mem_unit as u64, BINARY),
                swap_used
              )),
            memory_chunks[1],
          );

          let datasets = vec![Dataset::default()
            .name("Load Average")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(&self.load_history)];

          let max_load = self.load_history.iter().map(|(_, y)| *y).fold(1.0, f64::max);
          frame.render_widget(
            Chart::new(datasets)
              .block(Block::bordered().title("Load Average History"))
              .x_axis(
                ratatui::widgets::Axis::default()
                  .bounds([0.0, 100.0])
                  .labels(vec![].into_iter().collect::<Vec<String>>()),
              )
              .y_axis(
                ratatui::widgets::Axis::default()
                  .title("Load")
                  .bounds([0.0, max_load * 1.1])
                  .labels(
                    (0..=5)
                      .map(|i| format!("{:.1}", (i as f64 * max_load / 5.0)))
                      .collect::<Vec<String>>(),
                  ),
              ),
            chunks[1],
          );

          let memory_datasets = vec![Dataset::default()
            .name("Memory Usage %")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Green))
            .data(&self.memory_history)];

          frame.render_widget(
            Chart::new(memory_datasets)
              .block(Block::bordered().title("Memory Usage History"))
              .x_axis(
                ratatui::widgets::Axis::default()
                  .bounds([0.0, 100.0])
                  .labels(vec![].into_iter().collect::<Vec<String>>()),
              )
              .y_axis(
                ratatui::widgets::Axis::default()
                  .title("Usage %")
                  .bounds([0.0, 100.0])
                  .labels(
                    vec!["0%", "25%", "50%", "75%", "100%"]
                      .into_iter()
                      .map(|s| s.to_string())
                      .collect::<Vec<String>>(),
                  ),
              ),
            chunks[2],
          );

          let load_averages = format!(
            "Load Averages: 1min: {:.2}, 5min: {:.2}, 15min: {:.2}",
            info.loads[0] as f64 / 65536.0,
            info.loads[1] as f64 / 65536.0,
            info.loads[2] as f64 / 65536.0,
          );

          let system_info = [
            format!("Uptime: {}", Self::format_uptime(info.uptime)),
            format!("Running Processes: {}", info.procs),
            load_averages,
            format!(
              "Total High Memory: {}",
              format_size(info.totalhigh * info.mem_unit as u64, BINARY)
            ),
            format!(
              "Free High Memory: {}",
              format_size(info.freehigh * info.mem_unit as u64, BINARY)
            ),
            format!(
              "Shared RAM: {}",
              format_size(info.sharedram * info.mem_unit as u64, BINARY)
            ),
            format!(
              "Buffer RAM: {}",
              format_size(info.bufferram * info.mem_unit as u64, BINARY)
            ),
          ];

          frame.render_widget(
            Paragraph::new(system_info.join("\n"))
              .block(Block::bordered().title("System Information"))
              .style(Style::default().fg(Color::White)),
            chunks[3],
          );
        } else {
          frame.render_widget(
            Paragraph::new("Loading system information...").block(Block::bordered().title("System")),
            main_area[0],
          );
        }
      }
      CurrentTab::Disk => {
        let chunks = Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).split(main_area[0]);

        let disk_data: Vec<(String, Vec<(f64, f64)>, Vec<(f64, f64)>)> = self
          .disk_history
          .iter()
          .map(|(device, history)| {
            let read_data: Vec<(f64, f64)> = history.iter().map(|(x, read, _)| (*x, *read)).collect();

            let write_data: Vec<(f64, f64)> = history.iter().map(|(x, _, write)| (*x, *write)).collect();

            (device.clone(), read_data, write_data)
          })
          .collect();

        let mut all_datasets = Vec::new();
        for (device, read_data, write_data) in &disk_data {
          all_datasets.push(
            Dataset::default()
              .name(format!("{} Read", device))
              .marker(Marker::Braille)
              .graph_type(GraphType::Line)
              .style(Style::default().fg(Color::Green))
              .data(read_data),
          );

          all_datasets.push(
            Dataset::default()
              .name(format!("{} Write", device))
              .marker(Marker::Braille)
              .graph_type(GraphType::Line)
              .style(Style::default().fg(Color::Red))
              .data(write_data),
          );
        }

        let max_rate = self
          .disk_history
          .values()
          .flat_map(|history| history.iter().flat_map(|(_, read, write)| vec![read, write]))
          .fold(1.0, |max, &rate| if rate > max { rate } else { max });

        frame.render_widget(
          Chart::new(all_datasets)
            .block(Block::bordered().title("Disk I/O Rates"))
            .x_axis(
              ratatui::widgets::Axis::default()
                .bounds([0.0, 100.0])
                .labels(vec![].into_iter().collect::<Vec<String>>()),
            )
            .y_axis(
              ratatui::widgets::Axis::default()
                .title("Rate")
                .bounds([0.0, max_rate * 1.1])
                .labels(
                  (0..=5)
                    .map(|i| pro::format_rate(i as f64 * max_rate / 5.0))
                    .collect::<Vec<String>>(),
                ),
            ),
          chunks[0],
        );

        let mut rates_text = Vec::new();
        for disk in &self.disk_stats {
          if let Some(history) = self.disk_history.get(&disk.device) {
            if let Some((_, read_rate, write_rate)) = history.last() {
              rates_text.push(format!(
                "{}: Read: {}/s, Write: {}/s",
                disk.device,
                pro::format_rate(*read_rate),
                pro::format_rate(*write_rate)
              ));
            }
          }
        }

        frame.render_widget(
          Paragraph::new(rates_text.join("\n"))
            .block(Block::bordered().title("Current I/O Rates"))
            .style(Style::default().fg(Color::White)),
          chunks[1],
        );
      }
      CurrentTab::Network => {
        let chunks = Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).split(main_area[0]);

        let network_data: Vec<(String, Vec<(f64, f64)>, Vec<(f64, f64)>)> = self
          .network_history
          .iter()
          .map(|(interface, history)| {
            let rx_data: Vec<(f64, f64)> = history.iter().map(|(x, rx, _)| (*x, *rx)).collect();

            let tx_data: Vec<(f64, f64)> = history.iter().map(|(x, _, tx)| (*x, *tx)).collect();

            (interface.clone(), rx_data, tx_data)
          })
          .collect();

        let mut all_datasets = Vec::new();
        for (interface, rx_data, tx_data) in &network_data {
          all_datasets.push(
            Dataset::default()
              .name(format!("{} RX", interface))
              .marker(Marker::Braille)
              .graph_type(GraphType::Line)
              .style(Style::default().fg(Color::Green))
              .data(rx_data),
          );

          all_datasets.push(
            Dataset::default()
              .name(format!("{} TX", interface))
              .marker(Marker::Braille)
              .graph_type(GraphType::Line)
              .style(Style::default().fg(Color::Red))
              .data(tx_data),
          );
        }

        let max_rate = self
          .network_history
          .values()
          .flat_map(|history| history.iter().flat_map(|(_, rx, tx)| vec![rx, tx]))
          .fold(1.0, |max, &rate| if rate > max { rate } else { max });

        frame.render_widget(
          Chart::new(all_datasets)
            .block(Block::bordered().title("Network Traffic"))
            .x_axis(
              ratatui::widgets::Axis::default()
                .bounds([0.0, 100.0])
                .labels(vec![].into_iter().collect::<Vec<String>>()),
            )
            .y_axis(
              ratatui::widgets::Axis::default()
                .title("Rate")
                .bounds([0.0, max_rate * 1.1])
                .labels(
                  (0..=5)
                    .map(|i| pro::format_rate(i as f64 * max_rate / 5.0))
                    .collect::<Vec<String>>(),
                ),
            ),
          chunks[0],
        );

        let mut rates_text = Vec::new();
        for net in &self.network_stats {
          if let Some(history) = self.network_history.get(&net.interface) {
            if let Some((_, rx_rate, tx_rate)) = history.last() {
              rates_text.push(format!(
                "{}: RX: {}/s, TX: {}/s",
                net.interface,
                pro::format_rate(*rx_rate),
                pro::format_rate(*tx_rate)
              ));
            }
          }
        }

        frame.render_widget(
          Paragraph::new(rates_text.join("\n"))
            .block(Block::bordered().title("Current Network Rates"))
            .style(Style::default().fg(Color::White)),
          chunks[1],
        );
      }
      CurrentTab::Cpu => {
        if self.cpu_usage.is_empty() {
          frame.render_widget(
            Paragraph::new("Loading CPU information...").block(Block::bordered().title("CPU")),
            main_area[0],
          );
          return;
        }
        let chunks = Layout::vertical([Constraint::Percentage(50), Constraint::Fill(1)]).split(main_area[0]);

        let datasets = vec![Dataset::default()
          .name("CPU Usage")
          .marker(Marker::Braille)
          .graph_type(GraphType::Line)
          .style(Style::default().fg(Color::Cyan))
          .data(&self.cpu_history)];

        frame.render_widget(
          Chart::new(datasets)
            .block(Block::bordered().title("CPU Usage History"))
            .x_axis(
              ratatui::widgets::Axis::default()
                .bounds([0.0, 100.0])
                .labels(vec![].into_iter().collect::<Vec<String>>()),
            )
            .y_axis(
              ratatui::widgets::Axis::default()
                .title("Usage %")
                .bounds([0.0, 100.0])
                .labels(
                  vec!["0%", "25%", "50%", "75%", "100%"]
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),
                ),
            ),
          chunks[0],
        );

        let num_cores = self.cpu_usage.len() - 1;
        let num_rows = (num_cores + 3) / 4;
        let gauge_constraints = vec![Constraint::Percentage(25); 4];

        let row_constraints = vec![Constraint::Percentage((100 / num_rows) as u16); num_rows];
        let core_rows = Layout::vertical(row_constraints).split(chunks[1]);

        for row in 0..num_rows {
          let cores_in_row = Layout::horizontal(gauge_constraints.clone()).split(core_rows[row]);

          for col in 0..4 {
            let core_idx = row * 4 + col + 1;
            if core_idx <= num_cores {
              if let Some(&usage) = self.cpu_usage.get(core_idx) {
                frame.render_widget(
                  Gauge::default()
                    .block(Block::bordered().title(format!("CPU {}", core_idx)))
                    .gauge_style(Style::default().fg(Color::Green))
                    .percent(usage as u16)
                    .label(format!("{:>5.1}%", usage)),
                  cores_in_row[col],
                );
              }
            }
          }
        }
      }
      _ => {}
    }

    let status_text = if self.status_message.is_some() {
      self.status_message.clone().unwrap_or_default()
    } else if self.current_tab == CurrentTab::Processes && self.mode == Mode::Search {
      self.pattern.clone()
    } else {
      let info = self.sysinfo.as_ref().unwrap();
      format!(
        "Processes: {}, CPU: {}, Free Ram: {}\t\t Press (?) for help",
        self.processes.len(),
        if self.cpu_usage.is_empty() {
          "Loading...".to_string()
        } else {
          format!("{:.1}%", self.cpu_usage[0])
        },
        format_size((info.totalram - info.freeram) * info.mem_unit as u64, BINARY),
      )
    };

    frame.render_widget(
      Paragraph::new(status_text)
        .style(if self.status_message.is_none() {
          Style::default()
        } else if self.status_message_error {
          Style::default().fg(Color::Red)
        } else {
          Style::default().fg(Color::Green)
        })
        .block(Block::default()),
      main_area[1],
    );
  }

  fn handle_key_event(&mut self, key_event: KeyEvent) {
    match key_event.code {
      KeyCode::Up => self.decrement_list(),
      KeyCode::Down => self.increment_list(),
      KeyCode::Left => self.prev_tab(),
      KeyCode::Right => self.next_tab(),
      KeyCode::Char('?') => {
        self.status_message = Some(
          "[s]ort by: [n]ame, [p]id, [u]ser, [m]em; [/] search; flip [a]scending; [G]oto bottom; [k]ill; [q]uit, [n/N]ice+/-"
            .to_string(),
        );
      }
      KeyCode::Char('G') => {
        self.table_state.select(Some(self.accessible_processes.len() - 1));
      }
      KeyCode::Char('n') => {
        if let Some(selection) = self.table_state.selected() {
          let sel = &self.accessible_processes[selection];
          pro::set_priority(sel.pid, sel.priority + 1, Some(&self.output_tx));
        }
      }
      KeyCode::Char('N') => {
        if let Some(selection) = self.table_state.selected() {
          let sel = &self.accessible_processes[selection];
          pro::set_priority(sel.pid, sel.priority - 1, Some(&self.output_tx));
        }
      }
      KeyCode::Char('k') => {
        if let Some(selection) = self.table_state.selected() {
          let pid = self.accessible_processes[selection].pid;
          pro::kill_process(pid, 9, Some(&self.output_tx));
        }
      }
      KeyCode::Char('a') => {
        self.ascending = !self.ascending;
      }
      _ => {}
    }
  }

  fn next_tab(&mut self) {
    self.current_tab =
      CurrentTab::try_from((self.current_tab as u8 + 1) % CurrentTab::Max as u8).expect("Failed to get next tab")
  }

  fn prev_tab(&mut self) {
    self.current_tab = if self.current_tab as u8 != 0 {
      CurrentTab::try_from((self.current_tab as u8 - 1) % CurrentTab::Max as u8).expect("Failed to get previous tab")
    } else {
      CurrentTab::try_from(CurrentTab::Max as u8 - 1).expect("Failed to get last tab")
    }
  }

  fn increment_list(&mut self) {
    self.table_state.select_next();
  }

  fn decrement_list(&mut self) {
    self.table_state.select_previous();
  }

  fn list_processes(&mut self) -> io::Result<()> {
    self.accessible_processes = list_processes(
      self.processes.clone(),
      self.from,
      self.nprocs,
      &self.sort_by,
      self.ascending,
      &self.filter_by,
      &self.pattern,
      self.exact_match,
    )?;
    Ok(())
  }
}

pub fn run() -> io::Result<()> {
  let mut terminal = ratatui::init();
  let app_result = App::new().run(&mut terminal);
  ratatui::restore();
  app_result
}
