// main.rs

use iced::{
  executor, Application, Command, Element, Length, Settings, Theme,
};

use iced::widget::{Button, Column, Row, Scrollable, Text};

mod pro;
use pro::{
  build_tree, get_cpu_usage, get_priority, get_sysinfo, kill_process, read_process_info,
  read_processes, set_priority, ProcessInfo,
};

#[derive(Debug, Clone)]
enum Message {
  ProcessSelected(usize), // index in the processes vector
  HelpPressed,
  TreePressed,
  SearchPressed,
  NicePressed,
  KillPressed,
  QuitPressed,
}

struct ProcessManager {
  processes: Vec<ProcessInfo>,
  selected_process: Option<usize>, // index of the selected process
}

impl Application for ProcessManager {
  type Message = Message;
  type Executor = executor::Default;
  type Flags = ();
  type Theme = Theme;

  fn new(_flags: ()) -> (ProcessManager, Command<Message>) {
      let processes = read_processes().unwrap_or_default();
      (
          ProcessManager {
              processes,
              selected_process: None,
          },
          Command::none(),
      )
  }

  fn title(&self) -> String {
      String::from("Process Manager")
  }

  fn update(&mut self, message: Message) -> Command<Message> {
      match message {
          Message::ProcessSelected(index) => {
              self.selected_process = Some(index);
          }
          Message::HelpPressed => {
              // Display help information
              println!("Help: This is a process manager. You can select processes and perform actions.");
          }
          Message::TreePressed => {
              // Display process tree
              let tree = build_tree(&self.processes, 1); // Assuming PID 1 is the root
              tree.print(0);
          }
          Message::SearchPressed => {
              // Reload processes (you can implement a search dialog here)
              self.processes = read_processes().unwrap_or_default();
          }
          Message::NicePressed => {
              if let Some(index) = self.selected_process {
                  let pid = self.processes[index].pid;
                  // Increase niceness (reduce priority)
                  let current_priority = get_priority(pid);
                  set_priority(pid, current_priority + 1);
                  // Update process info
                  if let Ok(updated_process) = read_process_info(pid) {
                      self.processes[index] = updated_process;
                  } else {
                      // Process may have terminated
                      self.processes.remove(index);
                      self.selected_process = None;
                  }
              }
          }
          Message::KillPressed => {
              if let Some(index) = self.selected_process {
                  let pid = self.processes[index].pid;
                  kill_process(pid, 9); // Send SIGKILL
                  // Remove the process from the list
                  self.processes.remove(index);
                  self.selected_process = None;
              }
          }
          Message::QuitPressed => {
              // Exit the application
              std::process::exit(0);
          }
      }
      Command::none()
  }

  fn view(&self) -> Element<Message> {
      // Get system info
      let system_info = get_sysinfo();
      let mem_unit = 1_000_000 / system_info.mem_unit as u64;

      let mut stats_column = Column::new().spacing(5);

      stats_column = stats_column
          .push(Text::new(format!(
              "totalram: {}",
              system_info.totalram / mem_unit
          )))
          .push(Text::new(format!(
              "sharedram: {}",
              system_info.sharedram / mem_unit
          )))
          .push(Text::new(format!(
              "freeram: {}",
              system_info.freeram / mem_unit
          )))
          .push(Text::new(format!(
              "bufferram: {}",
              system_info.bufferram / mem_unit
          )))
          .push(Text::new(format!(
              "totalswap: {}",
              system_info.totalswap / mem_unit
          )))
          .push(Text::new(format!(
              "freeswap: {}",
              system_info.freeswap / mem_unit
          )))
          .push(Text::new(format!("uptime: {}", system_info.uptime)))
          .push(Text::new(format!("loads: {:?}", system_info.loads)));

      match get_cpu_usage() {
          Ok(cpu_usage) => {
              stats_column = stats_column.push(Text::new("CPU Usage:"));
              for (i, usage) in cpu_usage.iter().enumerate() {
                  if i == 0 {
                      stats_column =
                          stats_column.push(Text::new(format!("Total CPU: {:.2}%", usage)));
                  } else {
                      stats_column =
                          stats_column.push(Text::new(format!("Core {}: {:.2}%", i, usage)));
                  }
              }
          }
          Err(e) => {
              stats_column =
                  stats_column.push(Text::new(format!("Error retrieving CPU usage: {}", e)));
          }
      }

      let mut process_list = Column::new();

      // Header row
      process_list = process_list.push(
          Row::new()
              .push(Text::new("UID").width(Length::Fixed(50.0)))
              .push(Text::new("PID").width(Length::Fixed(50.0)))
              .push(Text::new("PPID").width(Length::Fixed(50.0)))
              .push(Text::new("STATE").width(Length::Fixed(50.0)))
              .push(Text::new("MEM(MB)").width(Length::Fixed(70.0)))
              .push(Text::new("THREADS").width(Length::Fixed(70.0)))
              .push(Text::new("VIRT_MEM(MB)").width(Length::Fixed(100.0)))
              .push(Text::new("USER_TIME").width(Length::Fixed(80.0)))
              .push(Text::new("SYS_TIME").width(Length::Fixed(80.0)))
              .push(Text::new("Priority").width(Length::Fixed(70.0)))
              .push(Text::new("Name").width(Length::Fixed(150.0))),
      );

      for (index, process) in self.processes.iter().enumerate() {
          let row = Row::new()
              .push(Text::new(&process.user).width(Length::Fixed(50.0)))
              .push(
                  Text::new(process.pid.to_string()).width(Length::Fixed(50.0)),
              )
              .push(
                  Text::new(process.ppid.to_string()).width(Length::Fixed(50.0)),
              )
              .push(
                  Text::new(process.state.to_string()).width(Length::Fixed(50.0)),
              )
              .push(
                  Text::new((process.memory / 1000).to_string()).width(Length::Fixed(70.0)),
              )
              .push(
                  Text::new(process.thread_count.to_string()).width(Length::Fixed(70.0)),
              )
              .push(
                  Text::new((process.virtual_memory / 1000).to_string())
                      .width(Length::Fixed(100.0)),
              )
              .push(
                  Text::new(process.user_time.to_string()).width(Length::Fixed(80.0)),
              )
              .push(
                  Text::new(process.system_time.to_string()).width(Length::Fixed(80.0)),
              )
              .push(
                  Text::new(process.priority.to_string()).width(Length::Fixed(70.0)),
              )
              .push(Text::new(&process.name).width(Length::Fixed(150.0)));

          let row_button = Button::new(row)
              .on_press(Message::ProcessSelected(index));

          process_list = process_list.push(row_button);
      }

      // Adjusted code here
      let scrollable = Scrollable::new(process_list)
          .height(Length::Fill)
          .width(Length::Fill);

      // Buttons
      let buttons = Row::new()
          .spacing(10)
          .padding(10)
          .push(
              Button::new(Text::new("Help")).on_press(Message::HelpPressed),
          )
          .push(
              Button::new(Text::new("Tree")).on_press(Message::TreePressed),
          )
          .push(
              Button::new(Text::new("Search")).on_press(Message::SearchPressed),
          )
          .push(
              Button::new(Text::new("Nice")).on_press(Message::NicePressed),
          )
          .push(
              Button::new(Text::new("Kill")).on_press(Message::KillPressed),
          )
          .push(
              Button::new(Text::new("Quit")).on_press(Message::QuitPressed),
          );

      let content = Column::new()
          .push(stats_column)
          .push(scrollable)
          .push(buttons)
          .spacing(10)
          .padding(10);

      content.into()
  }
}

// fn main() {
//   ProcessManager::run(Settings::default());
// }