use super::cache::SnapshotCache;
use super::protocol::{Request, Response};
use crate::platform::PlatformProvider;
use crate::types::*;
use serde_json::json;

pub fn handle_request<P: PlatformProvider + ?Sized>(platform: &P, cache: &SnapshotCache, req: Request) -> Response {
  let result = dispatch(platform, cache, &req.method, &req.params);
  match result {
    Ok(value) => Response::success(req.id, value),
    Err(e) => Response::error(req.id as u32, -1, e.to_string()),
  }
}

pub fn dispatch<P: PlatformProvider + ?Sized>(
  platform: &P,
  cache: &SnapshotCache,
  method: &str,
  params: &serde_json::Value,
) -> crate::error::Result<serde_json::Value> {
  match method {
    // ── Window ────────────────────────────────────────────────────────────────
    "window.list" => {
      let all = params["all"].as_bool().unwrap_or(false);
      let windows = if all {
        platform.list_windows()?
      } else {
        platform.list_visible_windows()?
      };
      Ok(json!(windows))
    }
    "window.find" => {
      let title = params["title"].as_str();
      let process = params["process"].as_str();
      let pid = params["pid"].as_u64().map(|v| v as u32);
      if let Some(p) = pid {
        return Ok(json!(platform.get_windows_by_process_id(p)?));
      }
      let (pattern, process_filter) = match (title, process) {
        (Some(t), p) => (t, p),
        (None, Some(p)) => ("", Some(p)),
        (None, None) => ("", None),
      };
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
      // Prefer pid-based focus for --app (works even when SCShareableContent is flaky).
      if let Some(name) = params["app"].as_str()
        && params["handle"].as_i64().is_none()
        && let Some(pid) = platform.resolve_app_pid(name)?
      {
        platform.focus_app(pid)?;
        return Ok(json!(null));
      }
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
      let handle = opt_handle(platform, params)?;
      let x = req_f64(params, "x")?;
      let y = req_f64(params, "y")?;
      let button = req_button(params)?;
      let double = params["double"].as_bool().unwrap_or(false);
      Ok(json!(platform.click(&handle, x, y, button, double)?))
    }
    "mouse.drag" => {
      let handle = opt_handle(platform, params)?;
      let x1 = req_f64(params, "x1")?;
      let y1 = req_f64(params, "y1")?;
      let x2 = req_f64(params, "x2")?;
      let y2 = req_f64(params, "y2")?;
      let button = req_button(params)?;
      Ok(json!(platform.drag(&handle, x1, y1, x2, y2, button)?))
    }
    "mouse.scroll" => {
      let handle = opt_handle(platform, params)?;
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
      let key: KeyCode =
        serde_json::from_value(params["key"].clone()).map_err(|e| anyhow::anyhow!("invalid key: {e}"))?;
      platform.key_down(key)?;
      Ok(json!(null))
    }
    "keyboard.key_up" => {
      let key: KeyCode =
        serde_json::from_value(params["key"].clone()).map_err(|e| anyhow::anyhow!("invalid key: {e}"))?;
      platform.key_release(key)?;
      Ok(json!(null))
    }

    // ── Element ───────────────────────────────────────────────────────────────
    "element.query" => {
      let scope = parse_scope(platform, params)?;
      let q = parse_query(params);
      let elements = platform.query_elements(scope.clone(), &q)?;
      let infos: Vec<serde_json::Value> = elements
        .iter()
        .enumerate()
        .map(|(i, e)| {
          let mut v = elem_to_json(e);
          v["ref"] = json!(format!("e{}", i + 1));
          v
        })
        .collect();
      cache.replace(scope, &elements);
      Ok(json!(infos))
    }
    "element.click" => {
      let button = req_button(params)?;
      let double = params["double"].as_bool().unwrap_or(false);

      // ref-based fast path: re-resolve the descriptor saved by the last
      // `element.query` and act on the freshly-found element.
      if let Some(ref_id) = params["ref"].as_str() {
        let (scope, elem) = cache.resolve(platform, ref_id)?;
        let cx = elem.x() as f64 + elem.width() as f64 / 2.0;
        let cy = elem.y() as f64 + elem.height() as f64 / 2.0;
        ensure_foreground(platform, &scope)?;
        let handle = WindowHandle(0);
        return Ok(json!(platform.click(&handle, cx, cy, button, double)?));
      }

      let scope = parse_scope(platform, params)?;
      let q = parse_query(params);
      let elem = platform.query_one(scope.clone(), &q)?;
      let cx = elem.x() as f64 + elem.width() as f64 / 2.0;
      let cy = elem.y() as f64 + elem.height() as f64 / 2.0;
      ensure_foreground(platform, &scope)?;
      // elem.x()/y() are screen-absolute pixels on both macOS (AX) and Windows
      // (UIA BoundingRectangle), so pass WindowHandle(0) to dispatch as
      // absolute-screen click. Passing the foreground HWND would route through
      // the Windows window-relative click path which expects 0-1 normalized
      // coords and rejects pixel values.
      let handle = WindowHandle(0);
      Ok(json!(platform.click(&handle, cx, cy, button, double)?))
    }
    "element.type" => {
      let value = req_str(params, "value")?;
      let enter = params["enter"].as_bool().unwrap_or(false);
      let clear = params["clear"].as_bool().unwrap_or(false);

      // ref-based fast path.
      if let Some(ref_id) = params["ref"].as_str() {
        let (scope, elem) = cache.resolve(platform, ref_id)?;
        ensure_foreground(platform, &scope)?;
        let _ = elem.set_focused();
        let pos = Some((
          elem.x() as f64 + elem.width() as f64 / 2.0,
          elem.y() as f64 + elem.height() as f64 / 2.0,
        ));
        platform.type_text(scope, value, pos, enter, clear)?;
        return Ok(json!(null));
      }

      let scope = parse_scope(platform, params)?;
      let q = parse_query(params);

      // Find all candidates and prefer one with a sane on-screen rect.
      // Qt apps often expose multiple "Edit"-like elements; some are off-screen
      // shadows (e.g. -1, 1057, ...). If the user passed `--nth N` we honour
      // it by indexing only into the visible subset (this matches what
      // `element find` shows).
      let candidates = platform.query_elements(scope.clone(), &q)?;
      if candidates.is_empty() {
        return Err(anyhow::anyhow!("no element matches the query"));
      }
      let visible: Vec<_> = candidates
        .iter()
        .filter(|e| e.x() >= 0 && e.y() >= 0 && e.width() > 8 && e.height() > 8)
        .collect();
      let pool = if visible.is_empty() {
        candidates.iter().collect()
      } else {
        visible
      };
      let nth = q.index.unwrap_or(0);
      let elem = pool
        .get(nth)
        .ok_or_else(|| anyhow::anyhow!("only {} matches; --nth {} out of range", pool.len(), nth))?;

      // 0. Force the target app to the foreground. Synthetic CGEvent keystrokes
      //    go to whichever app is frontmost; without this, when the CLI is run
      //    from a Terminal, the shell steals focus between commands.
      ensure_foreground(platform, &scope)?;

      // 1. AX focus (works for Cocoa, no-op for Qt customs).
      let _ = elem.set_focused();

      // 2. Click the element center to force Qt/Electron to take focus.
      let pos = Some((
        elem.x() as f64 + elem.width() as f64 / 2.0,
        elem.y() as f64 + elem.height() as f64 / 2.0,
      ));

      // 3. Synthesize keystrokes via CGEvent. type_text handles focus_window,
      //    optional mouse click, optional Cmd+A/Delete (clear), the typing,
      //    and optional Enter.
      platform.type_text(scope, value, pos, enter, clear)?;
      Ok(json!(null))
    }
    "element.tree" => {
      let scope = parse_scope(platform, params)?;
      let q = parse_query(params);
      Ok(json!(platform.render_tree(scope, &q)?))
    }
    "element.probe" => {
      let x = params["x"].as_i64().unwrap_or(0) as i32;
      let y = params["y"].as_i64().unwrap_or(0) as i32;
      Ok(json!(platform.probe_at_position(x, y)?))
    }
    "element.activate" => {
      // ref-based fast path.
      if let Some(ref_id) = params["ref"].as_str() {
        let (scope, elem) = cache.resolve(platform, ref_id)?;
        ensure_foreground(platform, &scope)?;
        elem.confirm()?;
        return Ok(json!(null));
      }

      let scope = parse_scope(platform, params)?;
      let q = parse_query(params);
      let elem = platform.query_one(scope.clone(), &q)?;
      ensure_foreground(platform, &scope)?;
      elem.confirm()?;
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
            || s
              .publisher
              .as_ref()
              .is_some_and(|p| p.to_lowercase().contains(&f_lower))
        });
      }
      Ok(json!(software))
    }

    // ── Registry / Startup ────────────────────────────────────────────────────
    "startup.add" => {
      let name = req_str(params, "name")?;
      let command = req_str(params, "command")?;
      let location = params["location"].as_str().unwrap_or("HKCU");
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
      let location = params["location"].as_str().unwrap_or("HKCU");
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
    if let Some(pid) = platform.resolve_app_pid(name)? {
      return Ok(ElementScope::Application(pid));
    }
    let wins = platform.find_windows_by_process(name)?;
    let w = wins
      .into_iter()
      .next()
      .ok_or_else(|| anyhow::anyhow!("app not found: {name}"))?;
    return Ok(ElementScope::Window(WindowHandle(w.hwnd)));
  }
  Ok(ElementScope::Foreground)
}

