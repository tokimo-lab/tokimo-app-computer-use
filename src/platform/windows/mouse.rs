use crate::platform::windows::elements::utils::find_first_element_by_xpath;
use crate::platform::windows::wnd;
use crate::types::{InputResult, MouseButton, WindowHandle};
use anyhow::Result;
use windows::Win32::{
  Foundation::HWND,
  UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

pub fn click_by_pos(x: i32, y: i32, button: MouseButton, is_double_click: bool) -> Result<()> {
  unsafe {
    SetCursorPos(x, y)?;
    thread_sleep(50);
    let (down, up) = match button {
      MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
      _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
    };
    let mut input = mouse_input(down);
    if is_double_click {
      SendInput(&[input], size_of_input());
      thread_sleep(50);
      input.Anonymous.mi.dwFlags = up;
      SendInput(&[input], size_of_input());
      let delay = (GetDoubleClickTime() / 2).clamp(10, 100) as u64;
      thread_sleep(delay);
      input.Anonymous.mi.dwFlags = down;
      SendInput(&[input], size_of_input());
      thread_sleep(50);
      input.Anonymous.mi.dwFlags = up;
      SendInput(&[input], size_of_input());
    } else {
      SendInput(&[input], size_of_input());
      thread_sleep(50);
      input.Anonymous.mi.dwFlags = up;
      SendInput(&[input], size_of_input());
    }
    thread_sleep(100);
  }
  Ok(())
}

fn mouse_input(flags: MOUSE_EVENT_FLAGS) -> INPUT {
  INPUT {
    r#type: INPUT_MOUSE,
    Anonymous: INPUT_0 {
      mi: MOUSEINPUT {
        dx: 0,
        dy: 0,
        mouseData: 0,
        dwFlags: flags,
        time: 0,
        dwExtraInfo: 0,
      },
    },
  }
}

fn size_of_input() -> i32 {
  std::mem::size_of::<INPUT>() as i32
}

fn thread_sleep(ms: u64) {
  std::thread::sleep(std::time::Duration::from_millis(ms));
}

pub fn click_by_hwnd_pos(hwnd: i64, x: f64, y: f64, button: MouseButton, double_click: bool) -> Result<InputResult> {
  let hwnd_handle = HWND(hwnd as *mut core::ffi::c_void);
  let coord = wnd::normalize_to_wnd_pos(hwnd, x, y)?;
  if !wnd::check_pos_in_wnd(hwnd, coord.screen_x, coord.screen_y)? {
    return Ok(InputResult::fail_with_coords(
      "Click outside window".into(),
      coord.screen_x,
      coord.screen_y,
      coord.relative_x,
      coord.relative_y,
    ));
  }
  bring_window_to_front_by_handle(hwnd_handle)?;
  click_by_pos(coord.screen_x, coord.screen_y, button, double_click)?;
  Ok(InputResult::success(
    coord.screen_x,
    coord.screen_y,
    coord.relative_x,
    coord.relative_y,
  ))
}

pub fn click_by_xpath(hwnd: i64, xpath: &str, button: MouseButton, double_click: bool) -> Result<InputResult> {
  let element = find_first_element_by_xpath(hwnd, xpath)?;
  let pos = element.pos(Some(hwnd)).map_err(|e| anyhow::anyhow!("{}", e))?;
  let (sx, sy, rx, ry) = (pos.center_x, pos.center_y, pos.relative_center_x, pos.relative_center_y);
  match click_by_pos(sx, sy, button, double_click) {
    Ok(_) => Ok(InputResult::success(sx, sy, rx, ry)),
    Err(e) => Ok(InputResult::fail_with_coords(e.to_string(), sx, sy, rx, ry)),
  }
}

fn bring_window_to_front_by_handle(hwnd: HWND) -> Result<()> {
  wnd::bring_window_to_front(hwnd);
  Ok(())
}

pub fn validate_click_position(screen_x: i32, screen_y: i32, hwnd: i64) -> Result<bool> {
  wnd::check_pos_in_wnd(hwnd, screen_x, screen_y)
}

// === NEW capabilities ===

pub fn move_cursor(x: i32, y: i32) -> Result<()> {
  unsafe {
    SetCursorPos(x, y)?;
  }
  Ok(())
}

pub fn get_cursor_position() -> Result<(i32, i32)> {
  unsafe {
    let mut pt = windows::Win32::Foundation::POINT::default();
    GetCursorPos(&mut pt)?;
    Ok((pt.x, pt.y))
  }
}

pub fn drag(
  handle: &WindowHandle,
  from_x: f64,
  from_y: f64,
  to_x: f64,
  to_y: f64,
  button: MouseButton,
) -> Result<InputResult> {
  let hwnd = handle.0;
  let from = wnd::normalize_to_wnd_pos(hwnd, from_x, from_y)?;
  let to = wnd::normalize_to_wnd_pos(hwnd, to_x, to_y)?;
  let (down_flag, up_flag) = match button {
    MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
    _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
  };
  unsafe {
    SetCursorPos(from.screen_x, from.screen_y)?;
    thread_sleep(50);
    let mut input = mouse_input(down_flag);
    SendInput(&[input], size_of_input());
    thread_sleep(50);
    // Move in steps
    let steps = 10;
    for i in 1..=steps {
      let cx = from.screen_x + (to.screen_x - from.screen_x) * i / steps;
      let cy = from.screen_y + (to.screen_y - from.screen_y) * i / steps;
      SetCursorPos(cx, cy)?;
      thread_sleep(10);
    }
    thread_sleep(50);
    input.Anonymous.mi.dwFlags = up_flag;
    SendInput(&[input], size_of_input());
    thread_sleep(100);
  }
  Ok(InputResult::success(
    to.screen_x,
    to.screen_y,
    to.relative_x,
    to.relative_y,
  ))
}

pub fn scroll(handle: &WindowHandle, x: f64, y: f64, delta_x: i32, delta_y: i32) -> Result<InputResult> {
  let hwnd = handle.0;
  let coord = wnd::normalize_to_wnd_pos(hwnd, x, y)?;
  unsafe {
    SetCursorPos(coord.screen_x, coord.screen_y)?;
    thread_sleep(50);
    if delta_y != 0 {
      let mut input = mouse_input(MOUSEEVENTF_WHEEL);
      input.Anonymous.mi.mouseData = delta_y as u32;
      SendInput(&[input], size_of_input());
      thread_sleep(50);
    }
    if delta_x != 0 {
      let mut input = mouse_input(MOUSEEVENTF_HWHEEL);
      input.Anonymous.mi.mouseData = delta_x as u32;
      SendInput(&[input], size_of_input());
      thread_sleep(50);
    }
  }
  Ok(InputResult::success(
    coord.screen_x,
    coord.screen_y,
    coord.relative_x,
    coord.relative_y,
  ))
}
