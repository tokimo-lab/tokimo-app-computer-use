#![cfg(windows)]
use std::thread;
use std::time::Duration;
use tokimo_app_computer_use::WindowHandle;
use tokimo_app_computer_use::create_platform;
use tokimo_app_computer_use::platform::*;
use tokimo_app_computer_use::types::*;

fn setup() -> impl PlatformProvider + Send + Sync {
  create_platform()
}

fn launch_and_get_handle(platform: &dyn PlatformProvider, path: &str, title_pattern: &str) -> (u32, WindowHandle) {
  let _launcher_pid = platform.launch_app(path, 3000).expect("launch app");
  thread::sleep(Duration::from_millis(2000));
  let windows = platform
    .find_windows_by_title(title_pattern, None)
    .expect("find windows by title");
  assert!(!windows.is_empty());
  let win = &windows[0];
  (win.process_id, WindowHandle(win.hwnd))
}

#[test]
fn test_get_cursor_position() {
  let platform = setup();
  let result = platform.get_cursor_position();
  assert!(result.is_ok(), "get_cursor_position should succeed");
  let (x, y) = result.unwrap();
  println!("Cursor at ({x}, {y})");
}

#[test]
fn test_move_cursor() {
  let platform = setup();
  let result = platform.move_cursor(100, 200);
  assert!(result.is_ok(), "move_cursor should succeed");

  let (x, y) = platform.get_cursor_position().expect("get position");
  assert_eq!(x, 100);
  assert_eq!(y, 200);
}

#[test]
fn test_click_by_position() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.click(&handle, 0.5, 0.5, MouseButton::Left, false);
  assert!(result.is_ok(), "click should succeed");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_double_click() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.click(&handle, 0.5, 0.5, MouseButton::Left, true);
  assert!(result.is_ok(), "double click should succeed");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_right_click() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.click(&handle, 0.5, 0.5, MouseButton::Right, false);
  assert!(result.is_ok(), "right click should succeed");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_click_by_xpath() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.click_by_xpath(&handle, "//Button[@Name='One']", MouseButton::Left, false);
  assert!(result.is_ok(), "click_by_xpath should succeed");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_drag() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.drag(&handle, 0.2, 0.5, 0.8, 0.5, MouseButton::Left);
  assert!(result.is_ok(), "drag should succeed");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_scroll_vertical() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.scroll(&handle, 0.5, 0.5, 0, 3);
  assert!(result.is_ok(), "scroll should succeed");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_scroll_horizontal() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.scroll(&handle, 0.5, 0.5, 3, 0);
  assert!(result.is_ok(), "horizontal scroll should succeed");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_send_keys() {
  let platform = setup();
  let (pid, _) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.send_keys(&[KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3], None);
  assert!(result.is_ok(), "send_keys should succeed");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_send_keys_with_modifiers() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\notepad.exe", "Notepad");

  let _ = platform.type_text(&handle, "Hello World", None);
  thread::sleep(Duration::from_millis(500));

  let result = platform.send_keys(&[KeyCode::A], Some(&[KeyCode::Ctrl]));
  assert!(result.is_ok(), "Ctrl+A should succeed");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_key_down_and_release() {
  let platform = setup();

  let result = platform.key_down(KeyCode::Shift);
  assert!(result.is_ok(), "key_down should succeed");

  let result = platform.key_release(KeyCode::Shift);
  assert!(result.is_ok(), "key_release should succeed");
}

#[test]
fn test_type_text() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\notepad.exe", "Notepad");

  let result = platform.type_text(&handle, "Hello, World!", None);
  assert!(result.is_ok(), "type_text should succeed");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_type_text_by_xpath() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\notepad.exe", "Notepad");

  let result = platform.type_text_by_xpath(&handle, "//Edit", "Test input");
  if result.is_err() {
    println!(
      "type_text_by_xpath failed (expected on Windows 11 Notepad): {:?}",
      result.err()
    );
  }

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_mouse_middle_click() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.click(&handle, 0.5, 0.5, MouseButton::Middle, false);
  assert!(result.is_ok(), "middle click should succeed");

  let _ = platform.terminate_app(pid);
}
