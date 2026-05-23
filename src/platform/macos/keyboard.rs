use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

// Raw FFI for CGEventKeyboardSetUnicodeString (not exposed in core-graphics 0.25)
type CGItemCount = i64;
unsafe extern "C" {
  fn CGEventKeyboardSetUnicodeString(event: core_graphics::sys::CGEventRef, length: CGItemCount, string: *const u16);
}

use crate::error::Result;
use crate::types::*;

// macOS virtual keycodes (used implicitly via cg_keycode)

fn create_source() -> Result<CGEventSource> {
  CGEventSource::new(CGEventSourceStateID::HIDSystemState)
    .map_err(|_| anyhow::anyhow!("failed to create CGEventSource"))
}

fn post_event(event: &CGEvent) -> Result<()> {
  event.post(CGEventTapLocation::HID);
  Ok(())
}

/// Post event to a specific process by PID (no focus steal).
fn post_event_to_pid(event: &CGEvent, pid: i32) {
  unsafe {
    use foreign_types::ForeignType;
    CGEventPostToPid(pid, event.as_ptr() as *const std::ffi::c_void);
  }
}

// Private API: post CGEvent to a specific process
type CGEventRef = *const std::ffi::c_void;
unsafe extern "C" {
  fn CGEventPostToPid(pid: i32, event: CGEventRef);
}

fn keycode_for_char(c: char) -> Option<u16> {
  match c {
    'a' => Some(0),
    'b' => Some(11),
    'c' => Some(8),
    'd' => Some(2),
    'e' => Some(14),
    'f' => Some(3),
    'g' => Some(5),
    'h' => Some(4),
    'i' => Some(34),
    'j' => Some(38),
    'k' => Some(40),
    'l' => Some(37),
    'm' => Some(46),
    'n' => Some(45),
    'o' => Some(31),
    'p' => Some(35),
    'q' => Some(12),
    'r' => Some(15),
    's' => Some(1),
    't' => Some(17),
    'u' => Some(32),
    'v' => Some(9),
    'w' => Some(13),
    'x' => Some(7),
    'y' => Some(16),
    'z' => Some(6),
    'A'..='Z' => keycode_for_char(c.to_ascii_lowercase()),
    '1' => Some(18),
    '2' => Some(19),
    '3' => Some(20),
    '4' => Some(21),
    '5' => Some(23),
    '6' => Some(22),
    '7' => Some(26),
    '8' => Some(28),
    '9' => Some(25),
    '0' => Some(29),
    ' ' => Some(49),
    '\n' => Some(36),
    '\t' => Some(48),
    '-' => Some(27),
    '=' => Some(24),
    '[' => Some(33),
    ']' => Some(30),
    '\\' => Some(42),
    ';' => Some(41),
    '\'' => Some(39),
    ',' => Some(43),
    '.' => Some(47),
    '/' => Some(44),
    '`' => Some(50),
    '!' => Some(18),
    '@' => Some(19),
    '#' => Some(20),
    '$' => Some(21),
    '%' => Some(23),
    '^' => Some(22),
    '&' => Some(26),
    '*' => Some(28),
    '(' => Some(25),
    ')' => Some(29),
    _ => None,
  }
}

fn needs_shift(c: char) -> bool {
  c.is_ascii_uppercase()
    || matches!(
      c,
      '!' | '@' | '#' | '$' | '%' | '^' | '&' | '*' | '(' | ')' | '_'
        | '+' | '{' | '}' | '|' | ':' | '"' | '<' | '>' | '?' | '~'
    )
}

