use image::{DynamicImage, ImageBuffer, ImageEncoder};
use std::io::Cursor;
use webp::{Encoder, WebPMemory};

pub fn encode_to_image_format(
  pixel_data: &[u8],
  width: u32,
  height: u32,
  format: &str,
  quality: Option<u8>,
  is_bgra: bool,
) -> anyhow::Result<Vec<u8>> {
  match format {
    "webp" => {
      let mut rgb = Vec::with_capacity((width * height * 3) as usize);
      for chunk in pixel_data.chunks_exact(4) {
        if is_bgra {
          rgb.push(chunk[2]);
          rgb.push(chunk[1]);
          rgb.push(chunk[0]);
        } else {
          rgb.extend_from_slice(&chunk[0..3]);
        }
      }
      let enc = Encoder::from_rgb(&rgb, width, height);
      let mem: WebPMemory = enc.encode(quality.unwrap_or(80) as f32);
      Ok(mem.to_vec())
    }
    "jpeg" | "jpg" => {
      let mut rgb = Vec::with_capacity((width * height * 3) as usize);
      for chunk in pixel_data.chunks_exact(4) {
        if is_bgra {
          rgb.push(chunk[2]);
          rgb.push(chunk[1]);
          rgb.push(chunk[0]);
        } else {
          rgb.push(chunk[0]);
          rgb.push(chunk[1]);
          rgb.push(chunk[2]);
        }
      }
      let mut out = Vec::new();
      let mut cursor = Cursor::new(&mut out);
      let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality.unwrap_or(80));
      enc
        .write_image(&rgb, width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| anyhow::anyhow!("JPEG: {}", e))?;
      Ok(out)
    }
    "png" => {
      let rgba = if is_bgra {
        let mut r = Vec::with_capacity(pixel_data.len());
        for chunk in pixel_data.chunks_exact(4) {
          r.push(chunk[2]);
          r.push(chunk[1]);
          r.push(chunk[0]);
          r.push(chunk[3]);
        }
        r
      } else {
        pixel_data.to_vec()
      };
      let img = ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, rgba)
        .ok_or_else(|| anyhow::anyhow!("ImageBuffer"))?;
      let mut out = Vec::new();
      DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| anyhow::anyhow!("PNG: {}", e))?;
      Ok(out)
    }
    _ => Err(anyhow::anyhow!("Unsupported format: {}", format)),
  }
}
