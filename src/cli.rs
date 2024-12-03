use std::io;
use core::panic;
use getopts::Options;
use libc::{self, pid_t};
use std::fs;
use std::io::Write;
use std::process::exit;
use std::time::Duration;
use crate::pro;

pub fn make_opts() -> Options {
  let mut opts = Options::new();
  opts.optopt("r", "refresh_rate", "Stats refresh rate", "[NUM]");
  opts.optopt("n", "nprocs", "Max number of processes to show", "[NUM]");
  opts.optopt("k", "kill", "Kill process with signal", "[SIG]");
  opts.optopt("i", "iters", "Number of iterations", "[NUM]");
  opts.optflag("h", "help", "Print help message");
  opts.optopt("p", "priority", "Priority of process", "[PRIO]");
  opts.optopt("o", "output", "output file (logs)", "[FILE]");
  opts.optopt("", "pid", "pid of process", "[PID]");
  opts.optopt(
    "s",
    "sort_by",
    "How to sort the processes",
    "[name|pid|memory|priority|user|state|threads|vmsize|utime|stime]",
  );
  opts.optopt("f", "filter_by", "Filter by", "[name|user|ppid|state]");
  opts.optopt("", "pattern", "Pattern to filter by", "[PATTERN]");
  opts.optflag("e", "exact_match", "The pattern should be an exact match");
  opts.optflag("d", "descending", "Sort in descending order");
  opts.optopt("c", "cpu_affinity", "List of cpus", "[CPU]");
  opts.optflag("a", "all", "Execute on all output processes");
  opts.optflag("t", "tree", "print process tree");
  opts
}

pub fn read_opts() -> getopts::Matches {
  let args: Vec<String> = std::env::args().collect();
  let opts = make_opts();
  let matches = match opts.parse(&args[1..]) {
    Ok(m) => m,
    Err(f) => {
      panic!("{}", f);
    }
  };

  if matches.opt_present("h") {
    pro::print_usage(&args[0], opts);
    exit(0);
  }

  matches
}

pub fn run() -> io::Result<()> {
  let matches = read_opts();

  let pid_p = matches.opt_present("pid");
  let all_p = matches.opt_present("a");
  let refresh_rate = matches
    .opt_get_default::<u64>("r", 1)
    .expect("Invalid refresh rate value");

  let nprocs = matches
    .opt_get_default::<usize>("n", usize::MAX)
    .expect("Invalid nprocs value");

  let iterations = matches
    .opt_get_default::<u32>("i", 0)
    .expect("Invalid iterations value");

  let sort_by = matches
    .opt_get_default::<String>("s", "pid".to_string())
    .expect("Invalid sort_by value");

  let filter_by = matches
    .opt_get_default::<String>("f", "".to_string())
    .expect("Invalid filter_by value");

  let pattern = matches
    .opt_get_default::<String>("pattern", "".to_string())
    .expect("Invalid pattern value");

  let descending = matches.opt_present("d");

  let output_file = matches
    .opt_get_default::<String>("o", "/tmp/procstat.log".to_string())
    .expect("Invalid output file value");

  let mut current_iteration = 0;
  let mut log_file = fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(output_file)
    .expect("Failed to open log file");
  let exact_match = matches.opt_present("e");

  if pid_p || all_p {
    let mut pids: Vec<pid_t> = Vec::new();
    let pid = matches.opt_get_default::<pid_t>("pid", 0).expect("Invalid pid value");
    if all_p {
      pids = pro::list_processes(
        pro::read_processes().unwrap(),
        0,
        nprocs,
        &sort_by,
        !descending,
        &filter_by,
        &pattern,
        exact_match,
      )
      .unwrap()
      .iter()
      .map(|p| p.pid)
      .collect();
    } else {
      pids.push(pid);
    }
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
    return Ok(());
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

    return Ok(());
  }

  while iterations == 0 || current_iteration != iterations {
    let output = pro::show_stats(nprocs, &sort_by, descending, &filter_by, &pattern, exact_match);
    current_iteration += 1;
    // Clear screen and display all at once
    print!("{esc}[2J{esc}[1;1H{}", output, esc = 27 as char);
    std::io::stdout().flush().unwrap();

    std::thread::sleep(Duration::from_secs(refresh_rate));
    writeln!(log_file, "{}", output).expect("Failed to write to log file");
  }

  Ok(())
}