fn cg_keycode(key: &KeyCode) -> Option<u16> {
  match key {
    KeyCode::A => Some(0),
    KeyCode::B => Some(11),
    KeyCode::C => Some(8),
    KeyCode::D => Some(2),
    KeyCode::E => Some(14),
    KeyCode::F => Some(3),
    KeyCode::G => Some(5),
    KeyCode::H => Some(4),
    KeyCode::I => Some(34),
    KeyCode::J => Some(38),
    KeyCode::K => Some(40),
    KeyCode::L => Some(37),
    KeyCode::M => Some(46),
    KeyCode::N => Some(45),
    KeyCode::O => Some(31),
    KeyCode::P => Some(35),
    KeyCode::Q => Some(12),
    KeyCode::R => Some(15),
    KeyCode::S => Some(1),
    KeyCode::T => Some(17),
    KeyCode::U => Some(32),
    KeyCode::V => Some(9),
    KeyCode::W => Some(13),
    KeyCode::X => Some(7),
    KeyCode::Y => Some(16),
    KeyCode::Z => Some(6),
    KeyCode::Digit0 => Some(29),
    KeyCode::Digit1 => Some(18),
    KeyCode::Digit2 => Some(19),
    KeyCode::Digit3 => Some(20),
    KeyCode::Digit4 => Some(21),
    KeyCode::Digit5 => Some(23),
    KeyCode::Digit6 => Some(22),
    KeyCode::Digit7 => Some(26),
    KeyCode::Digit8 => Some(28),
    KeyCode::Digit9 => Some(25),
    KeyCode::F1 => Some(122),
    KeyCode::F2 => Some(120),
    KeyCode::F3 => Some(99),
    KeyCode::F4 => Some(118),
    KeyCode::F5 => Some(96),
    KeyCode::F6 => Some(97),
    KeyCode::F7 => Some(98),
    KeyCode::F8 => Some(100),
    KeyCode::F9 => Some(101),
    KeyCode::F10 => Some(109),
    KeyCode::F11 => Some(103),
    KeyCode::F12 => Some(111),
    KeyCode::Enter | KeyCode::Return => Some(36),
    KeyCode::Tab => Some(48),
    KeyCode::Escape | KeyCode::Esc => Some(53),
    KeyCode::Space | KeyCode::Spacebar => Some(49),
    KeyCode::Backspace | KeyCode::Back => Some(51),
    KeyCode::Delete | KeyCode::Del => Some(117),
    KeyCode::Left | KeyCode::ArrowLeft => Some(123),
    KeyCode::Up | KeyCode::ArrowUp => Some(126),
    KeyCode::Right | KeyCode::ArrowRight => Some(124),
    KeyCode::Down | KeyCode::ArrowDown => Some(125),
    KeyCode::Home => Some(115),
    KeyCode::End => Some(119),
    KeyCode::PageUp => Some(116),
    KeyCode::PageDown => Some(121),
    _ => None,
  }
}

fn is_shift_key(key: &KeyCode) -> bool {
  matches!(key, KeyCode::Shift | KeyCode::LShift | KeyCode::RShift)
}

fn is_ctrl_key(key: &KeyCode) -> bool {
  matches!(key, KeyCode::Ctrl | KeyCode::LCtrl | KeyCode::RCtrl)
}

fn is_alt_key(key: &KeyCode) -> bool {
  matches!(key, KeyCode::Alt | KeyCode::LAlt | KeyCode::RAlt)
}

fn is_win_key(key: &KeyCode) -> bool {
  matches!(key, KeyCode::Win | KeyCode::LWin | KeyCode::RWin)
}

fn build_modifiers(shift: bool, ctrl: bool, alt: bool, cmd: bool) -> CGEventFlags {
  let mut flags = CGEventFlags::empty();
  if shift {
    flags |= CGEventFlags::CGEventFlagShift;
  }
  if ctrl {
    flags |= CGEventFlags::CGEventFlagControl;
  }
  if alt {
    flags |= CGEventFlags::CGEventFlagAlternate;
  }
  if cmd {
    flags |= CGEventFlags::CGEventFlagCommand;
  }
  flags
}

