//! End-to-end tests for the CLI and daemon.
//!
//! These tests start the daemon, connect via named pipe, and exercise all CLI commands.

use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use std::time::Duration;

const PIPE_NAME: &str = r"\\.\pipe\tokimo-app-computer-daemon";

struct DaemonGuard(Child);

impl Drop for DaemonGuard {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

fn bin_dir() -> std::path::PathBuf {
  let mut path = std::env::current_exe().unwrap();
  path.pop(); // remove test binary name
  path.pop(); // remove deps/
  path
}

fn daemon_bin() -> std::path::PathBuf {
  bin_dir().join("tokimo-app-computer-daemon.exe")
}

fn cli_bin() -> std::path::PathBuf {
  bin_dir().join("tokimo-app-computer-use.exe")
}

fn start_daemon() -> DaemonGuard {
  // Kill any existing daemon
  let _ = Command::new("taskkill")
    .args(["/f", "/im", "tokimo-app-computer-daemon.exe"])
    .output();
  std::thread::sleep(Duration::from_millis(500));

  let mut child = Command::new(daemon_bin())
    .creation_flags(0x08000000) // CREATE_NO_WINDOW
    .spawn()
    .expect("failed to start daemon");

  // Wait for pipe to be available
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
    // Retry connection with backoff — daemon needs time to call ConnectNamedPipe
    let file = {
      let mut last_err = None;
      let mut file = None;
      for i in 0..100 {
        match std::fs::OpenOptions::new()
          .read(true)
          .write(true)
          .open(PIPE_NAME)
        {
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
      file.unwrap_or_else(|| panic!("cannot connect to daemon pipe after retries: {:?}", last_err))
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
  let output = Command::new(cli_bin())
    .output()
    .expect("failed to run CLI");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("Usage:"));
}

// ── Daemon + pipe tests (need daemon running) ──

#[test]
fn daemon_system_info() {
  let _daemon = start_daemon();
  let mut client = PipeClient::connect();

  let result = client.call("system.info", serde_json::json!({}));
  assert!(result.get("locale_name").is_some());
  assert!(result.get("major_version").is_some());
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
  assert!(!windows.is_empty(), "should have at least one window");

  // Each window should have required fields
  let first = &windows[0];
  assert!(first.get("hwnd").is_some());
  assert!(first.get("title").is_some());
  assert!(first.get("processId").is_some());
}

#[test]
fn daemon_window_list_visible() {
  let _daemon = start_daemon();
  let mut client = PipeClient::connect();

  let result = client.call("window.list_visible", serde_json::json!({}));
  let windows = result.as_array().expect("expected array");
  assert!(!windows.is_empty(), "should have at least one visible window");

  // All returned windows should be visible
  for w in windows {
    assert_eq!(w.get("isVisible").and_then(|v| v.as_bool()), Some(true));
  }
}

#[test]
fn daemon_mouse_get_position() {
  let _daemon = start_daemon();
  let mut client = PipeClient::connect();

  let result = client.call("mouse.get_position", serde_json::json!({}));
  assert!(result.get("x").is_some());
  assert!(result.get("y").is_some());
}

#[test]
fn daemon_process_get_pids() {
  let _daemon = start_daemon();
  let mut client = PipeClient::connect();

  let result = client.call(
    "process.get_pids",
    serde_json::json!({"name": "explorer.exe"}),
  );
  let pids = result.as_array().expect("expected array");
  assert!(!pids.is_empty(), "explorer.exe should be running");
}

#[test]
fn daemon_unknown_method_returns_error() {
  let _daemon = start_daemon();
  let mut client = PipeClient::connect();

  // Send a request with an invalid method via raw pipe
  let req = serde_json::json!({
    "id": 999,
    "method": "nonexistent.method",
    "params": {}
  });
  let mut req_json = serde_json::to_string(&req).unwrap();
  req_json.push('\n');
  std::io::Write::write_all(&mut client.writer, req_json.as_bytes()).unwrap();

  let mut line = String::new();
  std::io::BufRead::read_line(&mut client.reader, &mut line).unwrap();
  let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

  assert!(resp.get("error").is_some());
  let msg = resp["error"]["message"].as_str().unwrap();
  assert!(msg.contains("unknown method"));
}

#[test]
fn daemon_multiple_requests_same_connection() {
  let _daemon = start_daemon();
  let mut client = PipeClient::connect();

  // Send multiple requests on the same connection
  let r1 = client.call("system.info", serde_json::json!({}));
  assert!(r1.get("locale_name").is_some());

  let r2 = client.call("system.screen_size", serde_json::json!({}));
  assert!(r2.get("width").is_some());

  let r3 = client.call("mouse.get_position", serde_json::json!({}));
  assert!(r3.get("x").is_some());
}

// ── CLI auto-start daemon test ──

#[test]
fn cli_auto_starts_daemon_and_gets_result() {
  let _daemon = start_daemon();

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
  // Compact format: key=value lines
  assert!(stdout.contains("locale_name="));
  assert!(stdout.contains("major_version="));
}

#[test]
fn cli_window_list_compact_format() {
  let _daemon = start_daemon();

  let output = Command::new(cli_bin())
    .args(["window", "list-visible"])
    .output()
    .expect("failed to run CLI");

  assert!(
    output.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  // TSV format: hwnd\tpid\tprocess\ttitle
  let lines: Vec<&str> = stdout.trim().lines().collect();
  assert!(!lines.is_empty(), "should have at least one window");
  for line in &lines {
    let cols: Vec<&str> = line.split('\t').collect();
    assert!(cols.len() >= 4, "expected 4 TSV columns, got: {line}");
  }
}

#[test]
fn cli_mouse_position_compact_format() {
  let _daemon = start_daemon();

  let output = Command::new(cli_bin())
    .args(["mouse", "position"])
    .output()
    .expect("failed to run CLI");

  assert!(
    output.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
  // Format: x,y
  let parts: Vec<&str> = stdout.split(',').collect();
  assert_eq!(parts.len(), 2, "expected 'x,y', got: {stdout}");
  assert!(parts[0].parse::<i64>().is_ok());
  assert!(parts[1].parse::<i64>().is_ok());
}

#[test]
fn cli_screen_size_compact_format() {
  let _daemon = start_daemon();

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
  // Format: WxH
  assert!(stdout.contains('x'), "expected 'WxH', got: {stdout}");
  let parts: Vec<&str> = stdout.split('x').collect();
  assert_eq!(parts.len(), 2);
  assert!(parts[0].parse::<u64>().unwrap() > 0);
  assert!(parts[1].parse::<u64>().unwrap() > 0);
}

#[test]
fn cli_process_pids_compact_format() {
  let _daemon = start_daemon();

  let output = Command::new(cli_bin())
    .args(["process", "get-pids", "explorer.exe"])
    .output()
    .expect("failed to run CLI");

  assert!(
    output.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  let lines: Vec<&str> = stdout.trim().lines().collect();
  assert!(!lines.is_empty(), "explorer.exe should be running");
  for line in &lines {
    assert!(line.parse::<u32>().is_ok(), "expected PID number, got: {line}");
  }
}
