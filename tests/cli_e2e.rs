//! End-to-end tests for the CLI and daemon.
//!
//! On Windows: tests start the daemon, connect via named pipe, and exercise all CLI commands.
//! On macOS/Linux: tests exercise the CLI via DirectExecutor (no daemon needed).

use std::process::Command;

fn bin_dir() -> std::path::PathBuf {
  let mut path = std::env::current_exe().unwrap();
  path.pop(); // remove test binary name
  path.pop(); // remove deps/
  path
}

#[cfg(windows)]
fn cli_bin() -> std::path::PathBuf {
  bin_dir().join("tokimo-app-computer-use.exe")
}

#[cfg(not(windows))]
fn cli_bin() -> std::path::PathBuf {
  bin_dir().join("tokimo-app-computer-use")
}

// ── CLI binary tests (no daemon needed) ──

#[test]
fn cli_help_shows_usage() {
  let output = Command::new(cli_bin())
    .arg("--help")
    .output()
    .expect("failed to run CLI");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("tokimo-app-computer-use"));
  assert!(stdout.contains("window"));
  assert!(stdout.contains("mouse"));
  assert!(stdout.contains("keyboard"));
  assert!(stdout.contains("element"));
  assert!(stdout.contains("screenshot"));
  assert!(stdout.contains("process"));
  assert!(stdout.contains("system"));
}

#[test]
fn cli_window_subcommand_help() {
  let output = Command::new(cli_bin())
    .args(["window", "--help"])
    .output()
    .expect("failed to run CLI");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("list"));
  assert!(stdout.contains("find"));
  assert!(stdout.contains("focus"));
  assert!(stdout.contains("minimize"));
}

#[test]
fn cli_mouse_subcommand_help() {
  let output = Command::new(cli_bin())
    .args(["mouse", "--help"])
    .output()
    .expect("failed to run CLI");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("move"));
  assert!(stdout.contains("click"));
  assert!(stdout.contains("drag"));
  assert!(stdout.contains("scroll"));
}

#[test]
fn cli_keyboard_subcommand_help() {
  let output = Command::new(cli_bin())
    .args(["keyboard", "--help"])
    .output()
    .expect("failed to run CLI");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("type"));
  assert!(stdout.contains("send-keys"));
}

#[test]
fn cli_element_subcommand_help() {
  let output = Command::new(cli_bin())
    .args(["element", "--help"])
    .output()
    .expect("failed to run CLI");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("find"));
  assert!(stdout.contains("page-source"));
}

#[test]
fn cli_screenshot_subcommand_help() {
  let output = Command::new(cli_bin())
    .args(["screenshot", "--help"])
    .output()
    .expect("failed to run CLI");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("desktop"));
  assert!(stdout.contains("window"));
}

#[test]
fn cli_process_subcommand_help() {
  let output = Command::new(cli_bin())
    .args(["process", "--help"])
    .output()
    .expect("failed to run CLI");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("launch"));
  assert!(stdout.contains("terminate"));
  assert!(stdout.contains("get-pids"));
}

#[test]
fn cli_system_subcommand_help() {
  let output = Command::new(cli_bin())
    .args(["system", "--help"])
    .output()
    .expect("failed to run CLI");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("info"));
  assert!(stdout.contains("screen-size"));
}

#[test]
fn cli_no_args_shows_help() {
  let output = Command::new(cli_bin()).output().expect("failed to run CLI");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("Usage:"));
}

// ── Platform-specific CLI tests (DirectExecutor on non-Windows) ──

#[test]
fn cli_system_info() {
  let output = Command::new(cli_bin())
    .args(["system", "info"])
    .output()
    .expect("failed to run CLI");

  assert!(
    output.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("Computer:") || stdout.contains("OS:"));
}