/// Post a unicode character via CGEventKeyboardSetUnicodeString.
/// This handles characters that don't have a direct keycode mapping.
fn send_unicode_char(ch: char) -> Result<()> {
  let source = create_source()?;
  // Create a key event with keycode 0
  let down = CGEvent::new_keyboard_event(source.clone(), 0, true)
    .map_err(|_| anyhow::anyhow!("failed to create unicode key down event"))?;
  let up = CGEvent::new_keyboard_event(source, 0, false)
    .map_err(|_| anyhow::anyhow!("failed to create unicode key up event"))?;

  // Set the unicode string using the C API
  let utf16: Vec<u16> = ch.encode_utf16(&mut [0u16; 2]).to_vec();
  unsafe {
    use foreign_types::ForeignType;
    let down_ref = down.as_ptr();
    let up_ref = up.as_ptr();
    CGEventKeyboardSetUnicodeString(
      down_ref,
      utf16.len() as CGItemCount,
      utf16.as_ptr(),
    );
    CGEventKeyboardSetUnicodeString(
      up_ref,
      utf16.len() as CGItemCount,
      utf16.as_ptr(),
    );
  }

  post_event(&down)?;
  post_event(&up)?;
  Ok(())
}

/// Send a single key press and release with a known keycode.
fn send_key_event(kc: u16, flags: CGEventFlags) -> Result<()> {
  let source = create_source()?;
  let down = CGEvent::new_keyboard_event(source.clone(), kc, true)
    .map_err(|_| anyhow::anyhow!("failed to create key down event"))?;
  down.set_flags(flags);
  post_event(&down)?;

  let up = CGEvent::new_keyboard_event(source, kc, false)
    .map_err(|_| anyhow::anyhow!("failed to create key up event"))?;
  up.set_flags(flags);
  post_event(&up)?;

  Ok(())
}

/// Send a key event to a specific PID (no focus steal).
fn send_key_event_to_pid(kc: u16, flags: CGEventFlags, pid: i32) -> Result<()> {
  let source = create_source()?;
  let down = CGEvent::new_keyboard_event(source.clone(), kc, true)
    .map_err(|_| anyhow::anyhow!("failed to create key down event"))?;
  down.set_flags(flags);
  post_event_to_pid(&down, pid);

  let up = CGEvent::new_keyboard_event(source, kc, false)
    .map_err(|_| anyhow::anyhow!("failed to create key up event"))?;
  up.set_flags(flags);
  post_event_to_pid(&up, pid);

  Ok(())
}

/// Type text to a specific PID via CGEventPostToPid.
#[allow(dead_code)]
pub fn type_text_to_pid(pid: i32, text: &str) -> Result<()> {
  let _guard = super::input_source::AsciiInputGuard::enter();
  for ch in text.chars() {
    if let Some(kc) = keycode_for_char(ch) {
      let shift = needs_shift(ch);
      let flags = build_modifiers(shift, false, false, false);
      send_key_event_to_pid(kc, flags, pid)?;
    } else {
      let source = create_source()?;
      let down = CGEvent::new_keyboard_event(source.clone(), 0, true)
        .map_err(|_| anyhow::anyhow!("failed to create unicode key down event"))?;
      let up = CGEvent::new_keyboard_event(source, 0, false)
        .map_err(|_| anyhow::anyhow!("failed to create unicode key up event"))?;
      let utf16: Vec<u16> = ch.encode_utf16(&mut [0u16; 2]).to_vec();
      unsafe {
        use foreign_types::ForeignType;
        CGEventKeyboardSetUnicodeString(down.as_ptr(), utf16.len() as CGItemCount, utf16.as_ptr());
        CGEventKeyboardSetUnicodeString(up.as_ptr(), utf16.len() as CGItemCount, utf16.as_ptr());
      }
      post_event_to_pid(&down, pid);
      post_event_to_pid(&up, pid);
    }
    std::thread::sleep(std::time::Duration::from_millis(20));
  }
  Ok(())
}

