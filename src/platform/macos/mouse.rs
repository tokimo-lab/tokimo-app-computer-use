use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use foreign_types::ForeignType;

use crate::error::Result;
use crate::types::*;

// Raw FFI for CGEventCreateScrollWheelEvent2 (not exposed in core-graphics 0.25)
type CGScrollEventUnit = u32;
const K_CG_SCROLL_EVENT_UNIT_PIXEL: CGScrollEventUnit = 0;

unsafe extern "C" {
  fn CGEventCreateScrollWheelEvent2(
    source: core_graphics::sys::CGEventSourceRef,
    units: CGScrollEventUnit,
    wheelCount: u32,
    wheel1: i32,
    wheel2: i32,
    wheel3: i32,
  ) -> core_graphics::sys::CGEventRef;
  fn CGWarpMouseCursorPosition(new_cursor_position: CGPoint);
}

fn create_source() -> Result<CGEventSource> {
  CGEventSource::new(CGEventSourceStateID::HIDSystemState)
    .map_err(|_| anyhow::anyhow!("failed to create CGEventSource"))
}

/// Return the logical (point-based) screen size — correct for Retina displays.
/// NSScreen.mainScreen.frame uses logical points, not physical pixels.
fn logical_screen_size() -> (f64, f64) {
  unsafe {
    let mtm = objc2::MainThreadMarker::new_unchecked();
    if let Some(screen) = objc2_app_kit::NSScreen::mainScreen(mtm) {
      let frame = screen.frame();
      return (frame.size.width, frame.size.height);
    }
  }
  // Fallback: physical pixels (wrong on Retina, but better than crashing)
  let d = core_graphics::display::CGDisplay::main();
  (d.pixels_wide() as f64, d.pixels_high() as f64)
}

/// CGEvent mouse events use top-left global display coordinates — the same
/// origin as AX, NSWindow.frame.... wait no — NSWindow is bottom-left, but
/// CGEvent mouse position and AX both use top-left. No flip is needed.
fn to_cg_y(y: f64) -> f64 {
  y
}

/// CGEvent.location() also returns top-left global display coordinates.
fn from_cg_y(y: f64) -> f64 {
  y
}

fn post_event(event: &CGEvent) -> Result<()> {
  event.post(CGEventTapLocation::HID);
  Ok(())
}

/// Post event to a specific process by PID (no focus steal).
/// Useful for daemon mode where focus management is needed.
#[allow(dead_code)]
fn post_event_to_pid(event: &CGEvent, pid: i32) {
  unsafe {
    use foreign_types::ForeignType;
    CGEventPostToPid(pid, event.as_ptr() as *const std::ffi::c_void);
  }
}

#[allow(dead_code)]
type CGEventRef = *const std::ffi::c_void;
#[allow(dead_code)]
unsafe extern "C" {
  fn CGEventPostToPid(pid: i32, event: CGEventRef);
}

pub fn move_cursor(x: i32, y: i32) -> Result<()> {
  let source = create_source()?;
  let point = CGPoint::new(x as f64, to_cg_y(y as f64));
  let event = CGEvent::new_mouse_event(source, CGEventType::MouseMoved, point, CGMouseButton::Left)
    .map_err(|_| anyhow::anyhow!("failed to create mouse move event"))?;
  post_event(&event)
}

pub fn get_cursor_position() -> Result<(i32, i32)> {
  let source = create_source()?;
  let event = CGEvent::new(source)
    .map_err(|_| anyhow::anyhow!("failed to create event for cursor position"))?;
  let pt = event.location();
  Ok((pt.x as i32, from_cg_y(pt.y) as i32))
}

fn screen_to_abs(handle: &WindowHandle, x: f64, y: f64) -> Result<(i32, i32)> {
  if handle.0 == 0 {
    return Ok((x as i32, y as i32));
  }
  let wins = crate::platform::macos::window::list_windows()?;
  if let Some(w) = wins.iter().find(|w| w.hwnd == handle.0) {
    Ok(((w.x as f64 + x) as i32, (w.y as f64 + y) as i32))
  } else {
    Ok((x as i32, y as i32))
  }
}

fn do_click(source: &CGEventSource, point: CGPoint, cg_button: CGMouseButton, down_type: CGEventType, up_type: CGEventType, click_state: i64) -> Result<()> {
  let down_event = CGEvent::new_mouse_event(source.clone(), down_type, point, cg_button)
    .map_err(|_| anyhow::anyhow!("failed to create mouse down event"))?;
  down_event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_state);
  post_event(&down_event)?;

  let up_event = CGEvent::new_mouse_event(source.clone(), up_type, point, cg_button)
    .map_err(|_| anyhow::anyhow!("failed to create mouse up event"))?;
  up_event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_state);
  post_event(&up_event)
}