/// Force the target app to the foreground before injecting synthetic input.
///
/// Synthetic CGEvent keystrokes / clicks are delivered to whichever app is
/// frontmost at the moment of injection. When the CLI is invoked one command
/// at a time from a Terminal, the Terminal re-claims focus between commands,
/// so input meant for the target app gets swallowed by the shell. Every
/// input-injection method must call this first.
fn ensure_foreground<P: PlatformProvider + ?Sized>(platform: &P, scope: &ElementScope) -> crate::error::Result<()> {
  let pid = match scope {
    ElementScope::Application(pid) => Some(*pid),
    ElementScope::Window(h) => platform
      .find_windows_by_title("", None)
      .ok()
      .and_then(|wins| wins.into_iter().find(|w| w.hwnd == h.0).map(|w| w.process_id)),
    ElementScope::Foreground => None,
  };
  if let Some(pid) = pid {
    let _ = platform.focus_app(pid);
    // Give the WindowServer a moment to actually swap key window.
    std::thread::sleep(std::time::Duration::from_millis(120));
  }
  Ok(())
}

fn parse_query(params: &serde_json::Value) -> ElementQuery {
  ElementQuery {
    role: params["role"].as_str().map(|s| s.to_string()),
    text: params["text"].as_str().map(|s| s.to_string()),
    text_exact: params["text_exact"].as_bool().unwrap_or(false),
    index: params["index"].as_u64().map(|v| v as usize),
    max_depth: params["max_depth"].as_u64().map(|v| v as usize),
    include_hidden: params["include_hidden"].as_bool().unwrap_or(false),
    no_hit_test: params["no_hit_test"].as_bool().unwrap_or(false),
  }
}

