mod bluetooth;
mod elements;
mod input_source;
mod keyboard;
mod mouse;
mod process;
mod screenshot;
mod system_info;
mod terminal;
mod window;

use crate::error::Result;
use crate::platform::*;
use crate::types::*;

pub struct MacPlatform;

impl Default for MacPlatform {
  fn default() -> Self {
    Self::new()
  }
}

impl MacPlatform {
  pub fn new() -> Self {
    Self
  }
}

// === MouseControl ===
impl MouseControl for MacPlatform {
  fn move_cursor(&self, x: i32, y: i32) -> Result<()> {
    mouse::move_cursor(x, y)
  }
  fn get_cursor_position(&self) -> Result<(i32, i32)> {
    mouse::get_cursor_position()
  }
  fn click(
    &self,
    handle: &WindowHandle,
    x: f64,
    y: f64,
    button: MouseButton,
    double_click: bool,
  ) -> Result<InputResult> {
    mouse::click(handle, x, y, button, double_click)
  }
  fn drag(
    &self,
    handle: &WindowHandle,
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
    button: MouseButton,
  ) -> Result<InputResult> {
    mouse::drag(handle, from_x, from_y, to_x, to_y, button)
  }
  fn scroll(&self, handle: &WindowHandle, x: f64, y: f64, delta_x: i32, delta_y: i32) -> Result<InputResult> {
    mouse::scroll(handle, x, y, delta_x, delta_y)
  }
}

// === KeyboardControl ===
impl KeyboardControl for MacPlatform {
  fn type_text(
    &self,
    scope: ElementScope,
    text: &str,
    position: Option<(f64, f64)>,
    enter: bool,
    clear: bool,
  ) -> Result<()> {
    use crate::types::KeyCode;

    // Resolve the target window and focus it
    let handle = match &scope {
      ElementScope::Window(h) => h.clone(),
      ElementScope::Application(pid) => {
        let wins = window::get_windows_by_process_id(*pid)?;
        wins
          .into_iter()
          .next()
          .map(|w| WindowHandle(w.hwnd))
          .ok_or_else(|| anyhow::anyhow!("no windows for pid {pid}"))?
      }
      ElementScope::Foreground => window::get_foreground_window()?,
    };
    window::focus_window(&handle)?;

    // Optional: click at position first
    if let Some((px, py)) = position {
      mouse::click(&handle, px, py, crate::types::MouseButton::Left, false)?;
    }

    // Optional: select-all and delete existing content
    if clear {
      keyboard::send_keys(&[KeyCode::A], Some(&[KeyCode::Win]))?;
      keyboard::send_keys(&[KeyCode::Delete], None)?;
    }

    // Type the text
    keyboard::type_text_raw(&handle, text)?;

    // Optional: press Enter/Return
    if enter {
      keyboard::send_keys(&[KeyCode::Return], None)?;
    }

    Ok(())
  }
  fn send_keys(&self, keys: &[KeyCode], modifiers: Option<&[KeyCode]>) -> Result<()> {
    keyboard::send_keys(keys, modifiers)
  }
  fn key_down(&self, key: KeyCode) -> Result<()> {
    keyboard::key_down(key)
  }
  fn key_release(&self, key: KeyCode) -> Result<()> {
    keyboard::key_release(key)
  }
}

