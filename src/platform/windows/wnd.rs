use crate::platform::windows::process::get_process_name;
use crate::types::{WindowHandle, WindowInfo, WindowRECT};
use anyhow::{Context, Result, anyhow, bail};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::System::Threading::AttachThreadInput;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

// === Window utils ===

pub fn bring_window_to_front(hwnd: HWND) -> bool {
  unsafe {
    let foreground = GetForegroundWindow();
    if foreground == hwnd {
      return false;
    }
    if is_window_or_child_in_foreground(hwnd, foreground) {
      return false;
    }
    perform_window_bring_to_front(hwnd, foreground)
  }
}

fn is_window_or_child_in_foreground(target: HWND, foreground: HWND) -> bool {
  unsafe {
    if IsChild(target, foreground).as_bool() {
      return true;
    }
    let mut parent = GetParent(foreground).unwrap_or(HWND(std::ptr::null_mut()));
    while !parent.0.is_null() {
      if parent == target {
        return true;
      }
      parent = GetParent(parent).unwrap_or(HWND(std::ptr::null_mut()));
    }
    false
  }
}

fn perform_window_bring_to_front(hwnd: HWND, foreground: HWND) -> bool {
  unsafe {
    let fg_thread = GetWindowThreadProcessId(foreground, None);
    let tgt_thread = GetWindowThreadProcessId(hwnd, None);
    if fg_thread != tgt_thread {
      let _ = AttachThreadInput(tgt_thread, fg_thread, true);
    }
    let mut placement = WINDOWPLACEMENT {
      length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
      ..Default::default()
    };
    let mut restored = false;
    if GetWindowPlacement(hwnd, &mut placement).is_ok() {
      match placement.showCmd {
        x if x == SW_SHOWMINIMIZED.0 as u32 => {
          let _ = ShowWindow(hwnd, SW_RESTORE);
          restored = true;
        }
        x if x == SW_SHOWMAXIMIZED.0 as u32 => {
          let _ = ShowWindow(hwnd, SW_SHOWMAXIMIZED);
        }
        _ => {
          let _ = ShowWindow(hwnd, SW_SHOW);
        }
      }
    }
    let _ = BringWindowToTop(hwnd);
    let _ = SetForegroundWindow(hwnd);
    if fg_thread != tgt_thread {
      let _ = AttachThreadInput(tgt_thread, fg_thread, false);
    }
    std::thread::sleep(std::time::Duration::from_millis(if restored { 400 } else { 200 }));
    if GetForegroundWindow() != hwnd {
      let _ = SetForegroundWindow(hwnd);
      let _ = BringWindowToTop(hwnd);
      std::thread::sleep(std::time::Duration::from_millis(100));
    }
    true
  }
}

pub fn get_window_title(hwnd: HWND) -> String {
  let mut buffer = [0u16; 512];
  let len = unsafe { GetWindowTextW(hwnd, &mut buffer) };
  if len > 0 {
    OsString::from_wide(&buffer[..len as usize])
      .to_string_lossy()
      .into_owned()
  } else {
    String::new()
  }
}

pub fn get_window_class_name(hwnd: HWND) -> String {
  let mut buffer = [0u16; 256];
  let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
  if len > 0 {
    OsString::from_wide(&buffer[..len as usize])
      .to_string_lossy()
      .into_owned()
  } else {
    String::new()
  }
}

pub fn get_wnd_rect(wnd: i64) -> Result<WindowRECT> {
  let hwnd = HWND(wnd as *mut core::ffi::c_void);
  let mut rect = RECT::default();
  unsafe {
    GetWindowRect(hwnd, &mut rect)?;
  }
  Ok(WindowRECT {
    x: rect.left,
    y: rect.top,
    w: rect.right - rect.left,
    h: rect.bottom - rect.top,
  })
}

pub fn check_pos_in_wnd(wnd: i64, screen_x: i32, screen_y: i32) -> Result<bool> {
  let rect = get_wnd_rect(wnd)?;
  Ok(screen_x >= rect.x && screen_x <= rect.x + rect.w && screen_y >= rect.y && screen_y <= rect.y + rect.h)
}

