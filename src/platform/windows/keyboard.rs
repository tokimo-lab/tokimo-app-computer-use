use crate::platform::Element;
use crate::platform::windows::elements::utils::find_first_element_by_xpath;
use crate::platform::windows::mouse::click_by_pos;
use crate::platform::windows::wnd;
use crate::types::MouseButton;
use crate::types::{InputPosition, InputResult, KeyCode};
use std::{thread, time::Duration};
use windows::Win32::{Foundation::HWND, UI::Input::KeyboardAndMouse::*};

/// Convert KeyCode to Windows virtual key code
pub fn to_virtual_key(key: &KeyCode) -> u16 {
  match key {
    KeyCode::A => VK_A.0,
    KeyCode::B => VK_B.0,
    KeyCode::C => VK_C.0,
    KeyCode::D => VK_D.0,
    KeyCode::E => VK_E.0,
    KeyCode::F => VK_F.0,
    KeyCode::G => VK_G.0,
    KeyCode::H => VK_H.0,
    KeyCode::I => VK_I.0,
    KeyCode::J => VK_J.0,
    KeyCode::K => VK_K.0,
    KeyCode::L => VK_L.0,
    KeyCode::M => VK_M.0,
    KeyCode::N => VK_N.0,
    KeyCode::O => VK_O.0,
    KeyCode::P => VK_P.0,
    KeyCode::Q => VK_Q.0,
    KeyCode::R => VK_R.0,
    KeyCode::S => VK_S.0,
    KeyCode::T => VK_T.0,
    KeyCode::U => VK_U.0,
    KeyCode::V => VK_V.0,
    KeyCode::W => VK_W.0,
    KeyCode::X => VK_X.0,
    KeyCode::Y => VK_Y.0,
    KeyCode::Z => VK_Z.0,
    KeyCode::Digit0 => VK_0.0,
    KeyCode::Digit1 => VK_1.0,
    KeyCode::Digit2 => VK_2.0,
    KeyCode::Digit3 => VK_3.0,
    KeyCode::Digit4 => VK_4.0,
    KeyCode::Digit5 => VK_5.0,
    KeyCode::Digit6 => VK_6.0,
    KeyCode::Digit7 => VK_7.0,
    KeyCode::Digit8 => VK_8.0,
    KeyCode::Digit9 => VK_9.0,
    KeyCode::F1 => VK_F1.0,
    KeyCode::F2 => VK_F2.0,
    KeyCode::F3 => VK_F3.0,
    KeyCode::F4 => VK_F4.0,
    KeyCode::F5 => VK_F5.0,
    KeyCode::F6 => VK_F6.0,
    KeyCode::F7 => VK_F7.0,
    KeyCode::F8 => VK_F8.0,
    KeyCode::F9 => VK_F9.0,
    KeyCode::F10 => VK_F10.0,
    KeyCode::F11 => VK_F11.0,
    KeyCode::F12 => VK_F12.0,
    KeyCode::F13 => VK_F13.0,
    KeyCode::F14 => VK_F14.0,
    KeyCode::F15 => VK_F15.0,
    KeyCode::F16 => VK_F16.0,
    KeyCode::F17 => VK_F17.0,
    KeyCode::F18 => VK_F18.0,
    KeyCode::F19 => VK_F19.0,
    KeyCode::F20 => VK_F20.0,
    KeyCode::F21 => VK_F21.0,
    KeyCode::F22 => VK_F22.0,
    KeyCode::F23 => VK_F23.0,
    KeyCode::F24 => VK_F24.0,
    KeyCode::Numpad0 => VK_NUMPAD0.0,
    KeyCode::Numpad1 => VK_NUMPAD1.0,
    KeyCode::Numpad2 => VK_NUMPAD2.0,
    KeyCode::Numpad3 => VK_NUMPAD3.0,
    KeyCode::Numpad4 => VK_NUMPAD4.0,
    KeyCode::Numpad5 => VK_NUMPAD5.0,
    KeyCode::Numpad6 => VK_NUMPAD6.0,
    KeyCode::Numpad7 => VK_NUMPAD7.0,
    KeyCode::Numpad8 => VK_NUMPAD8.0,
    KeyCode::Numpad9 => VK_NUMPAD9.0,
    KeyCode::NumpadMultiply => VK_MULTIPLY.0,
    KeyCode::NumpadAdd => VK_ADD.0,
    KeyCode::NumpadSeparator => VK_SEPARATOR.0,
    KeyCode::NumpadSubtract => VK_SUBTRACT.0,
    KeyCode::NumpadDecimal => VK_DECIMAL.0,
    KeyCode::NumpadDivide => VK_DIVIDE.0,
    KeyCode::Ctrl | KeyCode::LCtrl => VK_LCONTROL.0,
    KeyCode::RCtrl => VK_RCONTROL.0,
    KeyCode::Shift | KeyCode::LShift => VK_LSHIFT.0,
    KeyCode::RShift => VK_RSHIFT.0,
    KeyCode::Alt | KeyCode::LAlt => VK_LMENU.0,
    KeyCode::RAlt => VK_RMENU.0,
    KeyCode::Win | KeyCode::LWin => VK_LWIN.0,
    KeyCode::RWin => VK_RWIN.0,
    KeyCode::Left | KeyCode::ArrowLeft => VK_LEFT.0,
    KeyCode::Up | KeyCode::ArrowUp => VK_UP.0,
    KeyCode::Right | KeyCode::ArrowRight => VK_RIGHT.0,
    KeyCode::Down | KeyCode::ArrowDown => VK_DOWN.0,
    KeyCode::Home => VK_HOME.0,
    KeyCode::End => VK_END.0,
    KeyCode::PageUp => VK_PRIOR.0,
    KeyCode::PageDown => VK_NEXT.0,
    KeyCode::Insert => VK_INSERT.0,
    KeyCode::Enter | KeyCode::Return => VK_RETURN.0,
    KeyCode::Tab => VK_TAB.0,
    KeyCode::Escape | KeyCode::Esc => VK_ESCAPE.0,
    KeyCode::Space | KeyCode::Spacebar => VK_SPACE.0,
    KeyCode::Backspace | KeyCode::Back => VK_BACK.0,
    KeyCode::Delete | KeyCode::Del => VK_DELETE.0,
    KeyCode::CapsLock | KeyCode::Caps => VK_CAPITAL.0,
    KeyCode::NumLock => VK_NUMLOCK.0,
    KeyCode::ScrollLock => VK_SCROLL.0,
    KeyCode::PrintScreen | KeyCode::PrtScr => VK_SNAPSHOT.0,
    KeyCode::Pause => VK_PAUSE.0,
    KeyCode::Break => VK_CANCEL.0,
    KeyCode::Apps | KeyCode::Menu => VK_APPS.0,
    KeyCode::Semicolon => VK_OEM_1.0,
    KeyCode::Equals => VK_OEM_PLUS.0,
    KeyCode::Comma => VK_OEM_COMMA.0,
    KeyCode::Minus => VK_OEM_MINUS.0,
    KeyCode::Period => VK_OEM_PERIOD.0,
    KeyCode::Slash => VK_OEM_2.0,
    KeyCode::Backtick => VK_OEM_3.0,
    KeyCode::LeftBracket => VK_OEM_4.0,
    KeyCode::Backslash => VK_OEM_5.0,
    KeyCode::RightBracket => VK_OEM_6.0,
    KeyCode::Quote => VK_OEM_7.0,
    KeyCode::VolumeUp => VK_VOLUME_UP.0,
    KeyCode::VolumeDown => VK_VOLUME_DOWN.0,
    KeyCode::VolumeMute => VK_VOLUME_MUTE.0,
    KeyCode::MediaNextTrack => VK_MEDIA_NEXT_TRACK.0,
    KeyCode::MediaPrevTrack => VK_MEDIA_PREV_TRACK.0,
    KeyCode::MediaStop => VK_MEDIA_STOP.0,
    KeyCode::MediaPlayPause => VK_MEDIA_PLAY_PAUSE.0,
    KeyCode::BrowserBack => VK_BROWSER_BACK.0,
    KeyCode::BrowserForward => VK_BROWSER_FORWARD.0,
    KeyCode::BrowserRefresh => VK_BROWSER_REFRESH.0,
    KeyCode::BrowserStop => VK_BROWSER_STOP.0,
    KeyCode::BrowserSearch => VK_BROWSER_SEARCH.0,
    KeyCode::BrowserFavorites => VK_BROWSER_FAVORITES.0,
    KeyCode::BrowserHome => VK_BROWSER_HOME.0,
    KeyCode::LaunchMail => VK_LAUNCH_MAIL.0,
    KeyCode::LaunchMediaSelect => VK_LAUNCH_MEDIA_SELECT.0,
    KeyCode::LaunchApp1 => VK_LAUNCH_APP1.0,
    KeyCode::LaunchApp2 => VK_LAUNCH_APP2.0,
    KeyCode::Clear => VK_CLEAR.0,
    KeyCode::Select => VK_SELECT.0,
    KeyCode::Print => VK_PRINT.0,
    KeyCode::Execute => VK_EXECUTE.0,
    KeyCode::Help => VK_HELP.0,
    KeyCode::Sleep => VK_SLEEP.0,
    KeyCode::LeftMouse => VK_LBUTTON.0,
    KeyCode::RightMouse => VK_RBUTTON.0,
    KeyCode::MiddleMouse => VK_MBUTTON.0,
    KeyCode::XButton1 => VK_XBUTTON1.0,
    KeyCode::XButton2 => VK_XBUTTON2.0,
  }
}

