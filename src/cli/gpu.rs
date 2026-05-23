use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::{CommandExecutor, format_bytes};

#[derive(Subcommand, Debug)]
pub enum GpuAction {
  /// Show GPU information
  Info,
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: GpuAction) -> Result<()> {
  match action {
    GpuAction::Info => {
      let r = executor.call("system.info", json!({}))?;
      let gpus = r["gpus"].as_array();
      let Some(arr) = gpus else { println!("No GPU found."); return Ok(()) };
      if arr.is_empty() { println!("No GPU found."); return Ok(()); }
      for (i, g) in arr.iter().enumerate() {
        if i > 0 { println!(); }
        println!("GPU {}:", i);
        println!("  Name:              {}", g["name"].as_str().unwrap_or("?"));

        let driver = g["driver_version"].as_str().unwrap_or("");
        if !driver.is_empty() {
          println!("  Driver:            {}", driver);
        }
        let provider = g["provider_name"].as_str().unwrap_or("");
        if !provider.is_empty() {
          println!("  Provider:          {}", provider);
        }
        let date = g["driver_date"].as_str().unwrap_or("");
        if !date.is_empty() {
          println!("  Driver Date:       {}", date);
        }

        let vendor = g["vendor_id"].as_u64().unwrap_or(0);
        let device = g["device_id"].as_u64().unwrap_or(0);
        if vendor > 0 {
          println!("  Vendor ID:         0x{:04X} ({})", vendor, pci_vendor_name(vendor as u32));
        }
        if device > 0 {
          println!("  Device ID:         0x{:04X}", device);
        }

        let dedicated = g["dedicated_video_memory"].as_u64().unwrap_or(0);
        let shared = g["shared_system_memory"].as_u64().unwrap_or(0);
        if dedicated > 0 {
          println!("  Dedicated VRAM:    {}", format_bytes(dedicated));
        }
        if shared > 0 {
          println!("  Shared Memory:     {}", format_bytes(shared));
        }
        if dedicated == 0 && shared == 0 {
          let vram = g["vram_bytes"].as_u64().unwrap_or(0);
          if vram > 0 {
            println!("  VRAM:              {}", format_bytes(vram));
          }
        }

        if g["is_software"].as_bool().unwrap_or(false) {
          println!("  Type:              Software (WARP)");
        }
        if g["is_remote"].as_bool().unwrap_or(false) {
          println!("  Type:              Remote");
        }
      }
    }
  }
  Ok(())
}

fn pci_vendor_name(vendor_id: u32) -> &'static str {
  match vendor_id {
    0x10DE => "NVIDIA",
    0x1002 => "AMD",
    0x8086 => "Intel",
    0x1414 => "Microsoft",
    0x5333 => "S3 Graphics",
    0x102B => "Matrox",
    0x1106 => "VIA",
    0x1039 => "SiS",
    _ => "Unknown",
  }
}