// === WindowManager ===
impl WindowManager for MacPlatform {
  fn list_windows(&self) -> Result<Vec<WindowInfo>> {
    window::list_windows()
  }
  fn list_visible_windows(&self) -> Result<Vec<WindowInfo>> {
    window::list_visible_windows()
  }
  fn find_windows_by_title(&self, pattern: &str, process_name: Option<&str>) -> Result<Vec<WindowInfo>> {
    window::find_windows_by_title(pattern, process_name)
  }
  fn find_windows_by_process(&self, pattern: &str) -> Result<Vec<WindowInfo>> {
    let pat = pattern.to_lowercase();
    Ok(
      window::list_windows()?
        .into_iter()
        .filter(|w| w.process_name.to_lowercase().contains(&pat))
        .collect(),
    )
  }
  fn get_windows_by_process_id(&self, pid: u32) -> Result<Vec<WindowInfo>> {
    window::get_windows_by_process_id(pid)
  }
  fn focus_app(&self, pid: u32) -> Result<()> {
    window::focus_app_by_pid(pid)
  }
  fn get_window_title(&self, handle: &WindowHandle) -> Result<String> {
    window::get_window_title(handle)
  }
  fn get_foreground_window(&self) -> Result<WindowHandle> {
    window::get_foreground_window()
  }
  fn focus_window(&self, handle: &WindowHandle) -> Result<()> {
    window::focus_window(handle)
  }
  fn move_window(&self, handle: &WindowHandle, x: i32, y: i32) -> Result<()> {
    window::move_window(handle, x, y)
  }
  fn resize_window(&self, handle: &WindowHandle, width: i32, height: i32) -> Result<()> {
    window::resize_window(handle, width, height)
  }
  fn set_window_rect(&self, handle: &WindowHandle, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
    window::set_window_rect(handle, x, y, width, height)
  }
  fn minimize_window(&self, handle: &WindowHandle) -> Result<()> {
    window::minimize_window(handle)
  }
  fn maximize_window(&self, handle: &WindowHandle) -> Result<()> {
    window::maximize_window(handle)
  }
  fn restore_window(&self, handle: &WindowHandle) -> Result<()> {
    window::restore_window(handle)
  }
}

// === ElementFinder ===
impl ElementFinder for MacPlatform {
  fn query_elements(&self, scope: ElementScope, q: &ElementQuery) -> Result<Vec<Box<dyn Element>>> {
    elements::query_elements(&scope, q)
  }
  fn query_one(&self, scope: ElementScope, q: &ElementQuery) -> Result<Box<dyn Element>> {
    elements::query_one(&scope, q)
  }
  fn find_by_xpath(&self, scope: ElementScope, xpath: &str) -> Result<Vec<Box<dyn Element>>> {
    elements::find_by_xpath(&scope, xpath)
  }
}

// === UiTreeInspector ===
impl UiTreeInspector for MacPlatform {
  fn get_page_source(&self, handle: &WindowHandle) -> Result<String> {
    elements::get_page_source(handle)
  }
  fn get_page_source_verbose(&self, handle: &WindowHandle) -> Result<String> {
    elements::get_page_source_verbose(handle)
  }
  fn render_tree(&self, scope: ElementScope, query: &ElementQuery) -> Result<String> {
    elements::render_tree(&scope, query)
  }
  fn probe_at_position(&self, x: i32, y: i32) -> Result<String> {
    elements::probe_at_position(x, y)
  }
}

// === ScreenCapture ===
impl ScreenCapture for MacPlatform {
  fn take_desktop_screenshot(&self, config: Option<&ScreenshotConfig>) -> Result<Vec<u8>> {
    screenshot::take_desktop_screenshot(config)
  }
  fn take_window_screenshot(&self, handle: &WindowHandle, _config: Option<&ScreenshotConfig>) -> Result<Vec<u8>> {
    screenshot::take_window_screenshot(handle)
  }
}

// === ProcessManager ===
impl ProcessManager for MacPlatform {
  fn launch_app(&self, path: &str, wait_timeout_ms: u32) -> Result<u32> {
    process::launch_app(path, wait_timeout_ms)
  }
  fn launch_app_async(&self, path_or_bundle: &str, wait: bool) -> Result<u32> {
    process::launch_app_async(path_or_bundle, wait)
  }
  fn terminate_app(&self, pid: u32) -> Result<bool> {
    process::terminate_app(pid)
  }
  fn terminate_apps_by_name(&self, name: &str) -> Result<(u32, u32)> {
    process::terminate_apps_by_name(name)
  }
  fn get_process_ids_by_name(&self, name: &str) -> Result<Vec<u32>> {
    process::get_process_ids_by_name(name)
  }
  fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
    process::list_processes()
  }
  fn get_process_info(&self, pid: u32) -> Result<ProcessInfo> {
    process::get_process_info(pid)
  }
  fn resolve_app_pid(&self, name: &str) -> Result<Option<u32>> {
    process::resolve_app_pid(name)
  }
}

