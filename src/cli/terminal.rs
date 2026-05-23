use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum TermAction {
  Ps { #[arg(trailing_var_arg = true)] command: Vec<String> },
  Cmd { #[arg(trailing_var_arg = true)] command: Vec<String> },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: TermAction) -> Result<()> {
  match action {
    TermAction::Ps { command } => {
      let r = executor.call("terminal.execute", json!({"shell_type": "ps", "command": command.join(" ")}))?;
      print_terminal_result(&r);
    }
    TermAction::Cmd { command } => {
      let r = executor.call("terminal.execute", json!({"shell_type": "cmd", "command": command.join(" ")}))?;
      print_terminal_result(&r);
    }
  }
  Ok(())
}

fn print_terminal_result(value: &serde_json::Value) {
  let exit_code = value["exit_code"].as_i64().unwrap_or(-1);
  let stdout = value["stdout"].as_str().unwrap_or("");
  let stderr = value["stderr"].as_str().unwrap_or("");
  if !stdout.is_empty() { print!("{stdout}"); if !stdout.ends_with('\n') { println!(); } }
  if !stderr.is_empty() { eprint!("{stderr}"); if !stderr.ends_with('\n') { eprintln!(); } }
  if exit_code != 0 { eprintln!("exit code: {exit_code}"); }
}
