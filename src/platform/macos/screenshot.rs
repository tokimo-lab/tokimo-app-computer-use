use block2::RcBlock;
use core_graphics::image::CGImage;
use foreign_types::ForeignType;
use objc2::AnyThread;
use objc2_foundation::NSError;
use objc2_screen_capture_kit::{
  SCContentFilter, SCDisplay, SCScreenshotManager, SCShareableContent, SCStreamConfiguration,
};
use std::sync::mpsc;
use std::time::Duration;

use crate::error::Result;
use crate::types::*;

type SCKitCGImage = objc2_core_graphics::CGImage;

const TIMEOUT: Duration = Duration::from_secs(5);

fn convert_cgimage(sckit_image: &SCKitCGImage) -> CGImage {
  // The CGImage from ScreenCaptureKit is autoreleased - we must retain it
  // before the completion handler returns.
  let non_null: std::ptr::NonNull<SCKitCGImage> =
    unsafe { std::ptr::NonNull::new_unchecked(sckit_image as *const _ as *mut _) };
  let retained = unsafe { objc2_core_foundation::CFRetained::retain(non_null) };
  let ptr = objc2_core_foundation::CFRetained::as_ptr(&retained).as_ptr();
  // Prevent retained from being dropped (which would release the CGImage).
  // Leak it so the CGImage stays alive for the caller.
  std::mem::forget(retained);
  unsafe { CGImage::from_ptr(ptr as *mut core_graphics::sys::CGImage) }
}

fn cg_image_to_png(image: &CGImage) -> Result<Vec<u8>> {
  let width = image.width();
  let height = image.height();
  if width == 0 || height == 0 {
    return Err(anyhow::anyhow!("empty image: {width}x{height}"));
  }

  // Read raw pixel data directly from CGImage
  let cf_data = image.data();
  let raw = cf_data.bytes();
  let bpr = image.bytes_per_row();
  let bpp = image.bits_per_pixel();
  let bpc = image.bits_per_component();

  if bpp != 32 || bpc != 8 {
    return Err(anyhow::anyhow!("unsupported pixel format: {bpp}bpp, {bpc}bpc"));
  }

  // ScreenCaptureKit returns BGRA → convert to RGBA
  let mut rgba = Vec::with_capacity(width * height * 4);
  for row in 0..height {
    let row_start = row * bpr;
    for col in 0..width {
      let px = row_start + col * 4;
      if px + 3 < raw.len() {
        rgba.push(raw[px + 2]); // R
        rgba.push(raw[px + 1]); // G
        rgba.push(raw[px]);     // B
        rgba.push(raw[px + 3]); // A
      }
    }
  }

  use image::{ImageBuffer, Rgba};
  let buf: ImageBuffer<Rgba<u8>, Vec<u8>> =
    ImageBuffer::from_raw(width as u32, height as u32, rgba)
      .ok_or_else(|| anyhow::anyhow!("failed to create image buffer"))?;
  let mut out = Vec::new();
  buf.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
    .map_err(|e| anyhow::anyhow!("PNG encoding failed: {e}"))?;
  Ok(out)
}

fn get_shareable_content() -> Result<objc2::rc::Retained<SCShareableContent>> {
  let (tx, rx) = mpsc::channel();

  let block = RcBlock::new(move |content: *mut SCShareableContent, _error: *mut NSError| {
    if !content.is_null() {
      let retained = unsafe { objc2::rc::Retained::retain(content) };
      let _ = tx.send(retained.ok_or_else(|| anyhow::anyhow!("retain returned None")));
    } else {
      let _ = tx.send(Err(anyhow::anyhow!("getShareableContent returned null")));
    }
  });

  unsafe {
    SCShareableContent::getShareableContentWithCompletionHandler(&block);
  }

  match rx.recv_timeout(TIMEOUT) {
    Ok(Ok(content)) => Ok(content),
    Ok(Err(e)) => Err(e),
    Err(mpsc::RecvTimeoutError::Timeout) => Err(anyhow::anyhow!(
      "SCShareableContent timed out — check Screen Recording permission"
    )),
    Err(e) => Err(anyhow::anyhow!("channel error: {e}")),
  }
}

