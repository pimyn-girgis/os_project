use iced::widget::{button, column, container, row, text, text_input, Scrollable, Space};
use iced::{Alignment, Element, Length, Application, Command, Settings};
use libc::pid_t;

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
    ProcessSelected(pid_t),
    Help,
    CloseHelp,
}

impl Application for ProcessManagerApp {
    type Message = Message;
    type Executor = iced::executor::Default;
    type Theme = iced::Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let processes = pro::read_processes().unwrap_or_default();
        let mut app = Self {
            processes: processes.clone(),
            filtered_processes: processes,
            sort_column: "pid".to_string(),
            sort_ascending: true,
            search_input: String::new(),
            selected_process_pid: None,
            show_help: false,
        };
        app.apply_filters_and_sorting();
        (app, Command::none())
    }

    fn title(&self) -> String {
        "Amr ElKady Pro".to_string()
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
            Message::ShowProcessTree => {
                // TODO: Implement process tree view
            }
            Message::NiceProcess => {
                if let Some(pid) = self.selected_process_pid {
                    pro::set_priority(pid, 10);
                }
            }
            Message::KillProcess => {
                if let Some(pid) = self.selected_process_pid {
                    pro::kill_process(pid, libc::SIGTERM);
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
            Message::Tick => {
                if let Ok(new_processes) = pro::read_processes() {
                    self.processes = new_processes;
                    self.apply_filters_and_sorting();
                }
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
}

impl ProcessManagerApp {
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
            text(format!("Total: {:.2}%", cpu_usages.first().unwrap_or(&0.0))),
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
                    .into()
            } else {
                button(row_content)
                    .on_press(Message::ProcessSelected(process.pid))
                    .into()
            };
            row_element
        }).collect::<Vec<Element<Message>>>();

        let header = row![
            button("User").on_press(Message::SortByUser).width(Length::FillPortion(1)),
            button("PID").on_press(Message::SortByPid).width(Length::FillPortion(1)),
            button("Memory(MB)").on_press(Message::SortByMemory).width(Length::FillPortion(1)),
            button("Priority").on_press(Message::SortByPriority).width(Length::FillPortion(1)),
            button("State").on_press(Message::SortByState).width(Length::FillPortion(1)),
            button("Threads").on_press(Message::SortByThreads).width(Length::FillPortion(1)),
            button("VIRT_MEM(MB)").on_press(Message::SortByVMSize).width(Length::FillPortion(1)),
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
            button("Tree").on_press(Message::ShowProcessTree),
            text_input("Search", &self.search_input)
                .on_input(Message::SearchInputChanged)
                .on_submit(Message::SearchProcess)
                .padding(5)
                .width(Length::Fixed(200.0)),
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