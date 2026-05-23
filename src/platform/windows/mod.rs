use crate::error::Result;
use crate::platform::*;
use crate::types::*;
use ::windows::Win32::Foundation::HWND;

pub mod elements;
pub mod keyboard;
pub mod mouse;
pub mod process;
pub mod screenshot;
pub mod system_info;
pub mod ui_object;
pub mod wnd;
pub mod registry;
pub mod bluetooth;
pub mod service;
pub mod terminal;

pub struct WindowsPlatform;

impl Default for WindowsPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsPlatform {
  pub fn new() -> Self {
    Self
  }
}

// === MouseControl ===
impl MouseControl for WindowsPlatform {
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
    mouse::click_by_hwnd_pos(handle.0, x, y, button, double_click)
  }
  fn click_by_xpath(
    &self,
    handle: &WindowHandle,
    xpath: &str,
    button: MouseButton,
    double_click: bool,
  ) -> Result<InputResult> {
    mouse::click_by_xpath(handle.0, xpath, button, double_click)
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
impl KeyboardControl for WindowsPlatform {
  fn type_text(&self, handle: &WindowHandle, text: &str, position: Option<&InputPosition>) -> Result<InputResult> {
    keyboard::send_text_by_hwnd(handle.0, position, text)
  }
  fn type_text_by_xpath(&self, handle: &WindowHandle, xpath: &str, text: &str) -> Result<InputResult> {
    keyboard::send_text_by_xpath(handle.0, xpath, text)
  }
  fn type_text_raw(&self, handle: &WindowHandle, text: &str) -> Result<()> {
    let hwnd = HWND(handle.0 as *mut core::ffi::c_void);
    wnd::bring_window_to_front(hwnd);
    keyboard::send_text(text)
  }
  fn send_keys(&self, keys: &[KeyCode], modifiers: Option<&[KeyCode]>) -> Result<()> {
    keyboard::send_keys(keys.to_vec(), modifiers.map(|m| m.to_vec()))
  }
  fn key_down(&self, key: KeyCode) -> Result<()> {
    keyboard::key_down(key)
  }
  fn key_release(&self, key: KeyCode) -> Result<()> {
    keyboard::key_release(key)
  }
}

// === WindowManager ===
impl WindowManager for WindowsPlatform {
  fn list_windows(&self) -> Result<Vec<WindowInfo>> {
    wnd::fetch_all_windows()
  }
  fn list_visible_windows(&self) -> Result<Vec<WindowInfo>> {
    Ok(wnd::filter_visible_windows(wnd::fetch_all_windows()?))
  }
  fn find_windows_by_title(&self, pattern: &str, process_name: Option<&str>) -> Result<Vec<WindowInfo>> {
    let all = wnd::fetch_all_windows()?;
    let pat = pattern.to_lowercase();
    Ok(
      all
        .into_iter()
        .filter(|w| {
          if w.title.is_empty() { return false; }
          if wnd::is_system_window(&w.title, &w.process_name) { return false; }
          if w.width < 100 || w.height < 100 { return false; }
          let title_match = w.title.to_lowercase().contains(&pat);
          let proc_match = w.process_name.to_lowercase().contains(&pat);
          let pattern_match = title_match || proc_match;
          let filter_match = process_name.is_none_or(|p| w.process_name.to_lowercase().contains(&p.to_lowercase()));
          pattern_match && filter_match
        })
        .collect(),
    )
  }
  fn find_window_by_title(&self, title: &str) -> Result<WindowHandle> {
    wnd::find_window_handle_by_title(title).map(WindowHandle)
  }
  fn get_windows_by_process_id(&self, pid: u32) -> Result<Vec<WindowInfo>> {
    wnd::get_all_windows_by_process_id_internal(pid)
  }
  fn get_windows_by_process_id_with_title(&self, pid: u32, pattern: &str, fuzzy: bool) -> Result<Vec<WindowInfo>> {
    wnd::get_all_windows_by_process_id_with_title_internal(pid, pattern, fuzzy)
  }
  fn get_child_windows(&self, parent: &WindowHandle) -> Result<Vec<WindowInfo>> {
    wnd::get_child_windows_internal(parent.0)
  }
  fn get_window_title(&self, handle: &WindowHandle) -> Result<String> {
    wnd::get_window_title_by_handle(handle.0)
  }
  fn focus_window(&self, handle: &WindowHandle) -> Result<()> {
    wnd::bring_window_to_front(HWND(handle.0 as *mut core::ffi::c_void));
    Ok(())
  }
  fn move_window(&self, handle: &WindowHandle, x: i32, y: i32) -> Result<()> {
    wnd::move_window(handle, x, y)
  }
  fn resize_window(&self, handle: &WindowHandle, width: i32, height: i32) -> Result<()> {
    wnd::resize_window(handle, width, height)
  }
  fn set_window_rect(&self, handle: &WindowHandle, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
    wnd::set_window_rect(handle, x, y, width, height)
  }
  fn minimize_window(&self, handle: &WindowHandle) -> Result<()> {
    wnd::minimize_window(handle)
  }
  fn maximize_window(&self, handle: &WindowHandle) -> Result<()> {
    wnd::maximize_window(handle)
  }
  fn restore_window(&self, handle: &WindowHandle) -> Result<()> {
    wnd::restore_window(handle)
  }
}

// === ElementFinder ===
impl ElementFinder for WindowsPlatform {
  fn find_elements_by_xpath(&self, handle: &WindowHandle, xpath: &str) -> Result<Vec<Box<dyn Element>>> {
    let elems = elements::find::find_elements_by_handle_xpath_internal(handle.0, xpath)?;
    Ok(elems.into_iter().map(|e| Box::new(e) as Box<dyn Element>).collect())
  }
  fn find_first_element_by_xpath(&self, handle: &WindowHandle, xpath: &str) -> Result<Box<dyn Element>> {
    let e = elements::utils::find_first_element_by_xpath(handle.0, xpath)?;
    Ok(Box::new(e))
  }
}

// === UiTreeInspector ===
impl UiTreeInspector for WindowsPlatform {
  fn get_page_source(&self, handle: &WindowHandle) -> Result<String> {
    elements::source::get_page_source_from_hwnd(handle.0)
  }
}

// === ScreenCapture ===
impl ScreenCapture for WindowsPlatform {
  fn take_desktop_screenshot(&self, config: Option<&ScreenshotConfig>) -> Result<Vec<u8>> {
    screenshot::take_desktop_screenshot(config)
  }
  fn take_window_screenshot(&self, handle: &WindowHandle, _config: Option<&ScreenshotConfig>) -> Result<Vec<u8>> {
    screenshot::take_hwnd_screenshot(handle.0, None)
  }
}

// === ProcessManager ===
impl ProcessManager for WindowsPlatform {
  fn launch_app(&self, path: &str, wait_timeout_ms: u32) -> Result<u32> {
    process::launch_application_and_get_process_id(path, wait_timeout_ms)
  }
  fn terminate_app(&self, pid: u32) -> Result<bool> {
    process::terminate_application(pid)
  }
  fn terminate_apps_by_name(&self, name: &str) -> Result<(u32, u32)> {
    process::terminate_applications_by_name(name)
  }
  fn get_process_ids_by_name(&self, name: &str) -> Result<Vec<u32>> {
    process::get_processes_by_name(name)
  }
}

// === SystemInfoProvider ===
impl SystemInfoProvider for WindowsPlatform {
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

// === RegistryProvider ===
impl RegistryProvider for WindowsPlatform {
  fn read_value(&self, key_path: &str, value_name: Option<&str>) -> Result<(String, String)> {
    registry::read_value(key_path, value_name)
  }
  fn list_subkeys(&self, key_path: &str) -> Result<Vec<String>> {
    registry::list_subkeys(key_path)
  }
  fn list_values(&self, key_path: &str) -> Result<Vec<String>> {
    registry::list_values(key_path)
  }
  fn set_value(&self, key_path: &str, value_name: &str, value_type: &str, data: &str) -> Result<()> {
    registry::set_value(key_path, value_name, value_type, data)
  }
  fn create_key(&self, key_path: &str) -> Result<()> {
    registry::create_key(key_path)
  }
  fn delete_value(&self, key_path: &str, value_name: &str) -> Result<()> {
    registry::delete_value(key_path, value_name)
  }
  fn delete_key(&self, key_path: &str) -> Result<()> {
    registry::delete_key(key_path)
  }
}

// === BluetoothProvider ===
impl BluetoothProvider for WindowsPlatform {
  fn scan_classic(&self) -> Result<Vec<BluetoothDeviceInfo>> {
    bluetooth::scan_classic()
  }
  fn scan_ble(&self, duration_ms: u64) -> Result<Vec<BluetoothDeviceInfo>> {
    bluetooth::scan_ble(duration_ms)
  }
  fn list_pnp(&self) -> Result<Vec<BluetoothDeviceInfo>> {
    bluetooth::list_pnp()
  }
}

// === ServiceProvider ===
impl ServiceProvider for WindowsPlatform {
  fn list_services(&self) -> Result<Vec<ServiceInfo>> {
    service::list_services()
  }
  fn get_service_detail(&self, name: &str) -> Result<ServiceInfo> {
    service::get_service_detail(name)
  }
  fn start_service(&self, name: &str) -> Result<()> {
    service::start_service(name)
  }
  fn stop_service(&self, name: &str) -> Result<()> {
    service::stop_service(name)
  }
  fn restart_service(&self, name: &str) -> Result<()> {
    service::stop_service(name)?;
    service::start_service(name)
  }
}

// === TerminalProvider ===
impl TerminalProvider for WindowsPlatform {
  fn execute_command(&self, shell_type: &str, command: &str) -> Result<TerminalResult> {
    terminal::execute_command(shell_type, command)
  }
}

// === SoftwareProvider ===
impl SoftwareProvider for WindowsPlatform {
  fn get_installed_software(&self) -> Result<Vec<SoftwareInfo>> {
    system_info::get_installed_software()
  }
}

// === AudioControl ===
impl AudioControl for WindowsPlatform {
  fn set_volume(&self, device_index: Option<usize>, level: u32) -> Result<()> {
    system_info::set_audio_volume(device_index, level.min(100))
  }
  fn get_volume(&self, device_index: Option<usize>) -> Result<u32> {
    system_info::get_audio_volume(device_index)
  }
  fn set_mute(&self, device_index: Option<usize>, muted: bool) -> Result<()> {
    system_info::set_audio_mute(device_index, muted)
  }
  fn get_mute(&self, device_index: Option<usize>) -> Result<bool> {
    system_info::get_audio_mute(device_index)
  }
  fn set_default_device(&self, device_id: &str) -> Result<()> {
    system_info::set_default_audio_device(device_id)
  }
}