fn make_input(vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
  INPUT {
    r#type: INPUT_KEYBOARD,
    Anonymous: INPUT_0 {
      ki: KEYBDINPUT {
        wVk: VIRTUAL_KEY(vk),
        wScan: 0,
        dwFlags: flags,
        time: 0,
        dwExtraInfo: 0,
      },
    },
  }
}

fn send_unicode_char(ch: char) -> anyhow::Result<()> {
  let uv = ch as u16;
  unsafe {
    let inputs = [
      INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
          ki: KEYBDINPUT {
            wVk: VIRTUAL_KEY(0),
            wScan: uv,
            dwFlags: KEYBD_EVENT_FLAGS(4),
            time: 0,
            dwExtraInfo: 0,
          },
        },
      },
      INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
          ki: KEYBDINPUT {
            wVk: VIRTUAL_KEY(0),
            wScan: uv,
            dwFlags: KEYBD_EVENT_FLAGS(6),
            time: 0,
            dwExtraInfo: 0,
          },
        },
      },
    ];
    if SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) != 2 {
      return Err(anyhow::anyhow!("Failed to send unicode char"));
    }
  }
  Ok(())
}

pub fn send_text(text: &str) -> anyhow::Result<()> {
  for c in text.chars() {
    let use_vk = matches!(c, '0'..='9' | 'A'..='Z' | 'a'..='z');
    if use_vk {
      let vk = match c {
        '0'..='9' => 0x30 + (c as u8 - b'0') as u16,
        'A'..='Z' => 0x41 + (c as u8 - b'A') as u16,
        'a'..='z' => 0x41 + (c as u8 - b'a') as u16,
        _ => unreachable!(),
      };
      unsafe {
        SendInput(
          &[make_input(vk, KEYBD_EVENT_FLAGS(0))],
          std::mem::size_of::<INPUT>() as i32,
        );
        thread::sleep(Duration::from_millis(1));
        SendInput(&[make_input(vk, KEYEVENTF_KEYUP)], std::mem::size_of::<INPUT>() as i32);
      }
    } else {
      send_unicode_char(c)?;
    }
    thread::sleep(Duration::from_millis(20));
  }
  Ok(())
}

