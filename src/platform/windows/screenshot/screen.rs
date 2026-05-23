use crate::platform::windows::screenshot::utils::encode_to_image_format;
use crate::types::{ScreenshotConfig, ScreenshotOptions};
use anyhow::Result;
use std::ffi::c_void;
use windows::Win32::Graphics::Gdi::*;

pub struct ScreenCapture {
  hdc_screen: HDC,
}

impl ScreenCapture {
  pub fn new() -> Result<Self> {
    unsafe {
      let hdc = GetDC(None);
      if hdc.is_invalid() {
        return Err(anyhow::anyhow!("Failed to get screen DC"));
      }
      Ok(Self { hdc_screen: hdc })
    }
  }

  pub fn capture_region(&self, x: i32, y: i32, width: i32, height: i32, config: &ScreenshotConfig) -> Result<Vec<u8>> {
    if width <= 0 || height <= 0 {
      return Err(anyhow::anyhow!("Invalid dimensions"));
    }
    unsafe {
      let hdc_mem = CreateCompatibleDC(Some(self.hdc_screen));
      if hdc_mem.is_invalid() {
        return Err(anyhow::anyhow!("CreateCompatibleDC failed"));
      }
      let hbitmap = CreateCompatibleBitmap(self.hdc_screen, width, height);
      if hbitmap.is_invalid() {
        let _ = DeleteDC(hdc_mem);
        return Err(anyhow::anyhow!("CreateCompatibleBitmap failed"));
      }
      let old = SelectObject(hdc_mem, hbitmap.into());
      if BitBlt(hdc_mem, 0, 0, width, height, Some(self.hdc_screen), x, y, SRCCOPY).is_err() {
        SelectObject(hdc_mem, old);
        let _ = DeleteObject(hbitmap.into());
        let _ = DeleteDC(hdc_mem);
        return Err(anyhow::anyhow!("BitBlt failed"));
      }
      let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
          biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
          biWidth: width,
          biHeight: -height,
          biPlanes: 1,
          biBitCount: 32,
          biCompression: BI_RGB.0,
          ..Default::default()
        },
        ..Default::default()
      };
      let mut data = vec![0u8; (width * height * 4) as usize];
      let lines = GetDIBits(
        hdc_mem,
        hbitmap,
        0,
        height.unsigned_abs(),
        Some(data.as_mut_ptr() as *mut c_void),
        &mut bmi,
        DIB_RGB_COLORS,
      );
      SelectObject(hdc_mem, old);
      let _ = DeleteObject(hbitmap.into());
      let _ = DeleteDC(hdc_mem);
      if lines == 0 {
        return Err(anyhow::anyhow!("GetDIBits failed"));
      }
      let format = config.get_format();
      let quality = if config.get_quality() < 100 {
        Some(config.get_quality())
      } else {
        None
      };
      encode_to_image_format(&data, width as u32, height as u32, format, quality, true)
    }
  }
}

impl Drop for ScreenCapture {
  fn drop(&mut self) {
    unsafe {
      ReleaseDC(None, self.hdc_screen);
    }
  }
}
