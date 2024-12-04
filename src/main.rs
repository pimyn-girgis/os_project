use iced::widget::{button, column, container, row, text, text_input, Scrollable, Space};
use iced::{Alignment, Element, Length, Application, Command, Settings};
use libc::pid_t;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

mod pro;

// Main application state
struct ProcessManagerApp {
    processes: Vec<pro::ProcessInfo>,
    filtered_processes: Vec<pro::ProcessInfo>,
    sort_column: String,
    sort_ascending: bool,
    search_input: String,
    selected_process_pid: Option<pid_t>,
    show_help: bool,
    receiver: Arc<Mutex<mpsc::Receiver<Message>>>,
    cpu_usages: Vec<f64>,
}

#[derive(Debug, Clone)]
enum Message {
    SortByName,
    SortByPid,
    SortByUser,
    SortByPriority,
    SortByState,
    SortByThreads,
    SortByUserTime,
    SortBySystemTime,
    SortByVMSize,
    SortByMemory,
    SearchInputChanged(String),
    SearchProcess,
    NiceProcess,
    KillProcess,
    Quit,
    RefreshProcesses,
    ProcessSelected(pid_t),
    Help,
    CloseHelp,
    Tick,
}

impl Application for ProcessManagerApp {
    type Message = Message;
    type Executor = iced::executor::Default;
    type Theme = iced::Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let processes = pro::read_processes().unwrap_or_default();
        let cpu_usages = pro::get_cpu_usage().unwrap_or_default();

        let (sender, receiver) = mpsc::channel();

        // Wrap the receiver in Arc<Mutex<_>> for shared access
        let receiver = Arc::new(Mutex::new(receiver));

        // Clone the Arc to move into the thread
        let thread_receiver = receiver.clone();

        // Spawn a background thread to send Tick messages every 2 seconds
        thread::spawn(move || {
            loop {
                unsafe {
                    libc::sleep(2); // Sleep for 2 seconds
                }
                if sender.send(Message::Tick).is_err() {
                    // If the receiver has been dropped, exit the loop
                    break;
                }
            }
        });

