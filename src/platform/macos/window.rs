use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use core_graphics::display::CGDisplay;
use objc2_foundation::NSError;
use objc2_screen_capture_kit::SCShareableContent;

use crate::error::Result;
use crate::types::*;

fn cfstr(s: &str) -> CFString {
  CFString::new(s)
}

/// Fetch all windows via SCShareableContent (replaces deprecated CGWindowListCopyWindowInfo).
fn read_window_list(visible_only: bool) -> Result<Vec<WindowInfo>> {
  use block2::StackBlock;
  use std::sync::mpsc;

  let (tx, rx) = mpsc::channel();

  let handler = StackBlock::new(move |content: *mut SCShareableContent, _error: *mut NSError| {
    let mut windows = Vec::new();
    if !content.is_null() {
      let content_ref = unsafe { &*content };
      let sc_windows = unsafe { content_ref.windows() };
      for i in 0..sc_windows.len() {
        let w = sc_windows.objectAtIndex(i);
        let layer = unsafe { w.windowLayer() };
        if layer != 0 {
          continue;
        }
        let is_on_screen = unsafe { w.isOnScreen() };
        if visible_only && !is_on_screen {
          continue;
        }
        let title = unsafe { w.title() }
          .map(|s| s.to_string())
          .unwrap_or_default();
        let frame = unsafe { w.frame() };
        let (app_name, pid) = unsafe { w.owningApplication() }
          .map(|app| {
            let name = unsafe { app.applicationName() }.to_string();
            let pid = unsafe { app.processID() } as u32;
            (name, pid)
          })
          .unwrap_or_default();

        windows.push(WindowInfo {
          hwnd: unsafe { w.windowID() } as i64,
          parent_hwnd: 0,
          title,
          class_name: app_name.clone(),
          process_id: pid,
          process_name: app_name,
          is_visible: is_on_screen,
          is_minimized: false,
          is_maximized: false,
          is_topmost: false,
          is_tool_window: false,
          is_layered: false,
          is_no_activate: false,
          width: frame.size.width as i32,
          height: frame.size.height as i32,
          x: frame.origin.x as i32,
          y: frame.origin.y as i32,
        });
      }
    }
    let _ = tx.send(windows);
  });

  unsafe {
    SCShareableContent::getShareableContentWithCompletionHandler(&handler);
  }

  rx.recv()
    .map_err(|_| anyhow::anyhow!("shareable content channel closed"))
}

pub fn list_windows() -> Result<Vec<WindowInfo>> {
  read_window_list(false)
}

pub fn list_visible_windows() -> Result<Vec<WindowInfo>> {
  read_window_list(true)
}

pub fn find_windows_by_title(pattern: &str, process_name: Option<&str>) -> Result<Vec<WindowInfo>> {
  let pat = pattern.to_lowercase();
  Ok(
    list_windows()?
      .into_iter()
      .filter(|w| {
        let title_match = w.title.to_lowercase().contains(&pat);
        let proc_match = w.process_name.to_lowercase().contains(&pat);
        let pattern_match = title_match || proc_match;
        let filter_match =
          process_name.is_none_or(|p| w.process_name.to_lowercase().contains(&p.to_lowercase()));
        pattern_match && filter_match
      })
      .collect(),
  )
}

pub fn find_window_by_title(title: &str) -> Result<WindowHandle> {
  find_windows_by_title(title, None)?
    .into_iter()
    .find(|w| w.title.to_lowercase().contains(&title.to_lowercase()))
    .map(|w| WindowHandle(w.hwnd))
    .ok_or_else(|| anyhow::anyhow!("window not found: {title}"))
}

pub fn get_windows_by_process_id(pid: u32) -> Result<Vec<WindowInfo>> {
  Ok(list_windows()?.into_iter().filter(|w| w.process_id == pid).collect())
}

pub fn get_windows_by_process_id_with_title(pid: u32, pattern: &str, fuzzy: bool) -> Result<Vec<WindowInfo>> {
  let pat = pattern.to_lowercase();
  Ok(
    list_windows()?
      .into_iter()
      .filter(|w| {
        if w.process_id != pid {
          return false;
        }
        let t = w.title.to_lowercase();
        if fuzzy { t.contains(&pat) } else { t == pat }
      })
      .collect(),
  )
}

pub fn get_window_title(handle: &WindowHandle) -> Result<String> {
  list_windows()?
    .iter()
    .find(|w| w.hwnd == handle.0)
    .map(|w| w.title.clone())
    .ok_or_else(|| anyhow::anyhow!("window not found: {}", handle.0))
}

pub fn get_foreground_window() -> Result<WindowHandle> {
  let front_pid = get_frontmost_pid()?;
  let wins = list_visible_windows()?;
  wins
    .iter()
    .find(|w| w.process_id == front_pid)
    .map(|w| WindowHandle(w.hwnd))
    .or_else(|| wins.first().map(|w| WindowHandle(w.hwnd)))
    .ok_or_else(|| anyhow::anyhow!("no foreground window"))
}

