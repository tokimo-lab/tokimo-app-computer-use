#![cfg(windows)]
use std::thread;
use std::time::Duration;
use tokimo_app_computer_use::WindowHandle;
use tokimo_app_computer_use::create_platform;
use tokimo_app_computer_use::platform::*;

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
fn test_list_windows() {
  let platform = setup();
  let windows = platform.list_windows().expect("list_windows");
  assert!(!windows.is_empty());
}

#[test]
fn test_list_visible_windows() {
  let platform = setup();
  let windows = platform.list_visible_windows().expect("list_visible_windows");
  for w in &windows {
    assert!(w.is_visible, "Window '{}' should be visible", w.title);
  }
}

#[test]
fn test_find_window_by_title() {
  let platform = setup();
  let (pid, _) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");
  let handle = platform.find_window_by_title("Calculator");
  assert!(handle.is_ok(), "Should find Calculator window");
  let _ = platform.terminate_app(pid);
}

#[test]
fn test_find_windows_by_title_pattern() {
  let platform = setup();
  let (pid, _) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");
  let windows = platform.find_windows_by_title("Calc", None).expect("find by pattern");
  assert!(!windows.is_empty());
  let _ = platform.terminate_app(pid);
}

#[test]
fn test_window_title() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");
  let title = platform.get_window_title(&handle).expect("get title");
  assert!(
    title.contains("Calculator"),
    "Title should contain 'Calculator', got: {title}"
  );
  let _ = platform.terminate_app(pid);
}

#[test]
fn test_focus_window() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");
  let result = platform.focus_window(&handle);
  assert!(result.is_ok());
  let _ = platform.terminate_app(pid);
}

#[test]
fn test_move_window() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");
  let result = platform.move_window(&handle, 100, 100);
  assert!(result.is_ok());

  let windows = platform.get_windows_by_process_id(pid).expect("get windows by pid");
  if let Some(w) = windows.first() {
    assert_eq!(w.x, 100, "Window X should be 100");
    assert_eq!(w.y, 100, "Window Y should be 100");
  }
  let _ = platform.terminate_app(pid);
}

#[test]
fn test_resize_window() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");
  let result = platform.resize_window(&handle, 800, 600);
  assert!(result.is_ok());
  let _ = platform.terminate_app(pid);
}

#[test]
fn test_set_window_rect() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");
  let result = platform.set_window_rect(&handle, 50, 50, 900, 700);
  assert!(result.is_ok());
  let _ = platform.terminate_app(pid);
}

#[test]
fn test_minimize_maximize_restore() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.minimize_window(&handle);
  assert!(result.is_ok());
  thread::sleep(Duration::from_millis(500));

  let result = platform.maximize_window(&handle);
  assert!(result.is_ok());
  thread::sleep(Duration::from_millis(500));

  let result = platform.restore_window(&handle);
  assert!(result.is_ok());

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_get_child_windows() {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");
  let children = platform.get_child_windows(&handle).expect("get children");
  // UWP Calculator may not have traditional Win32 child windows
  println!("Found {} child windows", children.len());
  let _ = platform.terminate_app(pid);
}