        let mut app = Self {
            processes: processes.clone(),
            filtered_processes: processes,
            sort_column: "pid".to_string(),
            sort_ascending: true,
            search_input: String::new(),
            selected_process_pid: None,
            show_help: false,
            receiver: thread_receiver,
            cpu_usages,
        };
        app.apply_filters_and_sorting();
        let command = Self::listen_for_tick(Arc::clone(&receiver));
        (app, command)
    }

    fn title(&self) -> String {
        "Amr El-Kady Pro".to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SortByName => {
                if self.sort_column == "name" {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = "name".to_string();
                    self.sort_ascending = true;
                }
                self.apply_filters_and_sorting();
            }
            Message::SortByPid => {
                if self.sort_column == "pid" {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = "pid".to_string();
                    self.sort_ascending = true;
                }
                self.apply_filters_and_sorting();
            }
            Message::SortByUser => {
                if self.sort_column == "user" {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = "user".to_string();
                    self.sort_ascending = true;
                }
                self.apply_filters_and_sorting();
            }
            Message::SortByPriority => {
                if self.sort_column == "priority" {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = "priority".to_string();
                    self.sort_ascending = true;
                }
                self.apply_filters_and_sorting();
            }
            Message::SortByMemory => {
                if self.sort_column == "memory" {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = "memory".to_string();
                    self.sort_ascending = true;
                }
                self.apply_filters_and_sorting();
            }
            Message::SortByVMSize => {
                if self.sort_column == "vmsize" {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = "vmsize".to_string();
                    self.sort_ascending = true;
                }
                self.apply_filters_and_sorting();
            }
            Message::SortByState => {
                if self.sort_column == "state" {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = "state".to_string();
                    self.sort_ascending = true;
                }
                self.apply_filters_and_sorting();
            }
            Message::SortByThreads => {
                if self.sort_column == "threads" {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = "threads".to_string();
                    self.sort_ascending = true;
                }
                self.apply_filters_and_sorting();
            }
            Message::SortByUserTime => {
                if self.sort_column == "utime" {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = "utime".to_string();
                    self.sort_ascending = true;
                }
                self.apply_filters_and_sorting();
            }
            Message::SortBySystemTime => {
                if self.sort_column == "stime" {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = "stime".to_string();
                    self.sort_ascending = true;
                }
                self.apply_filters_and_sorting();
            }
            Message::SearchInputChanged(input) => {
                self.search_input = input;
                self.apply_filters_and_sorting();
            }
            Message::SearchProcess => {
                self.apply_filters_and_sorting();
            }
            Message::NiceProcess => {
                if let Some(pid) = self.selected_process_pid {
                    pro::set_priority(pid, 10);
                    println!("Changed priority of process {}", pid);
                }
            }
            Message::KillProcess => {
                if let Some(pid) = self.selected_process_pid {
                    pro::kill_process(pid, libc::SIGTERM);
                    println!("Sent SIGTERM to process {}", pid);
                }
            }
            Message::RefreshProcesses => {
                if let Ok(new_processes) = pro::read_processes() {
                    self.processes = new_processes;
                    self.apply_filters_and_sorting();
                }
            }
            Message::Quit => {
                std::process::exit(0);
            }
            Message::ProcessSelected(pid) => {
                self.selected_process_pid = Some(pid);
            }
            Message::Help => {
                self.show_help = true;
            }
            Message::CloseHelp => {
                self.show_help = false;
            }
            Message::Tick => {
              // Periodic update
              if let Ok(new_processes) = pro::read_processes() {
                  self.processes = new_processes;
                  self.apply_filters_and_sorting();
              }
              if let Ok(new_cpu_usages) = pro::get_cpu_usage() {
                self.cpu_usages = new_cpu_usages;
              }
              // Schedule the next Tick
              return Self::listen_for_tick(Arc::clone(&self.receiver));
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        if self.show_help {
            // Display help content
            let content = column![
                text("Help").size(30),
                text("This is a Linux Process Manager application.").size(20),
                text("Use the buttons to sort, filter, and manage processes.").size(20),
                text("Select a process by clicking on it in the list.").size(20),
                text("Then you can kill or nice the selected process.").size(20),
                text("Buttons:").size(20),
                text("- Help: Show this help message.").size(16),
                text("- Nice: Change the priority of the selected process.").size(16),
                text("- Kill: Terminate the selected process.").size(16),
                text("- Refresh: Manually refresh the process list.").size(16),
                text("- Quit: Exit the application.").size(16),
                button("Close").on_press(Message::CloseHelp),
            ]
            .padding(20)
            .spacing(10);

            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .into()
        } else {
            let system_info = self.render_system_info();
            let cpu_graph = self.render_cpu_usage_graph();
            let process_table = self.render_process_table();
            let action_buttons = self.render_action_buttons();

            let top_row = row![
                system_info,
                cpu_graph
            ]
            .spacing(10);

            let content = column![
                top_row,
                process_table,
                action_buttons
            ]
            .spacing(10)
            .padding(10);

            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .into()
        }
    }
}

