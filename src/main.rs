use clap::{CommandFactory, Parser};
use tokimo_app_computer_use::cli::Cli;

#[cfg(target_os = "macos")]
fn bootstrap_macos() {
  // CGS / ScreenCaptureKit window capture asserts `did_initialize` when the
  // process hasn't loaded AppKit. CLI binaries don't have an NSApplication
  // event loop, so initialize the shared NSApplication (without entering the
  // run loop) so Core Graphics Services bootstraps correctly.
  use objc2_app_kit::NSApplication;
  unsafe {
    if let Some(mtm) = objc2_foundation::MainThreadMarker::new() {
      let _ = NSApplication::sharedApplication(mtm);
    }
  }
}

#[cfg(not(target_os = "macos"))]
fn bootstrap_macos() {}

fn main() {
  // When invoked by agent-browser as a spawned daemon/dashboard process,
  // short-circuit BEFORE clap parses (clap would reject the empty/foreign
  // argv it inherits from the parent). The agent_browser::run entrypoint
  // detects AGENT_BROWSER_DAEMON / AGENT_BROWSER_DASHBOARD itself.
  if std::env::var("AGENT_BROWSER_DAEMON").is_ok() || std::env::var("AGENT_BROWSER_DASHBOARD").is_ok() {
    agent_browser::run(std::env::args().collect());
    return;
  }

  bootstrap_macos();
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