pub fn click(
  handle: &WindowHandle,
  x: f64,
  y: f64,
  button: MouseButton,
  double_click: bool,
) -> Result<InputResult> {
  let (abs_x, abs_y) = screen_to_abs(handle, x, y)?;
  let source = create_source()?;
  let point = CGPoint::new(abs_x as f64, to_cg_y(abs_y as f64));
  let cg_button = match button {
    MouseButton::Left => CGMouseButton::Left,
    MouseButton::Right => CGMouseButton::Right,
    MouseButton::Middle => CGMouseButton::Center,
  };
  let (down_type, up_type) = match button {
    MouseButton::Left => (CGEventType::LeftMouseDown, CGEventType::LeftMouseUp),
    MouseButton::Right => (CGEventType::RightMouseDown, CGEventType::RightMouseUp),
    MouseButton::Middle => (CGEventType::OtherMouseDown, CGEventType::OtherMouseUp),
  };

  if double_click {
    // BUG-06: Two full down/up sequences, click_state 1 then 2, with 50ms gap
    do_click(&source, point, cg_button, down_type, up_type, 1)?;
    std::thread::sleep(std::time::Duration::from_millis(50));
    do_click(&source, point, cg_button, down_type, up_type, 2)?;
  } else {
    do_click(&source, point, cg_button, down_type, up_type, 1)?;
  }

  Ok(InputResult::success(abs_x, abs_y, x as i32, y as i32))
}

/// Click at window-relative coordinates, posted to a specific PID (no focus steal).
/// Useful for daemon mode where focus management is needed.
#[allow(dead_code)]
pub fn click_to_pid(pid: i32, rel_x: i32, rel_y: i32) -> Result<()> {
  let wins = crate::platform::macos::window::list_windows()?;
  let win = wins.iter().find(|w| w.process_id as i32 == pid);
  let abs_x = win.map(|w| w.x + rel_x).unwrap_or(rel_x);
  let abs_y = win.map(|w| w.y + rel_y).unwrap_or(rel_y);

  let source = create_source()?;
  let point = CGPoint::new(abs_x as f64, to_cg_y(abs_y as f64));

  let down = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, point, CGMouseButton::Left)
    .map_err(|_| anyhow::anyhow!("failed to create mouse down event"))?;
  down.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 1);
  post_event_to_pid(&down, pid);

  let up = CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)
    .map_err(|_| anyhow::anyhow!("failed to create mouse up event"))?;
  up.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 1);
  post_event_to_pid(&up, pid);

  Ok(())
}

pub fn drag(
  _handle: &WindowHandle,
  from_x: f64,
  from_y: f64,
  to_x: f64,
  to_y: f64,
  _button: MouseButton,
) -> Result<InputResult> {
  let source = create_source()?;
  let start = CGPoint::new(from_x, to_cg_y(from_y));
  let end = CGPoint::new(to_x, to_cg_y(to_y));

  // Mouse down at start
  let down = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, start, CGMouseButton::Left)
    .map_err(|_| anyhow::anyhow!("failed to create drag down event"))?;
  post_event(&down)?;

  // Interpolate mouse drags (BUG-08: use LeftMouseDragged not MouseMoved)
  let steps = 10;
  for i in 1..=steps {
    let t = i as f64 / steps as f64;
    let pt = CGPoint::new(
      start.x + (end.x - start.x) * t,
      start.y + (end.y - start.y) * t,
    );
    let mv = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDragged, pt, CGMouseButton::Left)
      .map_err(|_| anyhow::anyhow!("failed to create drag move event"))?;
    post_event(&mv)?;
    std::thread::sleep(std::time::Duration::from_millis(10));
  }

  // Mouse up at end
  let up = CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, end, CGMouseButton::Left)
    .map_err(|_| anyhow::anyhow!("failed to create drag up event"))?;
  post_event(&up)?;

  Ok(InputResult::success(to_x as i32, to_y as i32, to_x as i32, to_y as i32))
}

pub fn scroll(
  _handle: &WindowHandle,
  x: f64,
  y: f64,
  delta_x: i32,
  delta_y: i32,
) -> Result<InputResult> {
  let source = create_source()?;

  // BUG-07: Move cursor to target position before scrolling
  let cg_point = CGPoint::new(x, to_cg_y(y));
  unsafe { CGWarpMouseCursorPosition(cg_point) };

  // CGEvent scroll uses positive = up, but our API uses positive = down, so negate
  let cg_dy = -delta_y;
  let cg_dx = -delta_x;

  let event_ref = unsafe {
    CGEventCreateScrollWheelEvent2(
      source.as_ptr(),
      K_CG_SCROLL_EVENT_UNIT_PIXEL,
      2, // wheel count (1=vertical only, 2=vertical+horizontal)
      cg_dy,
      cg_dx,
      0,
    )
  };
  if event_ref.is_null() {
    return Err(anyhow::anyhow!("failed to create scroll event"));
  }
  let event = unsafe { CGEvent::from_ptr(event_ref) };
  post_event(&event)?;

  Ok(InputResult::success(x as i32, y as i32, x as i32, y as i32))
}
