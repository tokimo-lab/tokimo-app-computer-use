use crate::error::Result;
use crate::platform::*;
use crate::types::*;
use ::windows::Win32::Foundation::HWND;

pub mod bluetooth;
pub mod elements;
pub mod keyboard;
pub mod mouse;
pub mod process;
pub mod registry;
pub mod screenshot;
pub mod service;
pub mod system_info;
pub mod terminal;
pub mod ui_object;
pub mod wnd;

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
    if handle.0 == 0 {
      let sx = x as i32;
      let sy = y as i32;
      mouse::click_by_pos(sx, sy, button, double_click)?;
      return Ok(InputResult::success(sx, sy, sx, sy));
    }
    mouse::click_by_hwnd_pos(handle.0, x, y, button, double_click)
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
  fn type_text(
    &self,
    scope: ElementScope,
    text: &str,
    position: Option<(f64, f64)>,
    enter: bool,
    clear: bool,
  ) -> Result<()> {
    // Resolve scope → pid → focus that app; this is what the macOS impl does
    // and the daemon's ensure_foreground() already does it for most methods,
    // but we re-do it here so callers using KeyboardControl directly behave
    // the same way.
    match &scope {
      ElementScope::Window(h) => {
        wnd::bring_window_to_front(HWND(h.0 as *mut core::ffi::c_void));
      }
      ElementScope::Application(pid) => {
        if let Ok(wins) = wnd::get_all_windows_by_process_id_internal(*pid)
          && let Some(w) = wins.into_iter().next()
        {
          wnd::bring_window_to_front(HWND(w.hwnd as *mut core::ffi::c_void));
        }
      }
      ElementScope::Foreground => {}
    }
    std::thread::sleep(std::time::Duration::from_millis(120));

    if let Some((px, py)) = position {
      mouse::click_by_pos(px as i32, py as i32, MouseButton::Left, false)?;
      std::thread::sleep(std::time::Duration::from_millis(80));
    }

    if clear {
      keyboard::send_keys(vec![KeyCode::A], Some(vec![KeyCode::Ctrl]))?;
      std::thread::sleep(std::time::Duration::from_millis(50));
      keyboard::send_keys(vec![KeyCode::Delete], None)?;
      std::thread::sleep(std::time::Duration::from_millis(50));
    }

    keyboard::send_text(text)?;

    if enter {
      std::thread::sleep(std::time::Duration::from_millis(50));
      keyboard::send_keys(vec![KeyCode::Return], None)?;
    }
    Ok(())
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
          if w.title.is_empty() {
            return false;
          }
          if wnd::is_system_window(&w.title, &w.process_name) {
            return false;
          }
          if w.width < 100 || w.height < 100 {
            return false;
          }
          let title_match = w.title.to_lowercase().contains(&pat);
          let proc_match = w.process_name.to_lowercase().contains(&pat);
          let pattern_match = title_match || proc_match;
          let filter_match = process_name.is_none_or(|p| w.process_name.to_lowercase().contains(&p.to_lowercase()));
          pattern_match && filter_match
        })
        .collect(),
    )
  }
  fn find_windows_by_process(&self, pattern: &str) -> Result<Vec<WindowInfo>> {
    use crate::match_util::best_name_score;
    let all = wnd::fetch_all_windows()?;
    let mut scored: Vec<(i32, WindowInfo)> = all
      .into_iter()
      .filter(|w| !wnd::is_system_window(&w.title, &w.process_name))
      .filter_map(|w| {
        let s = best_name_score(pattern, &[&w.process_name, &w.title]);
        if s > 0 { Some((s, w)) } else { None }
      })
      .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(scored.into_iter().map(|(_, w)| w).collect())
  }
  fn get_windows_by_process_id(&self, pid: u32) -> Result<Vec<WindowInfo>> {
    wnd::get_all_windows_by_process_id_internal(pid)
  }
  fn focus_app(&self, pid: u32) -> Result<()> {
    // Best-effort: focus the first window of this pid.
    let wins = wnd::get_all_windows_by_process_id_internal(pid)?;
    if let Some(w) = wins.into_iter().next() {
      wnd::bring_window_to_front(HWND(w.hwnd as *mut core::ffi::c_void));
      Ok(())
    } else {
      Err(anyhow::anyhow!("no windows for pid {pid}"))
    }
  }
  fn get_window_title(&self, handle: &WindowHandle) -> Result<String> {
    wnd::get_window_title_by_handle(handle.0)
  }
  fn get_foreground_window(&self) -> Result<WindowHandle> {
    use ::windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_invalid() {
      return Err(anyhow::anyhow!("no foreground window"));
    }
    Ok(WindowHandle(fg.0 as isize as i64))
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
  fn query_elements(&self, scope: ElementScope, q: &ElementQuery) -> Result<Vec<Box<dyn Element>>> {
    elements::query::query_elements(&scope, q)
  }
  fn query_one(&self, scope: ElementScope, q: &ElementQuery) -> Result<Box<dyn Element>> {
    elements::query::query_one(&scope, q)
  }
  fn find_by_xpath(&self, scope: ElementScope, xpath: &str) -> Result<Vec<Box<dyn Element>>> {
    elements::query::find_by_xpath(&scope, xpath)
  }
}