// --- AXUIElement helpers ---

/// Get the frontmost application PID via NSWorkspace.
fn get_frontmost_pid() -> Result<u32> {
  unsafe {
    let cls = objc2::class!(NSWorkspace);
    let ws: *mut objc2::runtime::AnyObject = objc2::msg_send![cls, sharedWorkspace];
    if ws.is_null() {
      return Err(anyhow::anyhow!("sharedWorkspace is null"));
    }
    let front: *mut objc2::runtime::AnyObject = objc2::msg_send![ws, frontmostApplication];
    if front.is_null() {
      return Err(anyhow::anyhow!("frontmostApplication is null"));
    }
    let pid: i32 = objc2::msg_send![front, processIdentifier];
    Ok(pid as u32)
  }
}

/// Activate an application by PID.
fn activate_app(pid: u32) -> Result<()> {
  // NSRunningApplication::activateWithOptions is deprecated in macOS 14+ (no effect).
  // Use `open -a` which reliably activates the app from CLI context.
  let app_name = get_app_name_for_pid(pid)?;
  let output = std::process::Command::new("open")
    .args(["-a", &app_name])
    .output()
    .map_err(|e| anyhow::anyhow!("open -a failed: {e}"))?;
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(anyhow::anyhow!("open -a failed: {}", stderr));
  }
  std::thread::sleep(std::time::Duration::from_millis(200));
  Ok(())
}

fn get_app_name_for_pid(pid: u32) -> Result<String> {
  unsafe {
    let cls = objc2::class!(NSRunningApplication);
    let app: *mut objc2::runtime::AnyObject =
      objc2::msg_send![cls, runningApplicationWithProcessIdentifier: pid as i32];
    if app.is_null() {
      return Err(anyhow::anyhow!("app not found for pid {pid}"));
    }
    let name: *mut objc2::runtime::AnyObject = objc2::msg_send![app, localizedName];
    if name.is_null() {
      return Err(anyhow::anyhow!("app has no localizedName for pid {pid}"));
    }
    let utf8: *const std::ffi::c_char = objc2::msg_send![name, UTF8String];
    if utf8.is_null() {
      return Err(anyhow::anyhow!("UTF8String returned null for pid {pid}"));
    }
    let cstr = std::ffi::CStr::from_ptr(utf8);
    Ok(cstr.to_string_lossy().into_owned())
  }
}

/// Get the AXUIElement for the first (or matching) window of an application.
fn ax_get_window(app: &accessibility::AXUIElement, window_title: &str) -> Result<accessibility::AXUIElement> {
  use accessibility::AXAttribute;
  let windows_attr = app
    .attribute(&AXAttribute::new(&cfstr("AXWindows")))
    .map_err(|e| anyhow::anyhow!("failed to get AXWindows: {e:?}"))?;

  // windows_attr is a CFType that should be a CFArray of AXUIElement
  // We need to iterate through it
  let arr: CFArray<CFType> = unsafe { TCFType::wrap_under_get_rule(windows_attr.as_CFTypeRef() as *const _) };

  for i in 0..arr.len() {
    let Some(item) = arr.get(i) else { continue };
    // Each item is an AXUIElement (which is a CFType)
    // Try to get its title
    let win_elem: accessibility::AXUIElement = unsafe {
      std::mem::transmute(item.clone())
    };
    if let Ok(title_attr) = win_elem.attribute(&AXAttribute::title()) {
      if title_attr.to_string() == window_title {
        return Ok(win_elem);
      }
    }
  }

  // Fallback: return the first window
  if arr.len() > 0 {
    let item = arr.get(0).unwrap();
    let win_elem: accessibility::AXUIElement = unsafe {
      std::mem::transmute(item.clone())
    };
    return Ok(win_elem);
  }

  Err(anyhow::anyhow!("no windows found for application"))
}

/// Set an AX attribute on a window, finding it by title.
fn ax_set_window_attribute(
  pid: u32,
  window_title: &str,
  attr_name: &str,
  value: CFType,
) -> Result<()> {
  let app = accessibility::AXUIElement::application(pid as i32);
  let win = ax_get_window(&app, window_title)?;
  let attr_name_cf = cfstr(attr_name);
  unsafe {
    let err = accessibility_sys::AXUIElementSetAttributeValue(
      win.as_concrete_TypeRef(),
      attr_name_cf.as_concrete_TypeRef(),
      value.as_concrete_TypeRef(),
    );
    if err != 0 {
      return Err(anyhow::anyhow!("set_attribute({attr_name}) failed: AXError {err}"));
    }
  }
  Ok(())
}