fn capture_display(display: &SCDisplay) -> Result<CGImage> {
  let empty = objc2_foundation::NSArray::array();
  let filter = unsafe {
    SCContentFilter::initWithDisplay_excludingWindows(SCContentFilter::alloc(), display, &empty)
  };
  let config = unsafe { SCStreamConfiguration::init(SCStreamConfiguration::alloc()) };

  let (tx, rx) = mpsc::channel();

  let block = RcBlock::new(move |image: *mut SCKitCGImage, error: *mut NSError| {
    if !image.is_null() {
      let _ = tx.send(Ok(convert_cgimage(unsafe { &*image })));
    } else {
      let msg = if !error.is_null() {
        format!("capture error: {}", unsafe { (*error).localizedDescription() })
      } else {
        "capture returned null image".to_string()
      };
      let _ = tx.send(Err(anyhow::anyhow!(msg)));
    }
  });

  unsafe {
    SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
      &filter, &config, Some(&block),
    );
  }

  match rx.recv_timeout(TIMEOUT) {
    Ok(result) => result,
    Err(mpsc::RecvTimeoutError::Timeout) => Err(anyhow::anyhow!("display capture timed out")),
    Err(e) => Err(anyhow::anyhow!("channel error: {e}")),
  }
}

pub fn take_desktop_screenshot(config: Option<&ScreenshotConfig>) -> Result<Vec<u8>> {
  let content = get_shareable_content()?;
  let displays = unsafe { content.displays() };
  if displays.len() == 0 {
    return Err(anyhow::anyhow!(
      "no displays — grant Screen Recording in System Settings > Privacy & Security"
    ));
  }
  let display = displays.objectAtIndex(0);
  let cg_image = capture_display(&display)?;

  if let Some(cfg) = config {
    if let Some((l, t, r, b)) = cfg.get_rect() {
      let rect = core_graphics::geometry::CGRect::new(
        &core_graphics::geometry::CGPoint::new(l as f64, t as f64),
        &core_graphics::geometry::CGSize::new((r - l) as f64, (b - t) as f64),
      );
      let cropped = cg_image.cropped(rect)
        .ok_or_else(|| anyhow::anyhow!("crop failed"))?;
      return cg_image_to_png(&cropped);
    }
  }
  cg_image_to_png(&cg_image)
}

pub fn take_window_screenshot(handle: &WindowHandle) -> Result<Vec<u8>> {
  // BUG-02: Use SCContentFilter::initWithDesktopIndependentWindow_ for window capture
  // instead of capturing the full display and cropping.
  let content = get_shareable_content()?;
  let windows = unsafe { content.windows() };
  let sc_window = (0..windows.len())
    .map(|i| windows.objectAtIndex(i))
    .find(|w| unsafe { w.windowID() } == handle.0 as u32)
    .ok_or_else(|| anyhow::anyhow!("window {} not found in shareable content", handle.0))?;

  let filter = unsafe {
    SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &sc_window)
  };
  let config = unsafe { SCStreamConfiguration::init(SCStreamConfiguration::alloc()) };

  // Default SCStreamConfiguration has width=height=0, producing a blank image.
  // SCWindow.frame() can also return a misleading minimal frame for layered
  // windows — read the authoritative size from our own window list.
  let (w_pts, h_pts) = crate::platform::macos::window::list_windows()
    .ok()
    .and_then(|ws| ws.into_iter().find(|w| w.hwnd == handle.0).map(|w| (w.width as usize, w.height as usize)))
    .unwrap_or((0, 0));
  if w_pts > 0 && h_pts > 0 {
    let scale: usize = 2; // Retina backing
    unsafe {
      config.setWidth(w_pts * scale);
      config.setHeight(h_pts * scale);
    }
  }

  let (tx, rx) = mpsc::channel();

  let block = RcBlock::new(move |image: *mut SCKitCGImage, error: *mut NSError| {
    if !image.is_null() {
      let _ = tx.send(Ok(convert_cgimage(unsafe { &*image })));
    } else {
      let msg = if !error.is_null() {
        format!("capture error: {}", unsafe { (*error).localizedDescription() })
      } else {
        "capture returned null image".to_string()
      };
      let _ = tx.send(Err(anyhow::anyhow!(msg)));
    }
  });

  unsafe {
    SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
      &filter, &config, Some(&block),
    );
  }

  let cg_image = match rx.recv_timeout(TIMEOUT) {
    Ok(result) => result?,
    Err(mpsc::RecvTimeoutError::Timeout) => return Err(anyhow::anyhow!("window capture timed out")),
    Err(e) => return Err(anyhow::anyhow!("channel error: {e}")),
  };

  cg_image_to_png(&cg_image)
}
