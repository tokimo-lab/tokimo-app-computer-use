pub mod screen;
pub mod utils;

use crate::platform::windows::screenshot::screen::ScreenCapture as GdiCapture;
use crate::types::{ScreenshotConfig, WindowScreenshotConfig};

use crate::platform::windows::wnd;
use anyhow::Result;
use windows::Win32::Foundation::*;

pub fn take_desktop_screenshot(config: Option<&ScreenshotConfig>) -> Result<Vec<u8>> {
  let default = ScreenshotConfig::default();
  let config = config.unwrap_or(&default);
  let rect = match config.get_rect() {
    Some((l, t, r, b)) => RECT {
      left: l,
      top: t,
      right: r,
      bottom: b,
    },
    None => {
      let (w, h) = crate::platform::windows::system_info::get_screen_size()?;
      RECT {
        left: 0,
        top: 0,
        right: w,
        bottom: h,
      }
    }
  };
  let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
  if w <= 0 || h <= 0 {
    return Err(anyhow::anyhow!("Invalid dimensions: {}x{}", w, h));
  }
  capture_region(rect, config)
}

pub fn take_hwnd_screenshot(wnd_handle: i64, config: Option<&WindowScreenshotConfig>) -> Result<Vec<u8>> {
  let hwnd = HWND(wnd_handle as *mut core::ffi::c_void);
  wnd::bring_window_to_front(hwnd);
  let r = wnd::get_wnd_rect(wnd_handle)?;
  if r.w <= 0 || r.h <= 0 {
    return Err(anyhow::anyhow!("Invalid dimensions: {}x{}", r.w, r.h));
  }
  let default = WindowScreenshotConfig::default();
  let config = config.unwrap_or(&default);
  let sc = ScreenshotConfig {
    left: None,
    top: None,
    right: None,
    bottom: None,
    quality: config.quality,
    format: config.format.clone(),
  };
  let rect = RECT {
    left: r.x,
    top: r.y,
    right: r.x + r.w,
    bottom: r.y + r.h,
  };
  capture_region(rect, &sc)
}

fn capture_region(rect: RECT, config: &ScreenshotConfig) -> Result<Vec<u8>> {
  let (x, y, w, h) = (rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top);
  if w <= 0 || h <= 0 {
    return Err(anyhow::anyhow!("Invalid dimensions"));
  }
  let cap = GdiCapture::new()?;
  cap.capture_region(x, y, w, h, config)
}