/// Perform an AX action on a window.
fn ax_perform_action(pid: u32, window_title: &str, action: &str) -> Result<()> {
  let app = accessibility::AXUIElement::application(pid as i32);
  let win = ax_get_window(&app, window_title)?;
  let action_str = cfstr(action);
  unsafe {
    let err = accessibility_sys::AXUIElementPerformAction(
      win.as_concrete_TypeRef(),
      action_str.as_concrete_TypeRef(),
    );
    if err != 0 {
      return Err(anyhow::anyhow!("AXUIElementPerformAction({action}) failed: {err}"));
    }
  }
  Ok(())
}

// --- Window operations ---

pub fn focus_window(handle: &WindowHandle) -> Result<()> {
  let wins = list_windows()?;
  let win = wins
    .iter()
    .find(|w| w.hwnd == handle.0)
    .ok_or_else(|| anyhow::anyhow!("window not found: {}", handle.0))?;

  // Raise via AX, then activate the app
  let _ = ax_perform_action(win.process_id, &win.title, "AXRaise");
  activate_app(win.process_id)
}

pub fn move_window(handle: &WindowHandle, x: i32, y: i32) -> Result<()> {
  let wins = list_windows()?;
  let win = wins
    .iter()
    .find(|w| w.hwnd == handle.0)
    .ok_or_else(|| anyhow::anyhow!("window not found: {}", handle.0))?;

  let point = core_graphics::geometry::CGPoint { x: x as f64, y: y as f64 };
  unsafe {
    let ax_value = accessibility_sys::AXValueCreate(
      accessibility_sys::kAXValueTypeCGPoint,
      &point as *const _ as *const _,
    );
    if ax_value.is_null() {
      return Err(anyhow::anyhow!("AXValueCreate failed for position"));
    }
    let value: CFType = TCFType::wrap_under_create_rule(ax_value as core_foundation::base::CFTypeRef);
    ax_set_window_attribute(win.process_id, &win.title, "AXPosition", value)?;
  }
  Ok(())
}

pub fn resize_window(handle: &WindowHandle, width: i32, height: i32) -> Result<()> {
  let wins = list_windows()?;
  let win = wins
    .iter()
    .find(|w| w.hwnd == handle.0)
    .ok_or_else(|| anyhow::anyhow!("window not found: {}", handle.0))?;

  let size = core_graphics::geometry::CGSize { width: width as f64, height: height as f64 };
  unsafe {
    let ax_value = accessibility_sys::AXValueCreate(
      accessibility_sys::kAXValueTypeCGSize,
      &size as *const _ as *const _,
    );
    if ax_value.is_null() {
      return Err(anyhow::anyhow!("AXValueCreate failed for size"));
    }
    let value: CFType = TCFType::wrap_under_create_rule(ax_value as core_foundation::base::CFTypeRef);
    ax_set_window_attribute(win.process_id, &win.title, "AXSize", value)?;
  }
  Ok(())
}

pub fn set_window_rect(handle: &WindowHandle, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
  move_window(handle, x, y)?;
  resize_window(handle, width, height)
}

pub fn minimize_window(handle: &WindowHandle) -> Result<()> {
  let wins = list_windows()?;
  let win = wins
    .iter()
    .find(|w| w.hwnd == handle.0)
    .ok_or_else(|| anyhow::anyhow!("window not found: {}", handle.0))?;
  let value: CFType = unsafe {
    TCFType::wrap_under_get_rule(CFBoolean::from(true).as_concrete_TypeRef() as core_foundation::base::CFTypeRef)
  };
  ax_set_window_attribute(win.process_id, &win.title, "AXMinimized", value)
}

pub fn maximize_window(handle: &WindowHandle) -> Result<()> {
  let (sw, sh) = get_screen_size_native();
  let menu_h = get_menu_bar_height();
  set_window_rect(handle, 0, menu_h, sw, sh - menu_h)
}

pub fn restore_window(handle: &WindowHandle) -> Result<()> {
  let wins = list_windows()?;
  let win = wins
    .iter()
    .find(|w| w.hwnd == handle.0)
    .ok_or_else(|| anyhow::anyhow!("window not found: {}", handle.0))?;
  let value: CFType = unsafe {
    TCFType::wrap_under_get_rule(CFBoolean::from(false).as_concrete_TypeRef() as core_foundation::base::CFTypeRef)
  };
  ax_set_window_attribute(win.process_id, &win.title, "AXMinimized", value)
}

// --- Screen helpers ---

fn get_screen_size_native() -> (i32, i32) {
  let display = CGDisplay::main();
  (display.pixels_wide() as i32, display.pixels_high() as i32)
}

fn get_menu_bar_height() -> i32 {
  // Hardcoded: macOS menu bar is typically 25 pixels.
  // The NSScreen approach with objc2 is complex due to CGRect encoding issues.
  25
}
