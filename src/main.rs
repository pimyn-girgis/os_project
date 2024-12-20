use std::env;
use std::io;
pub mod pro;
mod cli;
mod tui;
mod icegui;

fn main() -> io::Result<()> {
  if env::args().len() > 1 {
    cli::run()?
  } else {
    tui::run()?
  }
  Ok(())
}
