#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;

use crate::error::Result;
use crate::types::*;

// ============================================================
// Mouse Control
// ============================================================
pub trait MouseControl {
  fn move_cursor(&self, x: i32, y: i32) -> Result<()>;
  fn get_cursor_position(&self) -> Result<(i32, i32)>;
  fn click(
    &self,
    handle: &WindowHandle,
    x: f64,
    y: f64,
    button: MouseButton,
    double_click: bool,
  ) -> Result<InputResult>;
  fn drag(
    &self,
    handle: &WindowHandle,
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
    button: MouseButton,
  ) -> Result<InputResult>;
  fn scroll(&self, handle: &WindowHandle, x: f64, y: f64, delta_x: i32, delta_y: i32) -> Result<InputResult>;
}

// ============================================================
// Keyboard Control
// ============================================================
pub trait KeyboardControl {
  fn type_text(&self, scope: ElementScope, text: &str, position: Option<(f64, f64)>, enter: bool, clear: bool)
    -> Result<()>;
  fn send_keys(&self, keys: &[KeyCode], modifiers: Option<&[KeyCode]>) -> Result<()>;
  fn key_down(&self, key: KeyCode) -> Result<()>;
  fn key_release(&self, key: KeyCode) -> Result<()>;
}

// ============================================================
// Window Manager
// ============================================================
pub trait WindowManager {
  fn list_windows(&self) -> Result<Vec<WindowInfo>>;
  fn list_visible_windows(&self) -> Result<Vec<WindowInfo>>;
  fn find_windows_by_title(&self, pattern: &str, process_name: Option<&str>) -> Result<Vec<WindowInfo>>;
  fn find_windows_by_process(&self, pattern: &str) -> Result<Vec<WindowInfo>>;
  fn get_windows_by_process_id(&self, pid: u32) -> Result<Vec<WindowInfo>>;
  /// Activate an app by pid and raise its first AX window. Used when we have a pid
  /// but no valid WindowHandle (e.g. `--app` resolution via NSWorkspace).
  fn focus_app(&self, pid: u32) -> Result<()>;
  fn get_window_title(&self, handle: &WindowHandle) -> Result<String>;
  fn get_foreground_window(&self) -> Result<WindowHandle>;
  fn focus_window(&self, handle: &WindowHandle) -> Result<()>;
  fn move_window(&self, handle: &WindowHandle, x: i32, y: i32) -> Result<()>;
  fn resize_window(&self, handle: &WindowHandle, width: i32, height: i32) -> Result<()>;
  fn set_window_rect(&self, handle: &WindowHandle, x: i32, y: i32, width: i32, height: i32) -> Result<()>;
  fn minimize_window(&self, handle: &WindowHandle) -> Result<()>;
  fn maximize_window(&self, handle: &WindowHandle) -> Result<()>;
  fn restore_window(&self, handle: &WindowHandle) -> Result<()>;
}

// ============================================================
// Element (UI Automation element)
// ============================================================
pub trait Element: Send {
  fn automation_id(&self) -> String;
  fn name(&self) -> String;
  fn class_name(&self) -> String;
  fn control_type(&self) -> String;
  fn help_text(&self) -> String;
  fn is_enabled(&self) -> bool;
  fn is_clickable(&self) -> bool;
  fn x(&self) -> i32;
  fn y(&self) -> i32;
  fn width(&self) -> i32;
  fn height(&self) -> i32;
  fn text(&self) -> String;
  fn pos(&self, window: Option<&WindowHandle>) -> Result<ElementPosition>;
  fn get_value(&self) -> Result<String>;
  fn set_value(&self, value: &str) -> bool;
  fn can_set_value(&self) -> Result<bool>;
  fn set_range_value(&self, value: f64) -> Result<()>;
  fn focus(&self) -> Result<()>;
  fn set_focused(&self) -> Result<()>;
  fn is_focused(&self) -> Result<bool>;
  fn confirm(&self) -> Result<()>;
  fn to_xml(&self, indent: usize) -> String;
}

// ============================================================
// Element Finder
// ============================================================
pub trait ElementFinder {
  fn query_elements(&self, scope: ElementScope, q: &ElementQuery) -> Result<Vec<Box<dyn Element>>>;
  fn query_one(&self, scope: ElementScope, q: &ElementQuery) -> Result<Box<dyn Element>>;
  fn find_by_xpath(&self, scope: ElementScope, xpath: &str) -> Result<Vec<Box<dyn Element>>>;
}

// ============================================================
// UI Tree Inspector
// ============================================================
pub trait UiTreeInspector {
  fn get_page_source(&self, handle: &WindowHandle) -> Result<String>;
  fn get_page_source_verbose(&self, handle: &WindowHandle) -> Result<String>;
  /// Render a hierarchical tree of `scope`'s UI elements, honoring `query`'s
  /// role/text filter and including hit-test-discovered detached subtrees.
  fn render_tree(&self, scope: ElementScope, query: &ElementQuery) -> Result<String>;
  /// Probe the element at a screen position and dump all its AX attributes.
  fn probe_at_position(&self, x: i32, y: i32) -> Result<String>;
}

