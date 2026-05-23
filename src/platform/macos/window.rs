use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use objc2_app_kit::NSScreen;
use objc2_foundation::NSError;
use objc2_screen_capture_kit::SCShareableContent;

use crate::error::Result;
use crate::types::*;

fn cfstr(s: &str) -> CFString {
  CFString::new(s)
}

/// Return the logical (point-based) screen size — correct for Retina displays.
fn logical_screen_size() -> (f64, f64) {
  unsafe {
    let mtm = objc2::MainThreadMarker::new_unchecked();
    if let Some(screen) = NSScreen::mainScreen(mtm) {
      let frame = screen.frame();
      return (frame.size.width, frame.size.height);
    }
  }
  let d = core_graphics::display::CGDisplay::main();
  (d.pixels_wide() as f64, d.pixels_high() as f64)
}

/// Return menu-bar height in logical points via NSScreen.visibleFrame.
fn get_menu_bar_height() -> i32 {
  unsafe {
    let mtm = objc2::MainThreadMarker::new_unchecked();
    if let Some(screen) = NSScreen::mainScreen(mtm) {
      let full = screen.frame();
      let visible = screen.visibleFrame();
      // NSRect y=0 is bottom-left; menu bar is at the top.
      // visible.origin.y = dock height; full.size.height - visible.size.height - visible.origin.y = menu bar height
      let menu_h = full.size.height - (visible.origin.y + visible.size.height);
      return menu_h.max(0.0) as i32;
    }
  }
  25 // reasonable fallback
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

  // BUG-03: Add 5s timeout to prevent permanent block if Screen Recording denied
  match rx.recv_timeout(std::time::Duration::from_secs(5)) {
    Ok(windows) => Ok(windows),
    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
      Err(anyhow::anyhow!(crate::error::PlatformError::ScreenRecordingPermissionDenied))
    }
    Err(e) => Err(anyhow::anyhow!("shareable content channel error: {e}")),
  }
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
        // BUG-16: title-only match (no process name fallback)
        let title_match = w.title.to_lowercase().contains(&pat);
        let proc_filter =
          process_name.is_none_or(|p| w.process_name.to_lowercase().contains(&p.to_lowercase()));
        title_match && proc_filter
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

/// Get the frontmost application PID via NSWorkspace.
fn get_frontmost_pid() -> Result<u32> {
  let ws = unsafe { objc2_app_kit::NSWorkspace::sharedWorkspace() };
  let front = unsafe { ws.frontmostApplication() }
    .ok_or_else(|| anyhow::anyhow!("frontmostApplication is null"))?;
  Ok(unsafe { front.processIdentifier() } as u32)
}

/// Activate an application by PID using NSRunningApplication typed API.
fn activate_app(pid: u32) -> Result<()> {
  use accessibility::action::AXUIElementActions;

  let app = accessibility::AXUIElement::application(pid as i32);
  let _ = app.raise();

  if let Some(ns_app) =
    unsafe { objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32) }
  {
    // macOS 14+: prefer the no-option `activate()`, but `activateWithOptions:` still works
    // and reliably brings the app to the foreground from a CLI context (which has no
    // currently-active app of its own).
    unsafe {
      #[allow(deprecated)]
      ns_app.activateWithOptions(
        objc2_app_kit::NSApplicationActivationOptions::ActivateIgnoringOtherApps
          | objc2_app_kit::NSApplicationActivationOptions::ActivateAllWindows,
      );
    }
  }

  std::thread::sleep(std::time::Duration::from_millis(300));
  Ok(())
}

/// Activate an app by pid and raise its first AX window — does not require a
/// SCShareableContent window list, so it works even when SC content lookup is flaky.
pub fn focus_app_by_pid(pid: u32) -> Result<()> {
  use accessibility::AXAttribute;
  let app = accessibility::AXUIElement::application(pid as i32);
  let wins = get_ax_windows(&app);
  if let Some(first) = wins.first() {
    let action_str = cfstr("AXRaise");
    unsafe {
      let _ = accessibility_sys::AXUIElementPerformAction(
        first.as_concrete_TypeRef(),
        action_str.as_concrete_TypeRef(),
      );
    }
    // Also mark as main/focused
    let _ = first.set_attribute(&AXAttribute::main(), true);
  }
  activate_app(pid)?;

  // macOS 14+: NSRunningApplication.activate() is a no-op for non-bundled CLI
  // callers. Fall back to AppleScript via System Events, which has the
  // necessary "frontmost" privilege regardless of the caller's bundle status.
  // Confirms the activation actually took effect by checking frontmost pid.
  std::thread::sleep(std::time::Duration::from_millis(100));
  let frontmost = frontmost_pid().unwrap_or(0);
  if frontmost != pid {
    let _ = std::process::Command::new("osascript")
      .args([
        "-e",
        &format!(
          "tell application \"System Events\" to set frontmost of (first process whose unix id is {pid}) to true"
        ),
      ])
      .output();
    std::thread::sleep(std::time::Duration::from_millis(200));
  }
  Ok(())
}

