use core_graphics::display::CGDisplay;
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
}

fn create_source() -> Result<CGEventSource> {
  CGEventSource::new(CGEventSourceStateID::HIDSystemState)
    .map_err(|_| anyhow::anyhow!("failed to create CGEventSource"))
}

fn screen_height() -> f64 {
  CGDisplay::main().pixels_high() as f64
}

/// Convert top-left origin coordinates to CGEvent bottom-left origin.
fn to_cg_y(y: f64) -> f64 {
  screen_height() - y
}

/// Convert CGEvent bottom-left origin coordinates to top-left origin.
fn from_cg_y(y: f64) -> f64 {
  screen_height() - y
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
  let click_count = if double_click { 2_i64 } else { 1_i64 };

  let (down_type, up_type) = match button {
    MouseButton::Left => (CGEventType::LeftMouseDown, CGEventType::LeftMouseUp),
    MouseButton::Right => (CGEventType::RightMouseDown, CGEventType::RightMouseUp),
    MouseButton::Middle => (CGEventType::OtherMouseDown, CGEventType::OtherMouseUp),
  };

  // Mouse down
  let down_event = CGEvent::new_mouse_event(source.clone(), down_type, point, cg_button)
    .map_err(|_| anyhow::anyhow!("failed to create mouse down event"))?;
  down_event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count);
  post_event(&down_event)?;

  // Mouse up
  let up_event = CGEvent::new_mouse_event(source, up_type, point, cg_button)
    .map_err(|_| anyhow::anyhow!("failed to create mouse up event"))?;
  up_event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count);
  post_event(&up_event)?;

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

  // Interpolate mouse moves
  let steps = 10;
  for i in 1..=steps {
    let t = i as f64 / steps as f64;
    let pt = CGPoint::new(
      start.x + (end.x - start.x) * t,
      start.y + (end.y - start.y) * t,
    );
    let mv = CGEvent::new_mouse_event(source.clone(), CGEventType::MouseMoved, pt, CGMouseButton::Left)
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
  _x: f64,
  _y: f64,
  delta_x: i32,
  delta_y: i32,
) -> Result<InputResult> {
  let source = create_source()?;
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

  Ok(InputResult::success(0, 0, 0, 0))
}
