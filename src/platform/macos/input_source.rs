//! Temporarily switch the macOS keyboard input source to an ASCII-capable one
//! (e.g. ABC / U.S.) while synthesizing text so that Chinese / Japanese / Korean
//! IMEs do not intercept and re-interpret our CGEvent keystrokes.
//!
//! Uses Carbon's Text Input Source Services (TIS) APIs:
//! - `TISCopyCurrentKeyboardInputSource` — snapshot the user's current source
//! - `TISCopyCurrentASCIICapableKeyboardInputSource` — pick the best ASCII source
//! - `TISSelectInputSource` — swap to a given source
//!
//! Reference: <https://developer.apple.com/documentation/coreservices/text_input_sources>

use std::ffi::c_void;

#[allow(non_camel_case_types)]
type TISInputSourceRef = *mut c_void;
#[allow(non_camel_case_types)]
type OSStatus = i32;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
  fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
  fn TISCopyCurrentASCIICapableKeyboardInputSource() -> TISInputSourceRef;
  fn TISSelectInputSource(input_source: TISInputSourceRef) -> OSStatus;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
  fn CFRelease(cf: *const c_void);
}

/// Guard that restores the previously-selected input source on drop.
///
/// While this guard is alive the system keyboard input source is the
/// ASCII-capable one returned by `TISCopyCurrentASCIICapableKeyboardInputSource`,
/// which on systems with a Pinyin/Bopomofo IME active typically maps to "ABC"
/// or "U.S.". Restoration is best-effort: if either the saved or new ref is
/// null the guard becomes a no-op.
pub struct AsciiInputGuard {
  previous: TISInputSourceRef,
}

impl AsciiInputGuard {
  /// Enter ASCII input mode. Returns `None` if TIS APIs fail (in which case
  /// callers should proceed without guarding — better to type than to error).
  pub fn enter() -> Option<Self> {
    unsafe {
      let previous = TISCopyCurrentKeyboardInputSource();
      let ascii = TISCopyCurrentASCIICapableKeyboardInputSource();
      if ascii.is_null() {
        if !previous.is_null() {
          CFRelease(previous as *const c_void);
        }
        return None;
      }
      // If we're already on the ASCII source, we still take a no-op guard so
      // the caller's code path is uniform.
      let status = TISSelectInputSource(ascii);
      CFRelease(ascii as *const c_void);
      if status != 0 {
        if !previous.is_null() {
          CFRelease(previous as *const c_void);
        }
        return None;
      }
      // Give the system a beat to actually swap the source before keystrokes
      // start flowing — empirically Chrome / Electron need ~30ms otherwise the
      // first character is still consumed by the old IME's composition buffer.
      std::thread::sleep(std::time::Duration::from_millis(30));
      Some(Self { previous })
    }
  }
}

impl Drop for AsciiInputGuard {
  fn drop(&mut self) {
    if self.previous.is_null() {
      return;
    }
    unsafe {
      let _ = TISSelectInputSource(self.previous);
      CFRelease(self.previous as *const c_void);
    }
  }
}
