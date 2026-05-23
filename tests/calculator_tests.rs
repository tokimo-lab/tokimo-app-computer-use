use std::thread;
use std::time::Duration;
use tokimo_app_computer_use::create_platform;
use tokimo_app_computer_use::platform::*;
use tokimo_app_computer_use::WindowHandle;

fn setup() -> impl PlatformProvider + Send + Sync {
  create_platform()
}

/// Launch app, find its window by title, return (pid, handle).
/// Works with UWP apps where the launcher PID differs from the app PID.
fn launch_and_get_handle(
  platform: &dyn PlatformProvider,
  path: &str,
  title_pattern: &str,
) -> (u32, WindowHandle) {
  let _launcher_pid = platform.launch_app(path, 3000).expect("launch app");
  thread::sleep(Duration::from_millis(2000));

  // Find by title — works for both Win32 and UWP apps
  let windows = platform
    .find_windows_by_title(title_pattern, None)
    .expect("find windows by title");
  assert!(
    !windows.is_empty(),
    "Should find at least one window matching '{title_pattern}'"
  );

  let win = &windows[0];
  let pid = win.process_id;
  let handle = WindowHandle(win.hwnd);
  (pid, handle)
}

#[test]
fn test_calculator_launch_and_find() {
  let platform = setup();
  let (pid, handle) =
    launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let xml = platform.get_page_source(&handle).expect("get page source");
  assert!(!xml.is_empty(), "Page source should not be empty");
  assert!(
    xml.contains("UIAutomationTree"),
    "XML should contain UIAutomationTree root"
  );

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_calculator_click_buttons() {
  let platform = setup();
  let (pid, handle) =
    launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.click_by_xpath(
    &handle,
    "//Button[@Name='One']",
    tokimo_app_computer_use::MouseButton::Left,
    false,
  );
  assert!(result.is_ok(), "Click button 'One' should succeed");

  let result = platform.click_by_xpath(
    &handle,
    "//Button[@Name='Two']",
    tokimo_app_computer_use::MouseButton::Left,
    false,
  );
  assert!(result.is_ok(), "Click button 'Two' should succeed");

  thread::sleep(Duration::from_millis(500));
  let _ = platform.terminate_app(pid);
}

#[test]
fn test_calculator_keyboard_input() {
  let platform = setup();
  let (pid, handle) =
    launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let result = platform.type_text(&handle, "123", None);
  assert!(result.is_ok(), "Type text should succeed");

  thread::sleep(Duration::from_millis(500));
  let _ = platform.terminate_app(pid);
}

#[test]
fn test_calculator_screenshot() {
  let platform = setup();
  let (pid, handle) =
    launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let data = platform
    .take_window_screenshot(&handle, None)
    .expect("take screenshot");
  assert!(!data.is_empty(), "Screenshot data should not be empty");
  assert!(data.len() > 8, "Screenshot should be at least 8 bytes");

  let _ = platform.terminate_app(pid);
}
