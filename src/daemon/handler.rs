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
    // ── Window ────────────────────────────────────────────────────────────────
    "window.list" => {
      let all = params["all"].as_bool().unwrap_or(false);
      let windows = if all { platform.list_windows()? } else { platform.list_visible_windows()? };
      Ok(json!(windows))
    }
    "window.find" => {
      let title = params["title"].as_str();
      let process = params["process"].as_str();
      let pid = params["pid"].as_u64().map(|v| v as u32);
      if let Some(p) = pid {
        return Ok(json!(platform.get_windows_by_process_id(p)?));
      }
      let (pattern, process_filter) = match (title, process) { (Some(t), p) => (t, p), (None, Some(p)) => ("", Some(p)), (None, None) => ("", None), };
      Ok(json!(platform.find_windows_by_title(pattern, process_filter)?))
    }
    "window.info" => {
      let handle = req_handle(platform, params)?;
      let windows = platform.list_windows()?;
      let hwnd_val = handle.0;
      let info = windows
        .iter()
        .find(|w| w.hwnd == hwnd_val)
        .ok_or_else(|| anyhow::anyhow!("window not found: {hwnd_val}"))?;
      Ok(json!(info))
    }
    "window.focus" => {
      let handle = req_handle(platform, params)?;
      platform.focus_window(&handle)?;
      Ok(json!(null))
    }
    "window.move" => {
      let handle = req_handle(platform, params)?;
      let x = req_i32(params, "x")?;
      let y = req_i32(params, "y")?;
      platform.move_window(&handle, x, y)?;
      Ok(json!(null))
    }
    "window.resize" => {
      let handle = req_handle(platform, params)?;
      let w = req_i32(params, "w")?;
      let h = req_i32(params, "h")?;
      platform.resize_window(&handle, w, h)?;
      Ok(json!(null))
    }
    "window.rect" => {
      let handle = req_handle(platform, params)?;
      let x = req_i32(params, "x")?;
      let y = req_i32(params, "y")?;
      let w = req_i32(params, "w")?;
      let h = req_i32(params, "h")?;
      platform.set_window_rect(&handle, x, y, w, h)?;
      Ok(json!(null))
    }
    "window.state" => {
      let handle = req_handle(platform, params)?;
      let op = req_str(params, "op")?;
      match op {
        "minimize" => platform.minimize_window(&handle)?,
        "maximize" => platform.maximize_window(&handle)?,
        "restore" => platform.restore_window(&handle)?,
        other => return Err(anyhow::anyhow!("unknown window state op: {other}")),
      }
      Ok(json!(null))
    }
    "window.foreground" => {
      let handle = platform.get_foreground_window()?;
      let windows = platform.list_windows()?;
      let info = windows
        .iter()
        .find(|w| w.hwnd == handle.0)
        .ok_or_else(|| anyhow::anyhow!("foreground window not found in list"))?;
      Ok(json!(info))
    }

    // ── Mouse ─────────────────────────────────────────────────────────────────
    "mouse.move" => {
      let x = req_i32(params, "x")?;
      let y = req_i32(params, "y")?;
      platform.move_cursor(x, y)?;
      Ok(json!(null))
    }
    "mouse.pos" => {
      let (x, y) = platform.get_cursor_position()?;
      Ok(json!({"x": x, "y": y}))
    }
    "mouse.click" => {
      let handle = req_handle(platform, params)?;
      let x = req_f64(params, "x")?;
      let y = req_f64(params, "y")?;
      let button = req_button(params)?;
      let double = params["double"].as_bool().unwrap_or(false);
      Ok(json!(platform.click(&handle, x, y, button, double)?))
    }
    "mouse.drag" => {
      let handle = req_handle(platform, params)?;
      let x1 = req_f64(params, "x1")?;
      let y1 = req_f64(params, "y1")?;
      let x2 = req_f64(params, "x2")?;
      let y2 = req_f64(params, "y2")?;
      let button = req_button(params)?;
      Ok(json!(platform.drag(&handle, x1, y1, x2, y2, button)?))
    }
    "mouse.scroll" => {
      let handle = req_handle(platform, params)?;
      let x = req_f64(params, "x")?;
      let y = req_f64(params, "y")?;
      let dx = params["dx"].as_i64().unwrap_or(0) as i32;
      let dy = params["dy"].as_i64().unwrap_or(0) as i32;
      Ok(json!(platform.scroll(&handle, x, y, dx, dy)?))
    }

    // ── Keyboard ──────────────────────────────────────────────────────────────
    "keyboard.type" => {
      let scope = parse_scope(platform, params)?;
      let text = req_str(params, "text")?;
      let position = if let (Some(x), Some(y)) = (params["x"].as_f64(), params["y"].as_f64()) {
        Some((x, y))
      } else {
        None
      };
      let enter = params["enter"].as_bool().unwrap_or(false);
      let clear = params["clear"].as_bool().unwrap_or(false);
      platform.type_text(scope, text, position, enter, clear)?;
      Ok(json!(null))
    }
    "keyboard.press" => {
      let combo = req_str(params, "combo")?;
      let (key_codes, modifiers) = parse_key_combo(combo)?;
      platform.send_keys(&key_codes, if modifiers.is_empty() { None } else { Some(&modifiers) })?;
      Ok(json!(null))
    }
    "keyboard.key_down" => {
      let key: KeyCode = serde_json::from_value(params["key"].clone())
        .map_err(|e| anyhow::anyhow!("invalid key: {e}"))?;
      platform.key_down(key)?;
      Ok(json!(null))
    }
    "keyboard.key_up" => {
      let key: KeyCode = serde_json::from_value(params["key"].clone())
        .map_err(|e| anyhow::anyhow!("invalid key: {e}"))?;
      platform.key_release(key)?;
      Ok(json!(null))
    }

    // ── Element ───────────────────────────────────────────────────────────────
    "element.query" => {
      let scope = parse_scope(platform, params)?;
      let q = parse_query(params);
      let elements = platform.query_elements(scope, &q)?;
      let infos: Vec<serde_json::Value> = elements.iter().map(elem_to_json).collect();
      Ok(json!(infos))
    }
    "element.click" => {
      let scope = parse_scope(platform, params)?;
      let q = parse_query(params);
      let button = req_button(params)?;
      let double = params["double"].as_bool().unwrap_or(false);
      let elem = platform.query_one(scope, &q)?;
      let cx = elem.x() as f64 + elem.width() as f64 / 2.0;
      let cy = elem.y() as f64 + elem.height() as f64 / 2.0;
      let handle = platform.get_foreground_window()?;
      Ok(json!(platform.click(&handle, cx, cy, button, double)?))
    }
    "element.type" => {
      let scope = parse_scope(platform, params)?;
      let q = parse_query(params);
      let value = req_str(params, "value")?;
      let enter = params["enter"].as_bool().unwrap_or(false);
      let clear = params["clear"].as_bool().unwrap_or(false);
      let elem = platform.query_one(scope.clone(), &q)?;
      let pos = Some((
        elem.x() as f64 + elem.width() as f64 / 2.0,
        elem.y() as f64 + elem.height() as f64 / 2.0,
      ));
      platform.type_text(scope, value, pos, enter, clear)?;
      Ok(json!(null))
    }
    "element.tree" => {
      let handle = req_handle(platform, params)?;
      Ok(json!(platform.get_page_source(&handle)?))
    }
    "element.probe" => {
      let x = params["x"].as_i64().unwrap_or(0) as i32;
      let y = params["y"].as_i64().unwrap_or(0) as i32;
      Ok(json!(platform.probe_at_position(x, y)?))
    }
    "element.activate" => {
      let scope = parse_scope(platform, params)?;
      let q = parse_query(params);
      let elem = platform.query_one(scope, &q)?;
      elem.confirm()?;
      Ok(json!(null))
    }
    "element.xpath_query" => {
      let scope = parse_scope(platform, params)?;
      let xpath = req_str(params, "xpath")?;
      let elements = platform.find_by_xpath(scope, xpath)?;
      let infos: Vec<serde_json::Value> = elements.iter().map(elem_to_json).collect();
      Ok(json!(infos))
    }
    "element.xpath_click" => {
      let scope = parse_scope(platform, params)?;
      let xpath = req_str(params, "xpath")?;
      let button = req_button(params)?;
      let double = params["double"].as_bool().unwrap_or(false);
      let elements = platform.find_by_xpath(scope, xpath)?;
      let elem = elements.into_iter().next().ok_or_else(|| anyhow::anyhow!("no element found for xpath"))?;
      let cx = elem.x() as f64 + elem.width() as f64 / 2.0;
      let cy = elem.y() as f64 + elem.height() as f64 / 2.0;
      let handle = platform.get_foreground_window()?;
      Ok(json!(platform.click(&handle, cx, cy, button, double)?))
    }
    "element.xpath_type" => {
      let scope = parse_scope(platform, params)?;
      let xpath = req_str(params, "xpath")?;
      let value = req_str(params, "value")?;
      let enter = params["enter"].as_bool().unwrap_or(false);
      let clear = params["clear"].as_bool().unwrap_or(false);
      let elements = platform.find_by_xpath(scope.clone(), xpath)?;
      let elem = elements.into_iter().next().ok_or_else(|| anyhow::anyhow!("no element found for xpath"))?;
      let pos = Some((
        elem.x() as f64 + elem.width() as f64 / 2.0,
        elem.y() as f64 + elem.height() as f64 / 2.0,
      ));
      platform.type_text(scope, value, pos, enter, clear)?;
      Ok(json!(null))
    }

    // ── Screenshot ────────────────────────────────────────────────────────────
    "screenshot.desktop" => {
      let config = parse_screenshot_config(params);
      let data = platform.take_desktop_screenshot(config.as_ref())?;
      use base64::Engine;
      let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
      Ok(json!({"data": encoded, "size": data.len()}))
    }
    "screenshot.window" => {
      let handle = req_handle(platform, params)?;
      let config = parse_screenshot_config(params);
      let data = platform.take_window_screenshot(&handle, config.as_ref())?;
      use base64::Engine;
      let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
      Ok(json!({"data": encoded, "size": data.len()}))
    }

    // ── Process ───────────────────────────────────────────────────────────────
    "process.launch" => {
      let wait = params["wait"].as_bool().unwrap_or(false);
      if let Some(path) = params["path"].as_str() {
        let pid = if wait {
          platform.launch_app_async(path, true)?
        } else {
          platform.launch_app(path, 0)?
        };
        Ok(json!({"pid": pid}))
      } else if let Some(app) = params["app"].as_str() {
        let pid = platform.launch_app_async(app, wait)?;
        Ok(json!({"pid": pid}))
      } else {
        Err(anyhow::anyhow!("missing 'path' or 'app' param"))
      }
    }
    "process.kill" => {
      if let Some(pid) = params["pid"].as_u64() {
        platform.terminate_app(pid as u32)?;
      } else if let Some(name) = params["name"].as_str() {
        platform.terminate_apps_by_name(name)?;
      } else {
        return Err(anyhow::anyhow!("missing 'pid' or 'name' param"));
      }
      Ok(json!(null))
    }
    "process.find" => {
      let name = params["name"].as_str();
      let pid = params["pid"].as_u64().map(|v| v as u32);
      let processes = platform.list_processes()?;
      let filtered: Vec<_> = processes
        .into_iter()
        .filter(|p| {
          let name_match = name.map_or(true, |n| p.name.to_lowercase().contains(&n.to_lowercase()));
          let pid_match = pid.map_or(true, |id| p.pid == id);
          name_match && pid_match
        })
        .collect();
      Ok(json!(filtered))
    }
    "process.list" => Ok(json!(platform.list_processes()?)),

    // ── System ────────────────────────────────────────────────────────────────
    "system.info" => Ok(json!(platform.get_system_info()?)),
    "system.screen_size" => {
      let (w, h) = platform.get_screen_size()?;
      Ok(json!({"width": w, "height": h}))
    }

    // ── Audio ─────────────────────────────────────────────────────────────────
    "audio.set_volume" => {
      let device_index = params["device_index"].as_u64().map(|v| v as usize);
      let level = req_u64(params, "level")? as u32;
      platform.set_volume(device_index, level)?;
      Ok(json!(null))
    }
    "audio.get_volume" => {
      let device_index = params["device_index"].as_u64().map(|v| v as usize);
      let level = platform.get_volume(device_index)?;
      Ok(json!({"level": level}))
    }
    "audio.set_mute" => {
      let device_index = params["device_index"].as_u64().map(|v| v as usize);
      let muted = req_bool(params, "muted")?;
      platform.set_mute(device_index, muted)?;
      Ok(json!(null))
    }
    "audio.get_mute" => {
      let device_index = params["device_index"].as_u64().map(|v| v as usize);
      let muted = platform.get_mute(device_index)?;
      Ok(json!({"muted": muted}))
    }
    "audio.set_default" => {
      let device_id = req_str(params, "device_id")?;
      platform.set_default_device(device_id)?;
      Ok(json!(null))
    }

    // ── Printer ───────────────────────────────────────────────────────────────
    "printer.list" => Ok(json!(platform.list_printers()?)),
    "printer.print" => {
      let file_path = req_str(params, "file_path")?;
      let printer_name = req_str(params, "printer_name")?;
      platform.print_document(file_path, printer_name)?;
      Ok(json!(null))
    }

    // ── Bluetooth ─────────────────────────────────────────────────────────────
    "bluetooth.scan" => Ok(json!(platform.scan_classic()?)),
    "bluetooth.scan_ble" => {
      let duration_ms = params["duration_ms"].as_u64().unwrap_or(5000);
      Ok(json!(platform.scan_ble(duration_ms)?))
    }
    "bluetooth.list_pnp" => Ok(json!(platform.list_pnp()?)),

    // ── Service ───────────────────────────────────────────────────────────────
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

    // ── Terminal ──────────────────────────────────────────────────────────────
    "terminal.execute" => {
      let shell_type = req_str(params, "shell_type")?;
      let command = req_str(params, "command")?;
      Ok(json!(platform.execute_command(shell_type, command)?))
    }

    // ── Software ──────────────────────────────────────────────────────────────
    "software.get_installed" => {
      let filter = params["filter"].as_str();
      let mut software = platform.get_installed_software()?;
      if let Some(f) = filter {
        let f_lower = f.to_lowercase();
        software.retain(|s| {
          s.name.to_lowercase().contains(&f_lower)
            || s.publisher.as_ref().is_some_and(|p| p.to_lowercase().contains(&f_lower))
        });
      }
      Ok(json!(software))
    }

    // ── Registry / Startup ────────────────────────────────────────────────────
    "startup.add" => {
      let name = req_str(params, "name")?;
      let command = req_str(params, "command")?;
      let location = params["location"].as_str().unwrap_or("HKCU");
      let root = if location.to_uppercase() == "HKLM" { "HKLM" } else { "HKCU" };
      let key_path = format!("{root}\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
      platform.set_value(&key_path, name, "REG_SZ", command)?;
      Ok(json!(null))
    }
    "startup.remove" => {
      let name = req_str(params, "name")?;
      let location = params["location"].as_str().unwrap_or("HKCU");
      let root = if location.to_uppercase() == "HKLM" { "HKLM" } else { "HKCU" };
      let key_path = format!("{root}\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
      platform.delete_value(&key_path, name)?;
      Ok(json!(null))
    }
    "registry.read" => {
      let key_path = req_str(params, "key_path")?;
      let value_name = params["value_name"].as_str();
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_scope<P: PlatformProvider + ?Sized>(
  platform: &P,
  params: &serde_json::Value,
) -> crate::error::Result<ElementScope> {
  if let Some(h) = params["handle"].as_i64() {
    return Ok(ElementScope::Window(WindowHandle(h)));
  }
  if let Some(name) = params["app"].as_str() {
    let wins = platform.find_windows_by_process(name)?;
    let w = wins.into_iter().next().ok_or_else(|| anyhow::anyhow!("app not found: {name}"))?;
    return Ok(ElementScope::Window(WindowHandle(w.hwnd)));
  }
  Ok(ElementScope::Foreground)
}

fn parse_query(params: &serde_json::Value) -> ElementQuery {
  ElementQuery {
    role: params["role"].as_str().map(|s| s.to_string()),
    text: params["text"].as_str().map(|s| s.to_string()),
    text_exact: params["text_exact"].as_bool().unwrap_or(false),
    index: params["index"].as_u64().map(|v| v as usize),
    max_depth: params["max_depth"].as_u64().map(|v| v as usize),
  }
}

fn elem_to_json(e: &Box<dyn crate::platform::Element>) -> serde_json::Value {
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
}

fn parse_screenshot_config(params: &serde_json::Value) -> Option<ScreenshotConfig> {
  let has_config = params["left"].is_number()
    || params["top"].is_number()
    || params["right"].is_number()
    || params["bottom"].is_number()
    || params["quality"].is_number()
    || params["format"].is_string();
  if !has_config {
    return None;
  }
  Some(ScreenshotConfig {
    left: params["left"].as_i64().map(|v| v as i32),
    top: params["top"].as_i64().map(|v| v as i32),
    right: params["right"].as_i64().map(|v| v as i32),
    bottom: params["bottom"].as_i64().map(|v| v as i32),
    quality: params["quality"].as_u64().map(|v| v as u8),
    format: params["format"].as_str().map(|s| s.to_string()),
  })
}

fn req_handle<P: PlatformProvider + ?Sized>(
  platform: &P,
  params: &serde_json::Value,
) -> crate::error::Result<WindowHandle> {
  if let Some(hwnd) = params["handle"].as_i64() {
    return Ok(WindowHandle(hwnd));
  }
  if let Some(app) = params["app"].as_str() {
    let wins = platform.find_windows_by_process(app)?;
    let w = wins.into_iter().next().ok_or_else(|| anyhow::anyhow!("no window for app: {app}"))?;
    return Ok(WindowHandle(w.hwnd));
  }
  platform.get_foreground_window()
}

fn req_str<'a>(params: &'a serde_json::Value, key: &str) -> crate::error::Result<&'a str> {
  params[key]
    .as_str()
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_i32(params: &serde_json::Value, key: &str) -> crate::error::Result<i32> {
  params[key]
    .as_i64()
    .map(|v| v as i32)
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_f64(params: &serde_json::Value, key: &str) -> crate::error::Result<f64> {
  params[key]
    .as_f64()
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_u64(params: &serde_json::Value, key: &str) -> crate::error::Result<u64> {
  params[key]
    .as_u64()
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_bool(params: &serde_json::Value, key: &str) -> crate::error::Result<bool> {
  params[key]
    .as_bool()
    .ok_or_else(|| anyhow::anyhow!("missing '{key}' param"))
}

fn req_button(params: &serde_json::Value) -> crate::error::Result<MouseButton> {
  match params["button"].as_str() {
    Some("left") | None => Ok(MouseButton::Left),
    Some("right") => Ok(MouseButton::Right),
    Some("middle") => Ok(MouseButton::Middle),
    Some(other) => Err(anyhow::anyhow!("unknown button: {other}")),
  }
}

/// Parse a combo string like "Ctrl+C" or "Alt+F4" into (keys, modifiers).
fn parse_key_combo(combo: &str) -> crate::error::Result<(Vec<KeyCode>, Vec<KeyCode>)> {
  let parts: Vec<&str> = combo.split('+').map(|s| s.trim()).collect();
  let mut modifiers = Vec::new();
  let mut main_keys = Vec::new();
  for part in &parts {
    let key: KeyCode = serde_json::from_value(serde_json::Value::String(part.to_string()))
      .map_err(|_| anyhow::anyhow!("unknown key: {part}"))?;
    if is_modifier_key(&key) {
      modifiers.push(key);
    } else {
      main_keys.push(key);
    }
  }
  if main_keys.is_empty() {
    if let Some(last) = modifiers.pop() {
      main_keys.push(last);
    }
  }
  Ok((main_keys, modifiers))
}

fn is_modifier_key(key: &KeyCode) -> bool {
  matches!(
    key,
    KeyCode::Ctrl
      | KeyCode::LCtrl
      | KeyCode::RCtrl
      | KeyCode::Shift
      | KeyCode::LShift
      | KeyCode::RShift
      | KeyCode::Alt
      | KeyCode::LAlt
      | KeyCode::RAlt
      | KeyCode::Win
      | KeyCode::LWin
      | KeyCode::RWin
  )
}