// === UiTreeInspector ===
impl UiTreeInspector for WindowsPlatform {
  fn get_page_source(&self, handle: &WindowHandle) -> Result<String> {
    elements::source::get_page_source_from_hwnd(handle.0)
  }
  fn get_page_source_verbose(&self, handle: &WindowHandle) -> Result<String> {
    elements::source::get_page_source_from_hwnd(handle.0)
  }
  fn render_tree(&self, scope: ElementScope, query: &ElementQuery) -> Result<String> {
    elements::query::render_tree(&scope, query)
  }
  fn probe_at_position(&self, x: i32, y: i32) -> Result<String> {
    elements::query::probe_at_position(x, y)
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
  fn launch_app_async(&self, path_or_bundle: &str, wait: bool) -> Result<u32> {
    // 1. Look-up existing process by name first (idempotent — many apps are
    //    single-instance and just bring an existing window forward).
    let derived_name = if path_or_bundle.to_lowercase().ends_with(".exe") {
      std::path::Path::new(path_or_bundle)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path_or_bundle.to_string())
    } else {
      // Treat as bare name; try both `name` and `name.exe`.
      path_or_bundle.to_string()
    };
    let candidates = if derived_name.to_lowercase().ends_with(".exe") {
      vec![derived_name.clone()]
    } else {
      vec![format!("{derived_name}.exe"), derived_name.clone()]
    };
    for c in &candidates {
      if let Ok(pids) = process::get_processes_by_name(c)
        && let Some(pid) = pids.into_iter().next()
      {
        return Ok(pid);
      }
    }

    // 2. Resolve path: direct path > App Paths registry.
    let resolved_path = if path_or_bundle.contains('\\') || path_or_bundle.contains('/') {
      path_or_bundle.to_string()
    } else {
      // Look up HKLM\Software\Microsoft\Windows\CurrentVersion\App Paths\<name>.exe
      let exe_name = if derived_name.to_lowercase().ends_with(".exe") {
        derived_name.clone()
      } else {
        format!("{derived_name}.exe")
      };
      let key = format!("HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{exe_name}");
      match registry::read_value(&key, None) {
        Ok((_, v)) if !v.is_empty() => v,
        _ => {
          // Try WOW6432Node variant (32-bit installers register there).
          let key2 = format!("HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{exe_name}");
          match registry::read_value(&key2, None) {
            Ok((_, v)) if !v.is_empty() => v,
            _ => {
              return Err(anyhow::anyhow!(
                "could not resolve '{path_or_bundle}' to an executable (no path, no running process, no App Paths entry)"
              ));
            }
          }
        }
      }
    };

    let timeout_ms = if wait { 15_000 } else { 3_000 };
    process::launch_application_and_get_process_id(&resolved_path, timeout_ms)
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