/// Return the pid of the current frontmost application, if any.
fn frontmost_pid() -> Option<u32> {
  use objc2_app_kit::NSWorkspace;
  unsafe {
    let ws = NSWorkspace::sharedWorkspace();
    ws.frontmostApplication().map(|app| app.processIdentifier() as u32)
  }
}

/// Get all AX windows for an application element as a Vec<AXUIElement>.
fn get_ax_windows(app: &accessibility::AXUIElement) -> Vec<accessibility::AXUIElement> {
  use accessibility::AXAttribute;
  let Ok(windows_val) = app.attribute(&AXAttribute::new(&cfstr("AXWindows"))) else {
    return Vec::new();
  };
  let arr: CFArray<CFType> =
    unsafe { TCFType::wrap_under_get_rule(windows_val.as_CFTypeRef() as *const _) };
  let ax_type_id = unsafe { accessibility_sys::AXUIElementGetTypeID() };
  let mut result = Vec::new();
  for i in 0..arr.len() {
    let Some(item) = arr.get(i) else { continue };
    if unsafe { core_foundation::base::CFGetTypeID(item.as_CFTypeRef() as *const _) } == ax_type_id {
      let elem: accessibility::AXUIElement =
        unsafe { TCFType::wrap_under_get_rule(item.as_CFTypeRef() as accessibility_sys::AXUIElementRef) };
      result.push(elem);
    }
  }
  result
}

/// Find the AX window element for a given SCWindow ID (BUG-05).
/// Uses private `_AXWindowID` attribute; falls back to first window.
pub fn ax_window_for_id(pid: u32, window_id: i64) -> Result<accessibility::AXUIElement> {
  use accessibility::AXAttribute;
  let app = accessibility::AXUIElement::application(pid as i32);
  let windows = get_ax_windows(&app);

  for win in &windows {
    if let Ok(id_val) = win.attribute(&AXAttribute::new(&cfstr("_AXWindowID"))) {
      if id_val.instance_of::<core_foundation::number::CFNumber>() {
        let num: core_foundation::number::CFNumber =
          unsafe { TCFType::wrap_under_get_rule(id_val.as_CFTypeRef() as *const _) };
        if let Some(id) = num.to_i64() {
          if id == window_id {
            return Ok(win.clone());
          }
        }
      }
    }
  }

  // Fallback: return the first window
  windows
    .into_iter()
    .next()
    .ok_or_else(|| anyhow::anyhow!("no windows found for pid {pid}"))
}

/// Set an AX attribute on a window identified by its SCWindow handle.
fn ax_set_window_attribute_by_id(pid: u32, window_id: i64, attr_name: &str, value: CFType) -> Result<()> {
  let win = ax_window_for_id(pid, window_id)?;
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

// --- Window operations ---

pub fn focus_window(handle: &WindowHandle) -> Result<()> {
  let wins = list_windows()?;
  let win = wins
    .iter()
    .find(|w| w.hwnd == handle.0)
    .ok_or_else(|| anyhow::anyhow!("window not found: {}", handle.0))?;

  // BUG-05: Match by windowID, not title — works for untitled windows.
  let ax_win = ax_window_for_id(win.process_id, win.hwnd);
  if let Ok(ax_win) = ax_win {
    let action_str = cfstr("AXRaise");
    unsafe {
      let _ = accessibility_sys::AXUIElementPerformAction(
        ax_win.as_concrete_TypeRef(),
        action_str.as_concrete_TypeRef(),
      );
    }
  }
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
    ax_set_window_attribute_by_id(win.process_id, win.hwnd, "AXPosition", value)?;
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
    ax_set_window_attribute_by_id(win.process_id, win.hwnd, "AXSize", value)?;
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
  ax_set_window_attribute_by_id(win.process_id, win.hwnd, "AXMinimized", value)
}

pub fn maximize_window(handle: &WindowHandle) -> Result<()> {
  let (sw, sh) = logical_screen_size();
  let menu_h = get_menu_bar_height();
  set_window_rect(handle, 0, menu_h, sw as i32, sh as i32 - menu_h)
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
  ax_set_window_attribute_by_id(win.process_id, win.hwnd, "AXMinimized", value)
}
