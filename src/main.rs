use iced::widget::{
  button, column, container, row, 
  text, text_input, 
};
use iced::{Alignment, Element, Length, Application, Command, Settings};
use iced::widget::Scrollable;

mod pro;

// Main application state
struct ProcessManagerApp {
  processes: Vec<pro::ProcessInfo>,
  sort_column: String,
  sort_ascending: bool,
  search_input: String,
  selected_process_pid: Option<i32>,
}

// Define messages for interaction
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
  ShowProcessTree,
  NiceProcess,
  KillProcess,
  Quit,
  RefreshProcesses,
  Tick,
}

impl Application for ProcessManagerApp {
  type Message = Message;
  type Executor = iced::executor::Default;
  type Theme = iced::Theme;
  type Flags = ();

  fn new(_flags: ()) -> (Self, Command<Message>) {
      let processes = pro::read_processes().unwrap_or_default().into_iter().take(50).collect::<Vec<_>>();
      let app = Self {
          processes,
          sort_column: "pid".to_string(),
          sort_ascending: true,
          search_input: String::new(),
          selected_process_pid: None,
      };
      (app, Command::none())
  }

  fn title(&self) -> String {
      "Linux Process Manager".to_string()
  }

  fn update(&mut self, message: Message) -> Command<Message> {
      match message {
          Message::SortByName => {
              self.processes.sort_by(|a, b| a.name.cmp(&b.name));
          }
          Message::SortByPid => {
              self.processes.sort_by_key(|p| p.pid);
          }
          Message::SortByUser => {
            self.processes.sort_by_key(|p| p.user.clone());
          }
          Message::SortByPriority => {
            self.processes.sort_by_key(|p| p.priority);
          }
          Message::SortByMemory => {
            self.processes.sort_by_key(|p| p.memory);
          }
          Message::SortByVMSize => {
            self.processes.sort_by_key(|p| p.virtual_memory);
          }
          Message::SortByState => {
            self.processes.sort_by_key(|p| p.state);
          }
          Message::SortByThreads => {
            self.processes.sort_by_key(|p| p.thread_count);
          }
          Message::SortByUserTime => {
            self.processes.sort_by_key(|p| p.user_time);
          }
          Message::SortBySystemTime => {
            self.processes.sort_by_key(|p| p.system_time);
          }
          Message::SearchInputChanged(input) => {
              self.search_input = input;
          }
          Message::SearchProcess => {
              // Placeholder for search logic
          }
          Message::ShowProcessTree => {
              // TODO: Implement process tree view
          }
          Message::NiceProcess => {
              if let Some(pid) = self.selected_process_pid {
                  let _ = pro::set_priority(pid, 10);
              }
          }
          Message::KillProcess => {
              if let Some(pid) = self.selected_process_pid {
                  let _ = pro::kill_process(pid, libc::SIGTERM);
              }
          }
          Message::RefreshProcesses => {
              if let Ok(new_processes) = pro::read_processes() {
                  self.processes = new_processes;
              }
          }
          Message::Quit => {
              return Command::none();
          }
          Message::Tick => {
              return Command::batch(vec![
                  Command::perform(async { 
                      pro::read_processes().ok() 
                  }, |processes| Message::RefreshProcesses)
              ]);
          }
      }
      Command::none()
  }

  fn view(&self) -> Element<Message> {
      let system_info = self.render_system_info();
      let process_table = self.render_process_table();
      let action_buttons = self.render_action_buttons();
      
      let content = column![
          system_info,
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

impl ProcessManagerApp {
  fn render_system_info(&self) -> Element<Message> {
      let system_info = pro::get_sysinfo();
      let cpu_usages = pro::get_cpu_usage().unwrap_or_default();
      let mem_unit = 1_000_000 / system_info.mem_unit as u64;

      let info_text = column![
          text(format!("Total RAM: {}", system_info.totalram / mem_unit)),
          text(format!("Shared RAM: {}", system_info.sharedram / mem_unit)),
          text(format!("Free RAM: {}", system_info.freeram / mem_unit)),
          text(format!("Buffer RAM: {}", system_info.bufferram / mem_unit)),
          text(format!("Total Swap: {}", system_info.totalswap / mem_unit)),
          text(format!("Free Swap: {}", system_info.freeswap / mem_unit)),
          text(format!("Uptime: {}", system_info.uptime)),
          text(format!("Loads: {:?}", system_info.loads)),
          text("CPU Usage:"),
          text(format!("Total: {:.2}%", cpu_usages.first().unwrap_or(&0.0))),
      ]
      .spacing(5);

      container(info_text)
          .padding(10)
          .into()
  }

  fn render_process_table(&self) -> Element<Message> {
    let processes_list = self.processes.iter().map(|process| 
        row![
            text(&process.user),
            text(&process.pid.to_string()),
            text(&process.memory),
            text(&process.priority),
            text(&process.state),
            text(&process.thread_count),
            text(&process.virtual_memory),
            text(&process.user_time),
            text(&process.system_time),
            text(&process.name),
        ]
        .spacing(35)
        .padding(10)
        .into()
    ).collect::<Vec<Element<Message>>>();

    let header = row![
        button("User").on_press(Message::SortByUser),
        button("PID").on_press(Message::SortByPid),
        button("Memory(MB)").on_press(Message::SortByMemory),
        button("Priority").on_press(Message::SortByPriority),
        button("State").on_press(Message::SortByState),
        button("Threads").on_press(Message::SortByThreads),
        button("VIRT_MEM(MB)").on_press(Message::SortByVMSize),
        button("U_time").on_press(Message::SortByUserTime),
        button("S_time").on_press(Message::SortBySystemTime),
        button("Name").on_press(Message::SortByName),
    ]
    .spacing(15)
    .align_items(Alignment::Center);

    let scrollable_processes = Scrollable::new(column(processes_list).spacing(5))
        .width(Length::Fill)
        .height(Length::Fill);

    column![
        header,
        scrollable_processes
    ]
    .padding(10)
    .into()
  }

  fn render_action_buttons(&self) -> Element<Message> {
      let buttons = row![
          button("Help").on_press(Message::Quit),
          button("Tree").on_press(Message::ShowProcessTree),
          text_input("Search", &self.search_input)
              .on_input(Message::SearchInputChanged)
              .on_submit(Message::SearchProcess),
          button("Nice").on_press(Message::NiceProcess),
          button("Kill").on_press(Message::KillProcess),
          button("Quit").on_press(Message::Quit)
      ]
      .spacing(10)
      .align_items(Alignment::Center);

      container(buttons)
          .padding(10)
          .center_x()
          .into()
  }
}

// Add main function
fn main() -> iced::Result {
  ProcessManagerApp::run(Settings::default())
}