impl ProcessManagerApp {
    fn listen_for_tick(receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Command<Message> {
      // Spawn a blocking task to wait for Tick
      Command::perform(async move {
          // Run recv in a blocking thread
          let receiver = Arc::clone(&receiver);
          let msg = thread::spawn(move || {
              let lock = receiver.lock().unwrap();
              lock.recv().unwrap()
          })
          .join()
          .unwrap();
          msg
      }, |msg| msg)
    }
    fn apply_filters_and_sorting(&mut self) {
        // Filter processes according to self.search_input
        if self.search_input.is_empty() {
            self.filtered_processes = self.processes.clone();
        } else {
            let pattern = self.search_input.to_lowercase();
            self.filtered_processes = self.processes.iter()
                .filter(|p| p.name.to_lowercase().contains(&pattern) || p.user.to_lowercase().contains(&pattern))
                .cloned()
                .collect();
        }

        // Sort processes according to self.sort_column and self.sort_ascending
        match self.sort_column.as_str() {
            "name" => self.filtered_processes.sort_by_key(|p| p.name.clone()),
            "pid" => self.filtered_processes.sort_by_key(|p| p.pid),
            "user" => self.filtered_processes.sort_by_key(|p| p.user.clone()),
            "priority" => self.filtered_processes.sort_by_key(|p| p.priority),
            "memory" => self.filtered_processes.sort_by_key(|p| p.memory),
            "vmsize" => self.filtered_processes.sort_by_key(|p| p.virtual_memory),
            "state" => self.filtered_processes.sort_by_key(|p| p.state),
            "threads" => self.filtered_processes.sort_by_key(|p| p.thread_count),
            "utime" => self.filtered_processes.sort_by_key(|p| p.user_time),
            "stime" => self.filtered_processes.sort_by_key(|p| p.system_time),
            _ => {},
        }

        if !self.sort_ascending {
            self.filtered_processes.reverse();
        }
    }

    fn render_system_info(&self) -> Element<Message> {
        let system_info = pro::get_sysinfo();
        let cpu_usages = pro::get_cpu_usage().unwrap_or_default();
        let mem_unit = 1_000_000 / system_info.mem_unit as u64;

        let total_cpu = if let Some(&usage) = cpu_usages.first() {
          usage
        } else {
            0.0
        };

        let info_text = column![
            text(format!("Total RAM: {} MB", system_info.totalram / mem_unit)),
            text(format!("Shared RAM: {} MB", system_info.sharedram / mem_unit)),
            text(format!("Free RAM: {} MB", system_info.freeram / mem_unit)),
            text(format!("Buffer RAM: {} MB", system_info.bufferram / mem_unit)),
            text(format!("Total Swap: {} MB", system_info.totalswap / mem_unit)),
            text(format!("Free Swap: {} MB", system_info.freeswap / mem_unit)),
            text(format!("Uptime: {} seconds", system_info.uptime)),
            text(format!("Loads: {:?}", system_info.loads)),
            text("CPU Usage:"),
            text(format!("Total: {:.2}%", total_cpu)),
        ]
        .spacing(5);

        container(info_text)
            .padding(10)
            .into()
    }

    fn render_process_table(&self) -> Element<Message> {
        let processes_list = self.filtered_processes.iter().map(|process| {
            let row_content = row![
                text(&process.user).width(Length::FillPortion(1)),
                text(&process.pid.to_string()).width(Length::FillPortion(1)),
                text(&(process.memory / 1000).to_string()).width(Length::FillPortion(1)),
                text(&process.priority.to_string()).width(Length::FillPortion(1)),
                text(&process.state.to_string()).width(Length::FillPortion(1)),
                text(&process.thread_count.to_string()).width(Length::FillPortion(1)),
                text(&(process.virtual_memory / 1000).to_string()).width(Length::FillPortion(1)),
                text(&process.user_time.to_string()).width(Length::FillPortion(1)),
                text(&process.system_time.to_string()).width(Length::FillPortion(1)),
                text(&process.name).width(Length::FillPortion(3)),
            ]
            .spacing(15)
            .padding(5)
            .align_items(Alignment::Center);

            let row_element: Element<_> = if Some(process.pid) == self.selected_process_pid {
                container(row_content)
                    .style(iced::theme::Container::Custom(Box::new(SelectedRowStyle)))
                    .into()
            } else {
                button(row_content)
                    .on_press(Message::ProcessSelected(process.pid))
                    .style(iced::theme::Button::Custom(Box::new(RegularRowStyle)))
                    .into()
            };
            row_element
        }).collect::<Vec<Element<Message>>>();

        let header = row![
            button("User").on_press(Message::SortByUser).width(Length::FillPortion(1)),
            button("PID").on_press(Message::SortByPid).width(Length::FillPortion(1)),
            button("Mem").on_press(Message::SortByMemory).width(Length::FillPortion(1)),
            button("Priority").on_press(Message::SortByPriority).width(Length::FillPortion(1)),
            button("State").on_press(Message::SortByState).width(Length::FillPortion(1)),
            button("Threads").on_press(Message::SortByThreads).width(Length::FillPortion(1)),
            button("V_MEM)").on_press(Message::SortByVMSize).width(Length::FillPortion(1)),
            button("U_time").on_press(Message::SortByUserTime).width(Length::FillPortion(1)),
            button("S_time").on_press(Message::SortBySystemTime).width(Length::FillPortion(1)),
            button("Name").on_press(Message::SortByName).width(Length::FillPortion(3)),
        ]
        .spacing(15)
        .align_items(Alignment::Center)
        .padding(5);

        let scrollable_processes = Scrollable::new(column(processes_list).spacing(5))
            .width(Length::Fill)
            .height(Length::Fill);

        column![
            header,
            Space::with_height(Length::Fixed(10.0)),
            scrollable_processes
        ]
        .padding(10)
        .into()
    }

    fn render_action_buttons(&self) -> Element<Message> {
        let buttons = row![
            button("Help").on_press(Message::Help),
            text_input("Search", &self.search_input)
                .on_input(Message::SearchInputChanged)
                .on_submit(Message::SearchProcess)
                .padding(5)
                .width(Length::Fixed(200.0)),
            button("Nice").on_press(Message::NiceProcess),
            button("Kill").on_press(Message::KillProcess),
            button("Refresh").on_press(Message::RefreshProcesses),
            button("Quit").on_press(Message::Quit)
        ]
        .spacing(10)
        .align_items(Alignment::Center);

        container(buttons)
            .padding(10)
            .center_x()
            .into()
    }

    fn render_cpu_usage_graph(&self) -> Element<Message> {
      // Create a bar-like representation of CPU usage
      let cpu_bars = self.cpu_usages.iter().enumerate().map(|(i, &usage)| {
          let bar_height = (usage / 100.0) * 150.0; // Maximum height of 200 pixels
          let bar_color = if i == 0 {
              iced::Color::from_rgb(0.2, 0.6, 0.8) // Total CPU in a different color
          } else {
              iced::Color::from_rgb(0.4, 0.6, 1.0)
          };
  
          // Use owned Strings instead of references
          let label = if i == 0 {
              "Total".to_string()
          } else {
              format!("Core {}", i-1)
          };
  
          column![
              container(Space::new(Length::Fixed(20.0), Length::Fixed((150.0 - bar_height) as f32)))
                  .style(iced::theme::Container::Custom(Box::new(EmptyCpuBarStyle))),
              container(Space::new(Length::Fixed(20.0), Length::Fixed(bar_height as f32)))
                  .style(iced::theme::Container::Custom(Box::new(CpuBarStyle { color: bar_color }))),
              text(label)
                  .horizontal_alignment(iced::alignment::Horizontal::Center)
                  .size(12),
          ]
          .width(Length::Fixed(20.0))
          .spacing(2)
          .into()
      }).collect::<Vec<Element<Message>>>();
  
      let cpu_graph_container = row(cpu_bars)
          .spacing(10)
          .align_items(Alignment::End);
  
      let cpu_graph = column![
        text("CPU Usage").size(16).horizontal_alignment(iced::alignment::Horizontal::Center),
        cpu_graph_container
      ]
      .spacing(10)
      .padding(10);
  
      container(cpu_graph)
          .style(iced::theme::Container::Box) // Add a border or background if desired
          .into()
    }
}

struct CpuBarStyle {
  color: iced::Color,
}

impl iced::widget::container::StyleSheet for CpuBarStyle {
  type Style = iced::Theme;

  fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
      iced::widget::container::Appearance {
          background: Some(self.color.into()),
          border: Default::default(),
          shadow: Default::default(),
          text_color: None,
      }
  }
}

struct EmptyCpuBarStyle;

impl iced::widget::container::StyleSheet for EmptyCpuBarStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(iced::Color::from_rgb(0.9, 0.9, 0.9).into()),
            border: Default::default(),
            shadow: Default::default(),
            text_color: None,
        }
    }
}

// Custom styles for selected and regular rows
struct SelectedRowStyle;

impl iced::widget::container::StyleSheet for SelectedRowStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(iced::Color::from_rgb(0.9, 0.9, 1.0).into()),
            border: Default::default(),
            shadow: Default::default(),
            text_color: None,
        }
    }
}

struct RegularRowStyle;

impl iced::widget::button::StyleSheet for RegularRowStyle {
    type Style = iced::Theme;

    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(iced::Color::from_rgb(1.0, 1.0, 1.0).into()),
            border: Default::default(),
            shadow_offset: Default::default(), // Added this line
            shadow: Default::default(),        // Added this line
            text_color: iced::Color::BLACK,
        }
    }

    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(iced::Color::from_rgb(0.95, 0.95, 0.95).into()),
            border: Default::default(),
            shadow_offset: Default::default(),
            shadow: Default::default(),
            text_color: iced::Color::BLACK,
        }
    }
}


// Add main function
fn main() -> iced::Result {
  ProcessManagerApp::run(Settings::default())
}