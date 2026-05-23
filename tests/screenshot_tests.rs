use std::thread;
use std::time::Duration;
use tokimo_app_computer_use::create_platform;
use tokimo_app_computer_use::platform::*;
use tokimo_app_computer_use::types::*;
use tokimo_app_computer_use::WindowHandle;

fn setup() -> impl PlatformProvider + Send + Sync {
  create_platform()
}

fn launch_and_get_handle(
  platform: &dyn PlatformProvider,
  path: &str,
  title_pattern: &str,
) -> (u32, WindowHandle) {
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
fn test_desktop_screenshot_default() {
  let platform = setup();
  let data = platform
    .take_desktop_screenshot(None)
    .expect("desktop screenshot");
  assert!(!data.is_empty(), "Screenshot should not be empty");
  assert!(data.len() > 12, "Screenshot should be at least 12 bytes");
  assert_eq!(&data[0..4], b"RIFF", "Should start with RIFF (WebP)");
  assert_eq!(&data[8..12], b"WEBP", "Should contain WEBP marker");
}

#[test]
fn test_desktop_screenshot_png() {
  let platform = setup();
  let config = ScreenshotConfig {
    format: Some("png".to_string()),
    quality: Some(100),
    ..Default::default()
  };
  let data = platform
    .take_desktop_screenshot(Some(&config))
    .expect("png screenshot");
  assert!(!data.is_empty());
  assert_eq!(
    &data[0..8],
    &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
    "Should be valid PNG"
  );
}

#[test]
fn test_desktop_screenshot_jpeg() {
  let platform = setup();
  let config = ScreenshotConfig {
    format: Some("jpeg".to_string()),
    quality: Some(80),
    ..Default::default()
  };
  let data = platform
    .take_desktop_screenshot(Some(&config))
    .expect("jpeg screenshot");
  assert!(!data.is_empty());
  assert_eq!(&data[0..2], &[0xFF, 0xD8], "Should be valid JPEG");
}

#[test]
fn test_desktop_screenshot_webp() {
  let platform = setup();
  let config = ScreenshotConfig {
    format: Some("webp".to_string()),
    quality: Some(90),
    ..Default::default()
  };
  let data = platform
    .take_desktop_screenshot(Some(&config))
    .expect("webp screenshot");
  assert!(!data.is_empty());
  assert_eq!(&data[0..4], b"RIFF");
  assert_eq!(&data[8..12], b"WEBP");
}

#[test]
fn test_desktop_screenshot_with_region() {
  let platform = setup();
  let config = ScreenshotConfig {
    left: Some(0),
    top: Some(0),
    right: Some(800),
    bottom: Some(600),
    format: Some("png".to_string()),
    quality: Some(100),
  };
  let data = platform
    .take_desktop_screenshot(Some(&config))
    .expect("region screenshot");
  assert!(!data.is_empty());
  assert_eq!(
    &data[0..8],
    &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
  );
}

#[test]
fn test_window_screenshot() {
  let platform = setup();
  let (pid, handle) =
    launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");

  let data = platform
    .take_window_screenshot(&handle, None)
    .expect("window screenshot");
  assert!(!data.is_empty(), "Window screenshot should not be empty");

  let _ = platform.terminate_app(pid);
}

#[test]
fn test_screenshot_quality_variations() {
  let platform = setup();

  let config_low = ScreenshotConfig {
    format: Some("jpeg".to_string()),
    quality: Some(10),
    ..Default::default()
  };
  let config_high = ScreenshotConfig {
    format: Some("jpeg".to_string()),
    quality: Some(100),
    ..Default::default()
  };

  let low = platform
    .take_desktop_screenshot(Some(&config_low))
    .expect("low quality");
  let high = platform
    .take_desktop_screenshot(Some(&config_high))
    .expect("high quality");

  assert!(!low.is_empty());
  assert!(!high.is_empty());
  assert!(
    high.len() >= low.len(),
    "High quality JPEG should be >= low quality"
  );
}
