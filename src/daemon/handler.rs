use super::protocol::{Request, Response};
use crate::platform::PlatformProvider;
use crate::types::*;
use serde_json::json;

pub fn handle_request<P: PlatformProvider + ?Sized>(platform: &P, req: Request) -> Response {
  let result = dispatch(platform, &req.method, &req.params);
  match result {
    Ok(value) => Response::success(req.id, value),
    Err(e) => Response::error(req.id as u32, -1, e.to_string()),
  }
}

pub fn dispatch<P: PlatformProvider + ?Sized>(
  platform: &P,
  method: &str,
  params: &serde_json::Value,
) -> crate::error::Result<serde_json::Value> {
  match method {
    // Window management
    "window.list" => Ok(json!(platform.list_windows()?)),
    "window.list_visible" => Ok(json!(platform.list_visible_windows()?)),
    "window.find_by_title" => {
      let title = req_str(params, "title")?;
      Ok(json!(platform.find_window_by_title(title)?))
    }
    "window.find_windows_by_title" => {
      let pattern = req_str(params, "pattern")?;
      let process_name = params.get("process_name").and_then(|v| v.as_str());
      Ok(json!(platform.find_windows_by_title(pattern, process_name)?))
    }
    "window.title" => {
      let handle = req_handle(params)?;
      Ok(json!(platform.get_window_title(&handle)?))
    }
    "window.info" => {
      let handle = req_handle(params)?;
      let windows = platform.list_windows()?;
      let hwnd_val = handle.0;
      let info = windows
        .iter()
        .find(|w| w.hwnd == hwnd_val)
        .ok_or_else(|| anyhow::anyhow!("window not found: {hwnd_val}"))?;
      Ok(json!(info))
    }
    "window.focus" => {
      let handle = req_handle(params)?;
      platform.focus_window(&handle)?;
      Ok(json!(null))
    }
    "window.move" => {
      let handle = req_handle(params)?;
      let x = req_i32(params, "x")?;
      let y = req_i32(params, "y")?;
      platform.move_window(&handle, x, y)?;
      Ok(json!(null))
    }
    "window.resize" => {
      let handle = req_handle(params)?;
      let width = req_i32(params, "width")?;
      let height = req_i32(params, "height")?;
      platform.resize_window(&handle, width, height)?;
      Ok(json!(null))
    }
    "window.set_rect" => {
      let handle = req_handle(params)?;
      let x = req_i32(params, "x")?;
      let y = req_i32(params, "y")?;
      let width = req_i32(params, "width")?;
      let height = req_i32(params, "height")?;
      platform.set_window_rect(&handle, x, y, width, height)?;
      Ok(json!(null))
    }
    "window.minimize" => {
      let handle = req_handle(params)?;
      platform.minimize_window(&handle)?;
      Ok(json!(null))
    }
    "window.maximize" => {
      let handle = req_handle(params)?;
      platform.maximize_window(&handle)?;
      Ok(json!(null))
    }
    "window.restore" => {
      let handle = req_handle(params)?;
      platform.restore_window(&handle)?;
      Ok(json!(null))
    }
    "window.foreground" => Ok(json!(platform.get_foreground_window()?)),
    "window.children" => {
      let handle = req_handle(params)?;
      Ok(json!(platform.get_child_windows(&handle)?))
    }
    "window.by_process_id" => {
      let pid = req_u32(params, "pid")?;
      Ok(json!(platform.get_windows_by_process_id(pid)?))
    }

    // Mouse
    "mouse.move_cursor" => {
      let x = req_i32(params, "x")?;
      let y = req_i32(params, "y")?;
      platform.move_cursor(x, y)?;
      Ok(json!(null))
    }
    "mouse.get_position" => {
      let (x, y) = platform.get_cursor_position()?;
      Ok(json!({"x": x, "y": y}))
    }
    "mouse.click" => {
      let handle = req_handle(params)?;
      let x = req_f64(params, "x")?;
      let y = req_f64(params, "y")?;
      let button = req_button(params)?;
      let double_click = params.get("double_click").and_then(|v| v.as_bool()).unwrap_or(false);
      Ok(json!(platform.click(&handle, x, y, button, double_click)?))
    }
    "mouse.click_by_xpath" => {
      let handle = req_handle(params)?;
      let xpath = req_str(params, "xpath")?;
      let button = req_button(params)?;
      let double_click = params.get("double_click").and_then(|v| v.as_bool()).unwrap_or(false);
      Ok(json!(platform.click_by_xpath(&handle, xpath, button, double_click)?))
    }
    "mouse.drag" => {
      let handle = req_handle(params)?;
      let from_x = req_f64(params, "from_x")?;
      let from_y = req_f64(params, "from_y")?;
      let to_x = req_f64(params, "to_x")?;
      let to_y = req_f64(params, "to_y")?;
      let button = req_button(params)?;
      Ok(json!(platform.drag(&handle, from_x, from_y, to_x, to_y, button)?))
    }
    "mouse.scroll" => {
      let handle = req_handle(params)?;
      let x = req_f64(params, "x")?;
      let y = req_f64(params, "y")?;
      let delta_x = req_i32(params, "delta_x")?;
      let delta_y = req_i32(params, "delta_y")?;
      Ok(json!(platform.scroll(&handle, x, y, delta_x, delta_y)?))
    }

    // Keyboard
    "keyboard.type_text" => {
      let handle = req_handle(params)?;
      let text = req_str(params, "text")?;
      let position = params.get("position").and_then(|v| {
        Some(InputPosition {
          x: v.get("x")?.as_f64()?,
          y: v.get("y")?.as_f64()?,
        })
      });
      Ok(json!(platform.type_text(&handle, text, position.as_ref())?))
    }
    "keyboard.type_text_by_xpath" => {
      let handle = req_handle(params)?;
      let xpath = req_str(params, "xpath")?;
      let text = req_str(params, "text")?;
      Ok(json!(platform.type_text_by_xpath(&handle, xpath, text)?))
    }
    "keyboard.type_raw" => {
      let handle = req_handle(params)?;
      let text = req_str(params, "text")?;
      platform.type_text_raw(&handle, text)?;
      Ok(json!(null))
    }
    "keyboard.send_keys" => {
      let keys: Vec<KeyCode> = serde_json::from_value(params.get("keys").cloned().unwrap_or_default())
        .map_err(|e| anyhow::anyhow!("invalid keys: {e}"))?;
      let modifiers: Option<Vec<KeyCode>> = params
        .get("modifiers")
        .filter(|v| !v.is_null())
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()
        .map_err(|e| anyhow::anyhow!("invalid modifiers: {e}"))?;
      platform.send_keys(&keys, modifiers.as_deref())?;
      Ok(json!(null))
    }
    "keyboard.key_down" => {
      let key: KeyCode = serde_json::from_value(params.get("key").cloned().unwrap_or_default())
        .map_err(|e| anyhow::anyhow!("invalid key: {e}"))?;
      platform.key_down(key)?;
      Ok(json!(null))
    }
    "keyboard.key_release" => {
      let key: KeyCode = serde_json::from_value(params.get("key").cloned().unwrap_or_default())
        .map_err(|e| anyhow::anyhow!("invalid key: {e}"))?;
      platform.key_release(key)?;
      Ok(json!(null))
    }

    // Element
    "element.find" => {
      let handle = req_handle(params)?;
      let xpath = req_str(params, "xpath")?;
      let elements = platform.find_elements_by_xpath(&handle, xpath)?;
      let infos: Vec<serde_json::Value> = elements
        .iter()
        .map(|e| {
          json!({
              "name": e.name(),
              "text": e.text(),
              "automation_id": e.automation_id(),
              "class_name": e.class_name(),
              "control_type": e.control_type(),
              "x": e.x(),
              "y": e.y(),
              "width": e.width(),
              "height": e.height(),
          })
        })
        .collect();
      Ok(json!(infos))
    }
    "element.page_source" => {
      let handle = req_handle(params)?;
      Ok(json!(platform.get_page_source(&handle)?))
    }
    "element.type_and_submit" => {
      let handle = req_handle(params)?;
      let xpath = req_str(params, "xpath")?;
      let text = req_str(params, "text")?;
      // Type into element (same as Windows: set_value or click+type)
      platform.type_text_by_xpath(&handle, xpath, text)?;
      // Press Enter (window should be focused from type_text_by_xpath)
      platform.send_keys(&[KeyCode::Enter], None)?;
      Ok(json!(null))
    }

    // Screenshot
    "screenshot.desktop" => {
      let config = parse_screenshot_config(params);
      let data = platform.take_desktop_screenshot(config.as_ref())?;
      use base64::Engine;
      let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
      Ok(json!({"data": encoded, "size": data.len()}))
    }
    "screenshot.window" => {
      let handle = req_handle(params)?;
      let config = parse_screenshot_config(params);
      let data = platform.take_window_screenshot(&handle, config.as_ref())?;
      use base64::Engine;
      let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
      Ok(json!({"data": encoded, "size": data.len()}))
    }

    // Process
    "process.launch" => {
      let path = req_str(params, "path")?;
      let timeout = params.get("wait_timeout_ms").and_then(|v| v.as_u64()).unwrap_or(5000) as u32;
      Ok(json!(platform.launch_app(path, timeout)?))
    }
    "process.terminate" => {
      let pid = req_u32(params, "pid")?;
      Ok(json!(platform.terminate_app(pid)?))
    }
    "process.terminate_by_name" => {
      let name = req_str(params, "name")?;
      Ok(json!(platform.terminate_apps_by_name(name)?))
    }
    "process.get_pids" => {
      let name = req_str(params, "name")?;
      Ok(json!(platform.get_process_ids_by_name(name)?))
    }
    "process.list" => {
      let processes = platform.list_processes()?;
      Ok(json!(processes))
    }
    "process.info" => {
      let pid = req_u32(params, "pid")?;
      Ok(json!(platform.get_process_info(pid)?))
    }

    // System
    "system.info" => Ok(json!(platform.get_system_info()?)),
    "system.screen_size" => {
      let (w, h) = platform.get_screen_size()?;
      Ok(json!({"width": w, "height": h}))
    }

    // Audio
    "audio.set_volume" => {
      let device_index = params.get("device_index").and_then(|v| v.as_u64()).map(|v| v as usize);
      let level = req_u64(params, "level")? as u32;
      platform.set_volume(device_index, level)?;
      Ok(json!(null))
    }
    "audio.get_volume" => {
      let device_index = params.get("device_index").and_then(|v| v.as_u64()).map(|v| v as usize);
      let level = platform.get_volume(device_index)?;
      Ok(json!({"level": level}))
    }
    "audio.set_mute" => {
      let device_index = params.get("device_index").and_then(|v| v.as_u64()).map(|v| v as usize);
      let muted = req_bool(params, "muted")?;
      platform.set_mute(device_index, muted)?;
      Ok(json!(null))
    }
    "audio.get_mute" => {
      let device_index = params.get("device_index").and_then(|v| v.as_u64()).map(|v| v as usize);
      let muted = platform.get_mute(device_index)?;
      Ok(json!({"muted": muted}))
    }
    "audio.set_default" => {
      let device_id = req_str(params, "device_id")?;
      platform.set_default_device(device_id)?;
      Ok(json!(null))
    }

    // Printer
    "printer.list" => Ok(json!(platform.list_printers()?)),
    "printer.print" => {
      let file_path = req_str(params, "file_path")?;
      let printer_name = req_str(params, "printer_name")?;
      platform.print_document(file_path, printer_name)?;
      Ok(json!(null))
    }

    // Bluetooth
    "bluetooth.scan" => Ok(json!(platform.scan_classic()?)),
    "bluetooth.scan_ble" => {
      let duration_ms = params.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(5000);
      Ok(json!(platform.scan_ble(duration_ms)?))
    }
    "bluetooth.list_pnp" => Ok(json!(platform.list_pnp()?)),

    // Service
    "service.list" => Ok(json!(platform.list_services()?)),
    "service.detail" => {
      let name = req_str(params, "name")?;
      Ok(json!(platform.get_service_detail(name)?))
    }
    "service.start" => {
      let name = req_str(params, "name")?;
      platform.start_service(name)?;
      Ok(json!(null))
    }
    "service.stop" => {
      let name = req_str(params, "name")?;
      platform.stop_service(name)?;
      Ok(json!(null))
    }
    "service.restart" => {
      let name = req_str(params, "name")?;
      platform.restart_service(name)?;
      Ok(json!(null))
    }

    // Terminal
    "terminal.execute" => {
      let shell_type = req_str(params, "shell_type")?;
      let command = req_str(params, "command")?;
      Ok(json!(platform.execute_command(shell_type, command)?))
    }

    // Software
    "software.get_installed" => {
      let filter = params.get("filter").and_then(|v| v.as_str());
      let mut software = platform.get_installed_software()?;
      if let Some(f) = filter {
        let f_lower = f.to_lowercase();
        software.retain(|s| {
          s.name.to_lowercase().contains(&f_lower)
            || s
              .publisher
              .as_ref()
              .is_some_and(|p| p.to_lowercase().contains(&f_lower))
        });
      }
      Ok(json!(software))
    }

    // Registry
    "startup.add" => {
      let name = req_str(params, "name")?;
      let command = req_str(params, "command")?;
      let location = params.get("location").and_then(|v| v.as_str()).unwrap_or("HKCU");
      let root = if location.to_uppercase() == "HKLM" {
        "HKLM"
      } else {
        "HKCU"
      };
      let key_path = format!("{root}\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
      platform.set_value(&key_path, name, "REG_SZ", command)?;
      Ok(json!(null))
    }
    "startup.remove" => {
      let name = req_str(params, "name")?;
      let location = params.get("location").and_then(|v| v.as_str()).unwrap_or("HKCU");
      let root = if location.to_uppercase() == "HKLM" {
        "HKLM"
      } else {
        "HKCU"
      };
      let key_path = format!("{root}\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
      platform.delete_value(&key_path, name)?;
      Ok(json!(null))
    }
    "registry.read" => {
      let key_path = req_str(params, "key_path")?;
      let value_name = params.get("value_name").and_then(|v| v.as_str());
      let (value_type, value) = platform.read_value(key_path, value_name)?;
      Ok(json!({"type": value_type, "value": value}))
    }
    "registry.list_subkeys" => {
      let key_path = req_str(params, "key_path")?;
      Ok(json!(platform.list_subkeys(key_path)?))
    }
    "registry.list_values" => {
      let key_path = req_str(params, "key_path")?;
      Ok(json!(platform.list_values(key_path)?))
    }
    "registry.set_value" => {
      let key_path = req_str(params, "key_path")?;
      let value_name = req_str(params, "value_name")?;
      let value_type = req_str(params, "value_type")?;
      let data = req_str(params, "data")?;
      platform.set_value(key_path, value_name, value_type, data)?;
      Ok(json!(null))
    }
    "registry.create_key" => {
      let key_path = req_str(params, "key_path")?;
      platform.create_key(key_path)?;
      Ok(json!(null))
    }
    "registry.delete_value" => {
      let key_path = req_str(params, "key_path")?;
      let value_name = req_str(params, "value_name")?;
      platform.delete_value(key_path, value_name)?;
      Ok(json!(null))
    }
    "registry.delete_key" => {
      let key_path = req_str(params, "key_path")?;
      platform.delete_key(key_path)?;
      Ok(json!(null))
    }

    _ => Err(anyhow::anyhow!("unknown method: {method}")),
  }
}

fn parse_screenshot_config(params: &serde_json::Value) -> Option<ScreenshotConfig> {
  let has_config = params.get("left").is_some()
    || params.get("top").is_some()
    || params.get("right").is_some()
    || params.get("bottom").is_some()
    || params.get("quality").is_some()
    || params.get("format").is_some();
  if !has_config {
    return None;
  }
  Some(ScreenshotConfig {
    left: params.get("left").and_then(|v| v.as_i64()).map(|v| v as i32),
    top: params.get("top").and_then(|v| v.as_i64()).map(|v| v as i32),
    right: params.get("right").and_then(|v| v.as_i64()).map(|v| v as i32),
    bottom: params.get("bottom").and_then(|v| v.as_i64()).map(|v| v as i32),
    quality: params.get("quality").and_then(|v| v.as_u64()).map(|v| v as u8),
    format: params.get("format").and_then(|v| v.as_str()).map(|s| s.to_string()),
  })
}

fn req_handle(params: &serde_json::Value) -> crate::error::Result<WindowHandle> {
  if let Some(hwnd) = params.get("handle").and_then(|v| v.as_i64()) {
    return Ok(WindowHandle(hwnd));
  }
  // Fallback: use the foreground window (Windows only)
  #[cfg(windows)]
  {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_invalid() {
      return Err(anyhow::anyhow!("no foreground window and no handle provided"));
    }
    Ok(WindowHandle(fg.0 as isize as i64))
  }
  #[cfg(not(windows))]
  {
    Err(anyhow::anyhow!(
      "no handle provided — on this platform a window handle is required"
    ))
  }
}

fn req_str<'a>(params: &'a serde_json::Value, key: &str) -> crate::error::Result<&'a str> {
  params
    .get(key)
    .and_then(|v| v.as_str())
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_i32(params: &serde_json::Value, key: &str) -> crate::error::Result<i32> {
  params
    .get(key)
    .and_then(|v| v.as_i64())
    .map(|v| v as i32)
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_f64(params: &serde_json::Value, key: &str) -> crate::error::Result<f64> {
  params
    .get(key)
    .and_then(|v| v.as_f64())
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_u32(params: &serde_json::Value, key: &str) -> crate::error::Result<u32> {
  params
    .get(key)
    .and_then(|v| v.as_u64())
    .map(|v| v as u32)
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_u64(params: &serde_json::Value, key: &str) -> crate::error::Result<u64> {
  params
    .get(key)
    .and_then(|v| v.as_u64())
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_bool(params: &serde_json::Value, key: &str) -> crate::error::Result<bool> {
  params
    .get(key)
    .and_then(|v| v.as_bool())
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_button(params: &serde_json::Value) -> crate::error::Result<MouseButton> {
  match params.get("button").and_then(|v| v.as_str()) {
    Some("left") | None => Ok(MouseButton::Left),
    Some("right") => Ok(MouseButton::Right),
    Some("middle") => Ok(MouseButton::Middle),
    Some(other) => Err(anyhow::anyhow!("unknown button: {other}")),
  }
}