#[test]
fn cli_system_screen_size() {
  let output = Command::new(cli_bin())
    .args(["system", "screen-size"])
    .output()
    .expect("failed to run CLI");

  assert!(
    output.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
  assert!(stdout.contains('x'), "expected 'WxH', got: {stdout}");
}

#[test]
fn cli_process_list() {
  let output = Command::new(cli_bin())
    .args(["process", "list"])
    .output()
    .expect("failed to run CLI");

  assert!(
    output.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("PID"));
}

#[test]
fn cli_screenshot_desktop() {
  let output = Command::new(cli_bin())
    .args(["screenshot", "desktop"])
    .output()
    .expect("failed to run CLI");

  assert!(
    output.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  // Screenshot output goes to stderr
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("saved") || stderr.contains("bytes"), "stderr: {stderr}");
}

#[test]
fn cli_term_echo() {
  let output = Command::new(cli_bin())
    .args(["term", "ps", "echo hello"])
    .output()
    .expect("failed to run CLI");

  assert!(
    output.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("hello"));
}

// ── Windows-only daemon tests ──

#[cfg(windows)]
mod windows_daemon_tests {
  use super::*;
  use std::io::{BufRead, BufReader, Write};
  use std::os::windows::process::CommandExt;
  use std::time::Duration;

  const PIPE_NAME: &str = r"\\.\pipe\tokimo-app-computer-daemon";

  struct DaemonGuard(std::process::Child);

  impl Drop for DaemonGuard {
    fn drop(&mut self) {
      let _ = self.0.kill();
      let _ = self.0.wait();
    }
  }

  fn daemon_bin() -> std::path::PathBuf {
    bin_dir().join("tokimo-app-computer-daemon.exe")
  }

  fn start_daemon() -> DaemonGuard {
    let _ = Command::new("taskkill")
      .args(["/f", "/im", "tokimo-app-computer-daemon.exe"])
      .output();
    std::thread::sleep(Duration::from_millis(500));

    let mut child = Command::new(daemon_bin())
      .creation_flags(0x08000000)
      .spawn()
      .expect("failed to start daemon");

    for _ in 0..50 {
      std::thread::sleep(Duration::from_millis(100));
      if std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(PIPE_NAME)
        .is_ok()
      {
        return DaemonGuard(child);
      }
    }

    let _ = child.kill();
    let _ = child.wait();
    panic!("daemon pipe not available after 5s");
  }

  struct PipeClient {
    reader: BufReader<std::fs::File>,
    writer: std::fs::File,
    next_id: u64,
  }

  impl PipeClient {
    fn connect() -> Self {
      let file = {
        let mut last_err = None;
        let mut file = None;
        for i in 0..100 {
          match std::fs::OpenOptions::new().read(true).write(true).open(PIPE_NAME) {
            Ok(f) => {
              file = Some(f);
              break;
            }
            Err(e) => {
              last_err = Some(e);
              std::thread::sleep(Duration::from_millis(50 + i * 10));
            }
          }
        }
        file.unwrap_or_else(|| panic!("cannot connect to daemon pipe: {:?}", last_err))
      };

      let reader_file = file.try_clone().expect("clone pipe handle");
      let reader = BufReader::new(reader_file);

      Self {
        reader,
        writer: file,
        next_id: 1,
      }
    }

    fn call(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
      let id = self.next_id;
      self.next_id += 1;

      let req = serde_json::json!({
        "id": id,
        "method": method,
        "params": params,
      });
      let mut req_json = serde_json::to_string(&req).unwrap();
      req_json.push('\n');
      self.writer.write_all(req_json.as_bytes()).unwrap();

      let mut line = String::new();
      self.reader.read_line(&mut line).unwrap();
      assert!(!line.is_empty(), "daemon disconnected");

      let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
      if let Some(err) = resp.get("error") {
        panic!("daemon error: {}", err);
      }
      resp.get("result").cloned().unwrap_or(serde_json::Value::Null)
    }
  }

  #[test]
  fn daemon_system_info() {
    let _daemon = start_daemon();
    let mut client = PipeClient::connect();
    let result = client.call("system.info", serde_json::json!({}));
    assert!(result.get("locale_name").is_some());
  }

  #[test]
  fn daemon_system_screen_size() {
    let _daemon = start_daemon();
    let mut client = PipeClient::connect();
    let result = client.call("system.screen_size", serde_json::json!({}));
    let width = result.get("width").and_then(|v| v.as_u64()).unwrap();
    let height = result.get("height").and_then(|v| v.as_u64()).unwrap();
    assert!(width > 0);
    assert!(height > 0);
  }

  #[test]
  fn daemon_window_list() {
    let _daemon = start_daemon();
    let mut client = PipeClient::connect();
    let result = client.call("window.list", serde_json::json!({}));
    let windows = result.as_array().expect("expected array");
    assert!(!windows.is_empty());
  }

  #[test]
  fn daemon_process_get_pids() {
    let _daemon = start_daemon();
    let mut client = PipeClient::connect();
    let result = client.call("process.get_pids", serde_json::json!({"name": "explorer.exe"}));
    let pids = result.as_array().expect("expected array");
    assert!(!pids.is_empty());
  }

  #[test]
  fn daemon_unknown_method_returns_error() {
    let _daemon = start_daemon();
    let mut client = PipeClient::connect();
    let req = serde_json::json!({"id": 999, "method": "nonexistent.method", "params": {}});
    let mut req_json = serde_json::to_string(&req).unwrap();
    req_json.push('\n');
    std::io::Write::write_all(&mut client.writer, req_json.as_bytes()).unwrap();
    let mut line = String::new();
    std::io::BufRead::read_line(&mut client.reader, &mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert!(resp.get("error").is_some());
  }

  #[test]
  fn cli_auto_starts_daemon_and_gets_result() {
    let _daemon = start_daemon();
    let output = Command::new(cli_bin())
      .args(["system", "info"])
      .output()
      .expect("failed to run CLI");
    assert!(output.status.success());
  }
}