pub struct CoordinatePosition {
  pub screen_x: i32,
  pub screen_y: i32,
  pub relative_x: i32,
  pub relative_y: i32,
}

pub fn normalize_to_wnd_pos(hwnd: i64, x: f64, y: f64) -> Result<CoordinatePosition> {
  if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
    return Err(anyhow!("Normalized coordinates must be between 0.0 and 1.0"));
  }
  let rect = get_wnd_rect(hwnd)?;
  let rel_x = (rect.w as f64 * x) as i32;
  let rel_y = (rect.h as f64 * y) as i32;
  Ok(CoordinatePosition {
    screen_x: rect.x + rel_x,
    screen_y: rect.y + rel_y,
    relative_x: rel_x,
    relative_y: rel_y,
  })
}

// === Window core ===

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
  unsafe {
    let vec = &mut *(lparam.0 as *mut Vec<WindowInfo>);
    if IsWindow(Some(hwnd)).as_bool() {
      let mut pid: u32 = 0;
      GetWindowThreadProcessId(hwnd, Some(&mut pid));
      let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
      let mut rect = RECT::default();
      let _ = GetWindowRect(hwnd, &mut rect);
      let parent = GetParent(hwnd).unwrap_or(HWND(std::ptr::null_mut()));
      vec.push(WindowInfo {
        hwnd: hwnd.0 as i64,
        parent_hwnd: parent.0 as i64,
        title: get_window_title(hwnd),
        class_name: get_window_class_name(hwnd),
        process_id: pid,
        process_name: get_process_name(pid),
        is_visible: IsWindowVisible(hwnd).as_bool(),
        is_minimized: IsIconic(hwnd).as_bool(),
        is_maximized: IsZoomed(hwnd).as_bool(),
        is_topmost: (ex & WS_EX_TOPMOST.0) != 0,
        is_tool_window: (ex & WS_EX_TOOLWINDOW.0) != 0,
        is_layered: (ex & WS_EX_LAYERED.0) != 0,
        is_no_activate: (ex & WS_EX_NOACTIVATE.0) != 0,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
        x: rect.left,
        y: rect.top,
      });
    }
    true.into()
  }
}

pub fn fetch_all_windows() -> Result<Vec<WindowInfo>> {
  let mut windows = Vec::new();
  unsafe {
    EnumWindows(
      Some(enum_windows_proc),
      LPARAM(&mut windows as *mut Vec<WindowInfo> as isize),
    )
    .context("EnumWindows failed")?;
  }
  Ok(windows)
}

pub fn sort_windows(windows: &mut [WindowInfo]) {
  windows.sort_by(|a, b| match (a.title.is_empty(), b.title.is_empty()) {
    (true, false) => std::cmp::Ordering::Greater,
    (false, true) => std::cmp::Ordering::Less,
    _ => a.title.cmp(&b.title),
  });
}

pub fn filter_visible_windows(windows: Vec<WindowInfo>) -> Vec<WindowInfo> {
  windows
    .into_iter()
    .filter(|w| {
      if !w.is_visible || w.title.is_empty() {
        return false;
      }
      if w.is_tool_window {
        return false;
      }
      // Skip tiny windows (tray icons, overlays)
      if w.width < 100 || w.height < 100 {
        return false;
      }
      // Skip known system/utility windows
      if is_system_window(&w.title, &w.process_name) {
        return false;
      }
      true
    })
    .collect()
}

pub fn is_system_window(title: &str, process_name: &str) -> bool {
  let t = title.to_lowercase();
  let p = process_name.to_lowercase();
  // System shell
  if p == "explorer.exe" && (t == "program manager" || t.starts_with("workspace")) {
    return false; // Explorer with real titles is a user window
  }
  if t == "program manager" {
    return true;
  }
  // Input/IME overlays
  if p == "textinputhost.exe" || t.contains("windows input experience") {
    return true;
  }
  // GPU overlays
  if t.contains("nvidia geforce overlay") || t.contains("nvidia overlay") {
    return true;
  }
  // GDI+ helper windows
  if t.contains("gdi+ window") {
    return true;
  }
  // IME / input method windows
  if t == "default ime" || t == "msctfime ui" || t == "sogou_tsf_ui" {
    return true;
  }
  false
}

