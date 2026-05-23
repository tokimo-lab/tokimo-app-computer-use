use crate::error::Result;
use crate::types::*;

pub fn execute_command(shell_type: &str, command: &str) -> Result<TerminalResult> {
  let (shell, args) = match shell_type {
    "ps" | "bash" | "sh" => ("bash", vec!["-c", command]),
    "zsh" => ("zsh", vec!["-c", command]),
    "cmd" => ("bash", vec!["-c", command]), // map cmd to bash on macOS
    _ => ("bash", vec!["-c", command]),
  };
  let output = std::process::Command::new(shell).args(&args).output()?;
  Ok(TerminalResult {
    exit_code: output.status.code().unwrap_or(-1),
    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
  })
}