/// Send keys to a specific PID via CGEventPostToPid.
pub fn send_keys_to_pid(pid: i32, keys: &[KeyCode], modifiers: Option<&[KeyCode]>) -> Result<()> {
  let mut shift = false;
  let mut ctrl = false;
  let mut alt = false;
  let mut cmd = false;

  if let Some(mods) = modifiers {
    for m in mods {
      if is_shift_key(m) { shift = true; }
      if is_ctrl_key(m) { ctrl = true; }
      if is_alt_key(m) { alt = true; }
      if is_win_key(m) { cmd = true; }
    }
  }

  for key in keys {
    if is_shift_key(key) { shift = true; continue; }
    if is_ctrl_key(key) { ctrl = true; continue; }
    if is_alt_key(key) { alt = true; continue; }
    if is_win_key(key) { cmd = true; continue; }

    if let Some(kc) = cg_keycode(key) {
      let flags = build_modifiers(shift, ctrl, alt, cmd);
      send_key_event_to_pid(kc, flags, pid)?;
      std::thread::sleep(std::time::Duration::from_millis(20));
    }
  }
  Ok(())
}

pub fn type_text(_handle: &WindowHandle, text: &str, _position: Option<&InputPosition>) -> Result<InputResult> {
  let _guard = super::input_source::AsciiInputGuard::enter();
  for ch in text.chars() {
    if let Some(kc) = keycode_for_char(ch) {
      let shift = needs_shift(ch);
      let flags = build_modifiers(shift, false, false, false);
      send_key_event(kc, flags)?;
    } else {
      send_unicode_char(ch)?;
    }
    std::thread::sleep(std::time::Duration::from_millis(20));
  }
  Ok(InputResult::success(0, 0, 0, 0))
}

pub fn type_text_raw(_handle: &WindowHandle, text: &str) -> Result<()> {
  let _guard = super::input_source::AsciiInputGuard::enter();
  for ch in text.chars() {
    if let Some(kc) = keycode_for_char(ch) {
      let shift = needs_shift(ch);
      let flags = build_modifiers(shift, false, false, false);
      send_key_event(kc, flags)?;
    } else {
      send_unicode_char(ch)?;
    }
    std::thread::sleep(std::time::Duration::from_millis(20));
  }
  Ok(())
}

pub fn send_keys(keys: &[KeyCode], modifiers: Option<&[KeyCode]>) -> Result<()> {
  let mut shift = false;
  let mut ctrl = false;
  let mut alt = false;
  let mut cmd = false;

  // Collect modifiers from the modifiers parameter
  if let Some(mods) = modifiers {
    for m in mods {
      if is_shift_key(m) { shift = true; }
      if is_ctrl_key(m) { ctrl = true; }
      if is_alt_key(m) { alt = true; }
      if is_win_key(m) { cmd = true; }
    }
  }

  for key in keys {
    // Keys that are themselves modifiers
    if is_shift_key(key) { shift = true; continue; }
    if is_ctrl_key(key) { ctrl = true; continue; }
    if is_alt_key(key) { alt = true; continue; }
    if is_win_key(key) { cmd = true; continue; }

    if let Some(kc) = cg_keycode(key) {
      let flags = build_modifiers(shift, ctrl, alt, cmd);
      send_key_event(kc, flags)?;
      std::thread::sleep(std::time::Duration::from_millis(20));
    }
  }
  Ok(())
}

pub fn key_down(key: KeyCode) -> Result<()> {
  let source = create_source()?;
  if let Some(kc) = cg_keycode(&key) {
    let event = CGEvent::new_keyboard_event(source, kc, true)
      .map_err(|_| anyhow::anyhow!("failed to create key down event"))?;
    post_event(&event)?;
  }
  Ok(())
}

pub fn key_release(key: KeyCode) -> Result<()> {
  let source = create_source()?;
  if let Some(kc) = cg_keycode(&key) {
    let event = CGEvent::new_keyboard_event(source, kc, false)
      .map_err(|_| anyhow::anyhow!("failed to create key up event"))?;
    post_event(&event)?;
  }
  Ok(())
}