pub fn find_window_handle_by_title(title: &str) -> Result<i64> {
  if let Some(pattern) = title.strip_prefix('*') {
    let windows = fetch_all_windows()?;
    for w in windows {
      if w.title.contains(pattern) && w.is_visible {
        return Ok(w.hwnd);
      }
    }
    Err(anyhow!("Window containing '{}' not found", pattern))
  } else {
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe { FindWindowW(None, windows::core::PCWSTR(wide.as_ptr())) }
      .context(format!("FindWindowW('{}')", title))?;
    if hwnd == HWND(std::ptr::null_mut()) {
      bail!("Window '{}' not found", title);
    }
    Ok(hwnd.0 as i64)
  }
}

pub fn get_child_windows_internal(parent_hwnd: i64) -> Result<Vec<WindowInfo>> {
  let hwnd = HWND(parent_hwnd as *mut core::ffi::c_void);
  let mut main_pid: u32 = 0;
  unsafe {
    GetWindowThreadProcessId(hwnd, Some(&mut main_pid));
  }
  let all = fetch_all_windows()?;
  Ok(
    all
      .into_iter()
      .filter(|w| w.process_id == main_pid && w.hwnd != parent_hwnd && w.is_visible && !w.is_minimized)
      .collect(),
  )
}

pub fn get_window_title_by_handle(handle: i64) -> Result<String> {
  let hwnd = HWND(handle as *mut core::ffi::c_void);
  Ok(get_window_title(hwnd))
}

fn is_valid_top_level_window(hwnd: HWND) -> bool {
  unsafe {
    if !IsWindow(Some(hwnd)).as_bool() || !IsWindowVisible(hwnd).as_bool() {
      return false;
    }
    let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    if (ex & WS_EX_TOOLWINDOW.0) != 0 {
      return false;
    }
    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() {
      return false;
    }
    if (rect.right - rect.left) < 1 || (rect.bottom - rect.top) < 1 {
      return false;
    }
    let title = get_window_title(hwnd);
    if title.contains("IME") || title.contains("GDI+") || title.contains("HIDE MSG") || title.contains("MSCTFIME") {
      return false;
    }
    true
  }
}

struct FindByProcessData<'a> {
  process_id: u32,
  pattern: Option<&'a str>,
  windows: Vec<HWND>,
}

unsafe extern "system" fn enum_by_process_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
  unsafe {
    let data = &mut *(lparam.0 as *mut FindByProcessData);
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == data.process_id {
      if let Some(p) = data.pattern
        && !get_window_title(hwnd).to_lowercase().contains(&p.to_lowercase())
      {
        return true.into();
      }
      data.windows.push(hwnd);
    }
    true.into()
  }
}

fn build_window_info(hwnd: HWND) -> WindowInfo {
  unsafe {
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    let mut rect = RECT::default();
    let _ = GetWindowRect(hwnd, &mut rect);
    let parent = GetParent(hwnd).unwrap_or(HWND(std::ptr::null_mut()));
    WindowInfo {
      hwnd: hwnd.0 as i64,
      parent_hwnd: parent.0 as i64,
      title: get_window_title(hwnd),
      class_name: get_window_class_name(hwnd),
      process_id: pid,
      process_name: get_process_name(pid),
      is_visible: IsWindowVisible(hwnd).as_bool(),
      is_minimized: IsIconic(hwnd).as_bool(),
      is_maximized: IsZoomed(hwnd).as_bool(),
      is_topmost: (ex & WS_EX_TOPMOST.0) != 0,
      is_tool_window: (ex & WS_EX_TOOLWINDOW.0) != 0,
      is_layered: (ex & WS_EX_LAYERED.0) != 0,
      is_no_activate: (ex & WS_EX_NOACTIVATE.0) != 0,
      width: rect.right - rect.left,
      height: rect.bottom - rect.top,
      x: rect.left,
      y: rect.top,
    }
  }
}

