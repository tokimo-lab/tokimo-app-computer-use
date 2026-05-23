use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum AudioAction {
  /// List all audio devices
  List {
    /// Filter by type: output or input
    #[arg(long)]
    r#type: Option<String>,
  },
  /// Get details of an audio device by index
  Detail { index: usize },
  /// Set system volume (0-100)
  SetVolume {
    /// Volume level (0-100)
    level: u32,
    /// Device index (omit for default output)
    #[arg(long)]
    index: Option<usize>,
  },
  /// Get current system volume
  GetVolume {
    /// Device index (omit for default output)
    #[arg(long)]
    index: Option<usize>,
  },
  /// Mute or unmute system audio
  Mute {
    /// Unmute instead of mute
    #[arg(long)]
    off: bool,
    /// Device index (omit for default output)
    #[arg(long)]
    index: Option<usize>,
  },
  /// Get current mute state
  GetMute {
    /// Device index (omit for default output)
    #[arg(long)]
    index: Option<usize>,
  },
  /// Set default audio device
  SetDefault {
    /// Device index from `audio list`
    index: usize,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: AudioAction) -> Result<()> {
  match action {
    AudioAction::List { r#type } => {
      let r = executor.call("system.info", json!({}))?;
      let audios = r["audio_devices"].as_array();
      let Some(arr) = audios else { println!("No audio devices found."); return Ok(()) };
      if arr.is_empty() { println!("No audio devices found."); return Ok(()); }
      let filter = r#type.as_deref().map(|s| s.to_lowercase());
      let mut t = super::Table::new(vec![("IDX", 4), ("NAME", 40), ("TYPE", 9), ("VOL", 5), ("MUTE", 4), ("DEFAULT", 7)]);
      let mut display_idx = 0usize;
      for (i, a) in arr.iter().enumerate() {
        let dtype = a["device_type"].as_str().unwrap_or("");
        if let Some(ref f) = filter {
          if dtype != f.as_str() { continue; }
        }
        let name = a["name"].as_str().unwrap_or("?");
        let vol = a["volume"].as_u64().unwrap_or(0);
        let muted = if a["muted"].as_bool().unwrap_or(false) { "M" } else { "" };
        let is_default = if a["is_default"].as_bool().unwrap_or(false) { "*" } else { "" };
        t.row(vec![i.to_string(), name.to_string(), dtype.to_string(), format!("{vol}%"), muted.to_string(), is_default.to_string()]);
        display_idx += 1;
      }
      if display_idx == 0 {
        println!("No audio devices found for type '{}'.", filter.unwrap_or_default());
      } else {
        t.print();
      }
    }
    AudioAction::Detail { index } => {
      let r = executor.call("system.info", json!({}))?;
      let arr = r["audio_devices"].as_array().ok_or_else(|| anyhow::anyhow!("No audio devices found"))?;
      let a = arr.get(index).ok_or_else(|| anyhow::anyhow!("Audio index {index} out of range"))?;
      super::kv_print(&[
        ("Name:", a["name"].as_str().unwrap_or("?")),
        ("ID:", a["device_id"].as_str().unwrap_or("")),
        ("Type:", a["device_type"].as_str().unwrap_or("")),
        ("Volume:", &format!("{}%", a["volume"].as_u64().unwrap_or(0))),
        ("Muted:", if a["muted"].as_bool().unwrap_or(false) { "Yes" } else { "No" }),
        ("Default:", if a["is_default"].as_bool().unwrap_or(false) { "Yes" } else { "No" }),
      ]);
    }
    AudioAction::SetVolume { level, index } => {
      let mut params = json!({"level": level});
      if let Some(idx) = index {
        params["device_index"] = json!(idx);
      }
      executor.call("audio.set_volume", params)?;
      println!("ok");
    }
    AudioAction::GetVolume { index } => {
      let mut params = json!({});
      if let Some(idx) = index {
        params["device_index"] = json!(idx);
      }
      let r = executor.call("audio.get_volume", params)?;
      let level = r["level"].as_u64().unwrap_or(0);
      println!("{level}%");
    }
    AudioAction::Mute { off, index } => {
      let mut params = json!({"muted": !off});
      if let Some(idx) = index {
        params["device_index"] = json!(idx);
      }
      executor.call("audio.set_mute", params)?;
      println!("ok");
    }
    AudioAction::GetMute { index } => {
      let mut params = json!({});
      if let Some(idx) = index {
        params["device_index"] = json!(idx);
      }
      let r = executor.call("audio.get_mute", params)?;
      let muted = r["muted"].as_bool().unwrap_or(false);
      println!("{}", if muted { "muted" } else { "unmuted" });
    }
    AudioAction::SetDefault { index } => {
      let r = executor.call("system.info", json!({}))?;
      let arr = r["audio_devices"].as_array()
        .ok_or_else(|| anyhow::anyhow!("No audio devices found"))?;
      let device = arr.get(index)
        .ok_or_else(|| anyhow::anyhow!("Audio index {index} out of range"))?;
      let device_id = device["device_id"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing device_id"))?;
      executor.call("audio.set_default", json!({"device_id": device_id}))?;
      println!("ok");
    }
  }
  Ok(())
}
