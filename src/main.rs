use clap::{CommandFactory, Parser};
use tokimo_app_computer_use::cli::Cli;

fn main() {
  let cli = Cli::parse();

  match &cli.command {
    None => {
      // No subcommand — print help
      let mut cmd = Cli::command();
      let _ = cmd.print_help();
    }
    Some(_) => {
      // CLI mode
      if let Err(e) = tokimo_app_computer_use::cli::run_cli(cli) {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
      }
    }
  }
}