pub fn get_all_windows_by_process_id_internal(pid: u32) -> Result<Vec<WindowInfo>> {
  let mut data = FindByProcessData {
    process_id: pid,
    pattern: None,
    windows: Vec::new(),
  };
  unsafe {
    EnumWindows(
      Some(enum_by_process_proc),
      LPARAM(&mut data as *mut FindByProcessData as isize),
    )
    .context("EnumWindows by pid")?;
  }
  Ok(
    data
      .windows
      .iter()
      .filter(|&&h| is_valid_top_level_window(h))
      .map(|&h| build_window_info(h))
      .collect(),
  )
}

pub fn get_all_windows_by_process_id_with_title_internal(
  pid: u32,
  pattern: &str,
  fuzzy: bool,
) -> Result<Vec<WindowInfo>> {
  let mut data = FindByProcessData {
    process_id: pid,
    pattern: None,
    windows: Vec::new(),
  };
  unsafe {
    EnumWindows(
      Some(enum_by_process_proc),
      LPARAM(&mut data as *mut FindByProcessData as isize),
    )
    .context("EnumWindows by pid+title")?;
  }
  let pat_lower = pattern.to_lowercase();
  Ok(
    data
      .windows
      .iter()
      .filter(|&&h| {
        if !is_valid_top_level_window(h) {
          return false;
        }
        let t = get_window_title(h);
        if fuzzy {
          t.to_lowercase().contains(&pat_lower)
        } else {
          t == pattern
        }
      })
      .map(|&h| build_window_info(h))
      .collect(),
  )
}

// === NEW: Window manipulation ===

pub fn move_window(handle: &WindowHandle, x: i32, y: i32) -> Result<()> {
  let hwnd = HWND(handle.0 as *mut core::ffi::c_void);
  let rect = get_wnd_rect(handle.0)?;
  unsafe {
    SetWindowPos(hwnd, None, x, y, rect.w, rect.h, SWP_NOZORDER | SWP_NOSIZE).context("SetWindowPos move")?;
  }
  Ok(())
}

pub fn resize_window(handle: &WindowHandle, width: i32, height: i32) -> Result<()> {
  let hwnd = HWND(handle.0 as *mut core::ffi::c_void);
  let rect = get_wnd_rect(handle.0)?;
  unsafe {
    SetWindowPos(hwnd, None, rect.x, rect.y, width, height, SWP_NOZORDER | SWP_NOMOVE)
      .context("SetWindowPos resize")?;
  }
  Ok(())
}

pub fn set_window_rect(handle: &WindowHandle, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
  let hwnd = HWND(handle.0 as *mut core::ffi::c_void);
  unsafe {
    SetWindowPos(hwnd, None, x, y, width, height, SWP_NOZORDER).context("SetWindowPos rect")?;
  }
  Ok(())
}

pub fn minimize_window(handle: &WindowHandle) -> Result<()> {
  let hwnd = HWND(handle.0 as *mut core::ffi::c_void);
  unsafe {
    let _ = ShowWindow(hwnd, SW_SHOWMINIMIZED);
  }
  Ok(())
}

pub fn maximize_window(handle: &WindowHandle) -> Result<()> {
  let hwnd = HWND(handle.0 as *mut core::ffi::c_void);
  unsafe {
    let _ = ShowWindow(hwnd, SW_SHOWMAXIMIZED);
  }
  Ok(())
}

pub fn restore_window(handle: &WindowHandle) -> Result<()> {
  let hwnd = HWND(handle.0 as *mut core::ffi::c_void);
  unsafe {
    let _ = ShowWindow(hwnd, SW_RESTORE);
  }
  Ok(())
}