fn elem_to_json(e: &Box<dyn crate::platform::Element>) -> serde_json::Value {
  json!({
    "name": e.name(),
    "text": e.text(),
    "automation_id": e.automation_id(),
    "class_name": e.class_name(),
    "control_type": e.control_type(),
    "help_text": e.help_text(),
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
    // Pick the largest visible window (skip tiny tray/popups).
    let w = wins
      .into_iter()
      .filter(|w| w.width > 50 && w.height > 50)
      .max_by_key(|w| (w.width as i64) * (w.height as i64))
      .ok_or_else(|| anyhow::anyhow!("no window for app: {app}"))?;
    return Ok(WindowHandle(w.hwnd));
  }
  platform.get_foreground_window()
}

/// Like req_handle, but returns WindowHandle(0) when no -w/--app provided.
/// Used by mouse.* where default coords are screen-absolute (NOT window-relative).
fn opt_handle<P: PlatformProvider + ?Sized>(
  platform: &P,
  params: &serde_json::Value,
) -> crate::error::Result<WindowHandle> {
  if params["handle"].as_i64().is_some() || params["app"].as_str().is_some() {
    return req_handle(platform, params);
  }
  Ok(WindowHandle(0))
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
    let canonical = normalize_key_alias(part);
    let key: KeyCode = serde_json::from_value(serde_json::Value::String(canonical.to_string()))
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

/// Map common cross-platform aliases (cmd, command, meta, super, option, opt,
/// control, return) onto canonical KeyCode variant names.
fn normalize_key_alias(part: &str) -> String {
  let lower = part.to_ascii_lowercase();
  match lower.as_str() {
    "cmd" | "command" | "meta" | "super" => return "Win".to_string(),
    "lcmd" | "lcommand" | "lmeta" => return "LWin".to_string(),
    "rcmd" | "rcommand" | "rmeta" => return "RWin".to_string(),
    "option" | "opt" => return "Alt".to_string(),
    "loption" | "lopt" => return "LAlt".to_string(),
    "roption" | "ropt" => return "RAlt".to_string(),
    "control" => return "Ctrl".to_string(),
    "lcontrol" => return "LCtrl".to_string(),
    "rcontrol" => return "RCtrl".to_string(),
    "return" => return "Enter".to_string(),
    "esc" => return "Escape".to_string(),
    "del" => return "Delete".to_string(),
    "ins" => return "Insert".to_string(),
    _ => {}
  }
  // Single ASCII letter → uppercase (a → A) so it deserializes to KeyCode::A.
  if part.len() == 1 {
    let c = part.chars().next().unwrap();
    if c.is_ascii_alphabetic() {
      return c.to_ascii_uppercase().to_string();
    }
  }
  part.to_string()
}