// === SystemInfoProvider ===
impl SystemInfoProvider for MacPlatform {
  fn get_system_info(&self) -> Result<SystemInfo> {
    system_info::get_system_info()
  }
  fn get_screen_size(&self) -> Result<(i32, i32)> {
    system_info::get_screen_size()
  }
  fn list_printers(&self) -> Result<Vec<PrinterInfo>> {
    system_info::list_printers()
  }
  fn print_document(&self, file_path: &str, printer_name: &str) -> Result<()> {
    system_info::print_document(file_path, printer_name)
  }
}

// === BluetoothProvider ===
impl BluetoothProvider for MacPlatform {
  fn scan_classic(&self) -> Result<Vec<BluetoothDeviceInfo>> {
    // macOS doesn't expose classic BT scanning via API
    Ok(Vec::new())
  }
  fn scan_ble(&self, duration_ms: u64) -> Result<Vec<BluetoothDeviceInfo>> {
    bluetooth::scan_ble(duration_ms)
  }
  fn list_pnp(&self) -> Result<Vec<BluetoothDeviceInfo>> {
    system_info::list_bluetooth_devices()
  }
}

// === ServiceProvider ===
impl ServiceProvider for MacPlatform {
  fn list_services(&self) -> Result<Vec<ServiceInfo>> {
    system_info::list_services()
  }
  fn get_service_detail(&self, name: &str) -> Result<ServiceInfo> {
    system_info::get_service_detail(name)
  }
  fn start_service(&self, name: &str) -> Result<()> {
    system_info::start_service(name)
  }
  fn stop_service(&self, name: &str) -> Result<()> {
    system_info::stop_service(name)
  }
  fn restart_service(&self, name: &str) -> Result<()> {
    system_info::stop_service(name)?;
    system_info::start_service(name)
  }
}

// === TerminalProvider ===
impl TerminalProvider for MacPlatform {
  fn execute_command(&self, shell_type: &str, command: &str) -> Result<TerminalResult> {
    terminal::execute_command(shell_type, command)
  }
}

// === RegistryProvider ===
impl RegistryProvider for MacPlatform {
  fn read_value(&self, _key_path: &str, _value_name: Option<&str>) -> Result<(String, String)> {
    Err(anyhow::anyhow!("registry not available on macOS"))
  }
  fn list_subkeys(&self, _key_path: &str) -> Result<Vec<String>> {
    Err(anyhow::anyhow!("registry not available on macOS"))
  }
  fn list_values(&self, _key_path: &str) -> Result<Vec<String>> {
    Err(anyhow::anyhow!("registry not available on macOS"))
  }
  fn set_value(&self, _key_path: &str, _value_name: &str, _value_type: &str, _data: &str) -> Result<()> {
    Err(anyhow::anyhow!("registry not available on macOS"))
  }
  fn create_key(&self, _key_path: &str) -> Result<()> {
    Err(anyhow::anyhow!("registry not available on macOS"))
  }
  fn delete_value(&self, _key_path: &str, _value_name: &str) -> Result<()> {
    Err(anyhow::anyhow!("registry not available on macOS"))
  }
  fn delete_key(&self, _key_path: &str) -> Result<()> {
    Err(anyhow::anyhow!("registry not available on macOS"))
  }
}

// === SoftwareProvider ===
impl SoftwareProvider for MacPlatform {
  fn get_installed_software(&self) -> Result<Vec<SoftwareInfo>> {
    system_info::get_installed_software()
  }
}

// === AudioControl ===
impl AudioControl for MacPlatform {
  fn set_volume(&self, _device_index: Option<usize>, level: u32) -> Result<()> {
    system_info::set_volume(level)
  }
  fn get_volume(&self, _device_index: Option<usize>) -> Result<u32> {
    system_info::get_volume()
  }
  fn set_mute(&self, _device_index: Option<usize>, muted: bool) -> Result<()> {
    system_info::set_mute(muted)
  }
  fn get_mute(&self, _device_index: Option<usize>) -> Result<bool> {
    system_info::get_mute()
  }
  fn set_default_device(&self, device_id: &str) -> Result<()> {
    system_info::set_default_device(device_id)
  }
}
