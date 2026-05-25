//! Daemon lifecycle and IPC tests for both macOS and Windows.

use std::process::Command;
use std::time::Duration;
use std::thread;

fn binary_path() -> String {
  let mut path = std::env::current_exe()
    .unwrap()
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf();
  path.push("tokimo-app-computer-use");
  path.to_string_lossy().to_string()
}

fn daemon_binary_path() -> String {
  let mut path = std::env::current_exe()
    .unwrap()
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf();
  path.push("tokimo-app-computer-daemon");
  path.to_string_lossy().to_string()
}

fn run_cmd(args: &[&str]) -> (String, bool) {
  let output = Command::new(binary_path())
    .args(args)
    .output()
    .expect("failed to execute command");
  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let success = output.status.success();
  (stdout, success)
}

#[cfg(unix)]
fn daemon_pid() -> Option<u32> {
  let output = Command::new("pgrep")
    .arg("-x")
    .arg("tokimo-app-computer-daemon")
    .output()
    .ok()?;
  let stdout = String::from_utf8_lossy(&output.stdout);
  stdout.trim().lines().next()?.parse().ok()
}

#[cfg(windows)]
fn daemon_pid() -> Option<u32> {
  let output = Command::new("tasklist")
    .args(["/FI", "IMAGENAME eq tokimo-app-computer-daemon.exe", "/FO", "CSV", "/NH"])
    .output()
    .ok()?;
  let stdout = String::from_utf8_lossy(&output.stdout);
  stdout
    .lines()
    .next()?
    .split(',')
    .nth(1)?
    .trim_matches('"')
    .parse()
    .ok()
}

fn is_daemon_running() -> bool {
  daemon_pid().is_some()
}

fn kill_daemon() {
  #[cfg(unix)]
  {
    let _ = Command::new("pkill")
      .arg("-x")
      .arg("tokimo-app-computer-daemon")
      .output();
  }
  #[cfg(windows)]
  {
    let _ = Command::new("taskkill")
      .args(["/IM", "tokimo-app-computer-daemon.exe", "/F"])
      .output();
  }
  thread::sleep(Duration::from_millis(500));
}

fn start_daemon() {
  let _ = Command::new(daemon_binary_path())
    .spawn()
    .expect("failed to spawn daemon");
  thread::sleep(Duration::from_millis(1000));
}

#[test]
fn test_daemon_status_when_not_running() {
  kill_daemon();
  assert!(!is_daemon_running());

  let (stdout, success) = run_cmd(&["daemon", "status"]);
  assert!(success);
  assert!(stdout.contains("Daemon is not running"));
}

#[test]
fn test_daemon_start_and_stop() {
  kill_daemon();

  // Start via CLI
  let (stdout, success) = run_cmd(&["daemon", "start"]);
  assert!(success);
  assert!(stdout.contains("Daemon started"));
  assert!(is_daemon_running());

  // Stop via CLI
  let (stdout, success) = run_cmd(&["daemon", "stop"]);
  assert!(success);
  assert!(stdout.contains("Daemon stopped"));
  assert!(!is_daemon_running());
}

#[test]
fn test_daemon_status_when_running() {
  kill_daemon();
  start_daemon();
  assert!(is_daemon_running());

  let (stdout, success) = run_cmd(&["daemon", "status"]);
  assert!(success);
  assert!(stdout.contains("Daemon is running"));
  assert!(stdout.contains("PID"));

  kill_daemon();
}

#[cfg(target_os = "macos")]
mod macos_tests {
  use super::*;

  fn ensure_calculator() {
    let _ = Command::new("open").arg("-a").arg("Calculator").output();
    thread::sleep(Duration::from_millis(1500));
  }

  #[test]
  fn test_element_find_via_daemon() {
    ensure_calculator();
    kill_daemon();
    start_daemon();

    let (stdout, success) = run_cmd(&["element", "find", "--app", "Calculator"]);
    assert!(success, "find failed: {stdout}");
    assert!(stdout.contains("Calculator"));
    assert!(stdout.contains("Button"));

    kill_daemon();
  }

  #[test]
  fn test_element_click_via_daemon() {
    ensure_calculator();
    kill_daemon();
    start_daemon();

    let (find_out, success) = run_cmd(&["element", "find", "--app", "Calculator"]);
    assert!(success);

    let btn_ref = find_out
      .lines()
      .find(|l| l.contains("Button") && l.contains("Seven"))
      .and_then(|l| l.split_whitespace().next())
      .expect("no button 7");

    let (stdout, success) = run_cmd(&["element", "click", "--app", "Calculator", "--ref", btn_ref]);
    assert!(success);
    assert!(stdout.contains("ok"));

    kill_daemon();
  }
}

#[cfg(windows)]
mod windows_tests {
  use super::*;

  fn ensure_calculator() {
    let _ = Command::new("cmd").args(["/C", "start", "calc.exe"]).output();
    thread::sleep(Duration::from_millis(2000));
  }

  #[test]
  fn test_element_find_via_daemon() {
    ensure_calculator();
    kill_daemon();
    start_daemon();

    let (stdout, success) = run_cmd(&["element", "find", "--app", "Calculator"]);
    assert!(success, "find failed: {stdout}");
    assert!(stdout.contains("Window") || stdout.contains("Button"));

    kill_daemon();
  }

  #[test]
  fn test_element_click_via_daemon() {
    ensure_calculator();
    kill_daemon();
    start_daemon();

    let (find_out, success) = run_cmd(&["element", "find", "--app", "Calculator"]);
    assert!(success);

    let btn_ref = find_out
      .lines()
      .find(|l| l.contains("Button"))
      .and_then(|l| l.split_whitespace().next())
      .expect("no button");

    let (stdout, success) = run_cmd(&["element", "click", "--app", "Calculator", "--ref", btn_ref]);
    assert!(success);
    assert!(stdout.contains("ok"));

    kill_daemon();
  }
}