// ============================================================
// Screen Capture
// ============================================================
pub trait ScreenCapture {
  fn take_desktop_screenshot(&self, config: Option<&ScreenshotConfig>) -> Result<Vec<u8>>;
  fn take_window_screenshot(&self, handle: &WindowHandle, config: Option<&ScreenshotConfig>) -> Result<Vec<u8>>;
}

// ============================================================
// Process Manager
// ============================================================
pub trait ProcessManager {
  fn launch_app(&self, path: &str, wait_timeout_ms: u32) -> Result<u32>;
  /// Launch app and optionally wait for it to be ready; returns PID.
  fn launch_app_async(&self, path_or_bundle: &str, wait: bool) -> Result<u32>;
  fn terminate_app(&self, pid: u32) -> Result<bool>;
  fn terminate_apps_by_name(&self, name: &str) -> Result<(u32, u32)>;
  fn get_process_ids_by_name(&self, name: &str) -> Result<Vec<u32>>;
  fn list_processes(&self) -> Result<Vec<ProcessInfo>>;
  fn get_process_info(&self, pid: u32) -> Result<ProcessInfo>;
  /// Resolve an "app" identifier (localized name / bundle id / executable name) to a PID.
  /// Returns `None` if the app is not currently running.
  fn resolve_app_pid(&self, name: &str) -> Result<Option<u32>>;
}

// ============================================================
// System Info
// ============================================================
pub trait SystemInfoProvider {
  fn get_system_info(&self) -> Result<SystemInfo>;
  fn get_screen_size(&self) -> Result<(i32, i32)>;
  fn list_printers(&self) -> Result<Vec<PrinterInfo>>;
  fn print_document(&self, file_path: &str, printer_name: &str) -> Result<()>;
}

// ============================================================
// Bluetooth
// ============================================================
pub trait BluetoothProvider {
  fn scan_classic(&self) -> Result<Vec<BluetoothDeviceInfo>>;
  fn scan_ble(&self, duration_ms: u64) -> Result<Vec<BluetoothDeviceInfo>>;
  fn list_pnp(&self) -> Result<Vec<BluetoothDeviceInfo>>;
}

// ============================================================
// Service
// ============================================================
pub trait ServiceProvider {
  fn list_services(&self) -> Result<Vec<ServiceInfo>>;
  fn get_service_detail(&self, name: &str) -> Result<ServiceInfo>;
  fn start_service(&self, name: &str) -> Result<()>;
  fn stop_service(&self, name: &str) -> Result<()>;
  fn restart_service(&self, name: &str) -> Result<()>;
}

// ============================================================
// Terminal
// ============================================================
pub trait TerminalProvider {
  fn execute_command(&self, shell_type: &str, command: &str) -> Result<TerminalResult>;
}

// ============================================================
// Registry
// ============================================================
pub trait RegistryProvider {
  /// Read a registry value. Returns (type_name, value_as_string).
  fn read_value(&self, key_path: &str, value_name: Option<&str>) -> Result<(String, String)>;
  /// List subkey names under a key path.
  fn list_subkeys(&self, key_path: &str) -> Result<Vec<String>>;
  /// List value names under a key path.
  fn list_values(&self, key_path: &str) -> Result<Vec<String>>;
  /// Set (create or update) a registry value.
  fn set_value(&self, key_path: &str, value_name: &str, value_type: &str, data: &str) -> Result<()>;
  /// Create a new subkey.
  fn create_key(&self, key_path: &str) -> Result<()>;
  /// Delete a registry value.
  fn delete_value(&self, key_path: &str, value_name: &str) -> Result<()>;
  /// Delete a registry key (must be empty).
  fn delete_key(&self, key_path: &str) -> Result<()>;
}

// ============================================================
// Software
// ============================================================
pub trait SoftwareProvider {
  fn get_installed_software(&self) -> Result<Vec<SoftwareInfo>>;
}

// ============================================================
// Audio Control
// ============================================================
pub trait AudioControl {
  fn set_volume(&self, device_index: Option<usize>, level: u32) -> Result<()>;
  fn get_volume(&self, device_index: Option<usize>) -> Result<u32>;
  fn set_mute(&self, device_index: Option<usize>, muted: bool) -> Result<()>;
  fn get_mute(&self, device_index: Option<usize>) -> Result<bool>;
  fn set_default_device(&self, device_id: &str) -> Result<()>;
}

// ============================================================
// Composite trait
// ============================================================
pub trait PlatformProvider:
  MouseControl
  + KeyboardControl
  + WindowManager
  + ElementFinder
  + UiTreeInspector
  + ScreenCapture
  + ProcessManager
  + SystemInfoProvider
  + BluetoothProvider
  + ServiceProvider
  + TerminalProvider
  + RegistryProvider
  + SoftwareProvider
  + AudioControl
{
}

impl<T> PlatformProvider for T where
  T: MouseControl
    + KeyboardControl
    + WindowManager
    + ElementFinder
    + UiTreeInspector
    + ScreenCapture
    + ProcessManager
    + SystemInfoProvider
    + BluetoothProvider
    + ServiceProvider
    + TerminalProvider
    + RegistryProvider
    + SoftwareProvider
    + AudioControl
{
}