pub fn send_keys(keys: Vec<KeyCode>, modifiers: Option<Vec<KeyCode>>) -> anyhow::Result<()> {
  let main_vks: Vec<VIRTUAL_KEY> = keys.iter().map(|k| VIRTUAL_KEY(to_virtual_key(k))).collect();
  unsafe {
    if let Some(ref mods) = modifiers {
      for m in mods {
        SendInput(
          &[make_input(to_virtual_key(m), KEYBD_EVENT_FLAGS(0))],
          std::mem::size_of::<INPUT>() as i32,
        );
        thread::sleep(Duration::from_millis(20));
      }
    }
    for vk in &main_vks {
      SendInput(
        &[make_input(vk.0, KEYBD_EVENT_FLAGS(0))],
        std::mem::size_of::<INPUT>() as i32,
      );
      thread::sleep(Duration::from_millis(20));
    }
    thread::sleep(Duration::from_millis(50));
    for vk in main_vks.iter().rev() {
      SendInput(
        &[make_input(vk.0, KEYEVENTF_KEYUP)],
        std::mem::size_of::<INPUT>() as i32,
      );
      thread::sleep(Duration::from_millis(20));
    }
    if let Some(ref mods) = modifiers {
      for m in mods.iter().rev() {
        SendInput(
          &[make_input(to_virtual_key(m), KEYEVENTF_KEYUP)],
          std::mem::size_of::<INPUT>() as i32,
        );
        thread::sleep(Duration::from_millis(20));
      }
    }
  }
  Ok(())
}

pub fn key_down(key: KeyCode) -> anyhow::Result<()> {
  unsafe {
    SendInput(
      &[make_input(to_virtual_key(&key), KEYBD_EVENT_FLAGS(0))],
      std::mem::size_of::<INPUT>() as i32,
    );
  }
  Ok(())
}

pub fn key_release(key: KeyCode) -> anyhow::Result<()> {
  unsafe {
    SendInput(
      &[make_input(to_virtual_key(&key), KEYEVENTF_KEYUP)],
      std::mem::size_of::<INPUT>() as i32,
    );
  }
  Ok(())
}

pub fn send_text_by_hwnd(wnd: i64, position: Option<&InputPosition>, text: &str) -> anyhow::Result<InputResult> {
  let hwnd = HWND(wnd as *mut core::ffi::c_void);
  wnd::bring_window_to_front(hwnd);
  let mut result = InputResult {
    success: true,
    error: None,
    screen_x: 0,
    screen_y: 0,
    relative_x: 0,
    relative_y: 0,
  };
  if let Some(pos) = position {
    let coord = wnd::normalize_to_wnd_pos(wnd, pos.x, pos.y)?;
    result.screen_x = coord.screen_x;
    result.screen_y = coord.screen_y;
    result.relative_x = coord.relative_x;
    result.relative_y = coord.relative_y;
    if !wnd::check_pos_in_wnd(wnd, result.screen_x, result.screen_y)? {
      result.success = false;
      result.error = Some("Position outside window".to_string());
      return Ok(result);
    }
    click_by_pos(result.screen_x, result.screen_y, MouseButton::Left, false)?;
    thread::sleep(Duration::from_millis(50));
  }
  send_keys(vec![KeyCode::A], Some(vec![KeyCode::Ctrl]))?;
  thread::sleep(Duration::from_millis(50));
  send_keys(vec![KeyCode::Delete], None)?;
  thread::sleep(Duration::from_millis(50));
  send_text(text)?;
  Ok(result)
}

pub fn send_text_by_xpath(hwnd: i64, xpath: &str, text: &str) -> anyhow::Result<InputResult> {
  let element = find_first_element_by_xpath(hwnd, xpath)?;
  let pos = element.pos(Some(hwnd)).map_err(|e| anyhow::anyhow!("{}", e))?;
  let (sx, sy, rx, ry) = (pos.center_x, pos.center_y, pos.relative_center_x, pos.relative_center_y);
  if element.set_value(text) {
    return Ok(InputResult::success(sx, sy, rx, ry));
  }
  let nx = if pos.window_width > 0 {
    (rx as f64) / (pos.window_width as f64)
  } else {
    0.5
  }
  .clamp(0.0, 1.0);
  let ny = if pos.window_height > 0 {
    (ry as f64) / (pos.window_height as f64)
  } else {
    0.5
  }
  .clamp(0.0, 1.0);
  match send_text_by_hwnd(hwnd, Some(&InputPosition { x: nx, y: ny }), text) {
    Ok(mut r) => {
      r.screen_x = sx;
      r.screen_y = sy;
      r.relative_x = rx;
      r.relative_y = ry;
      Ok(r)
    }
    Err(e) => Ok(InputResult::fail_with_coords(
      format!("Text input failed: {}", e),
      sx,
      sy,
      rx,
      ry,
    )),
  }
}
