use anyhow::{Result, anyhow};
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum ScreenshotAction {
  Desktop {
    #[arg(long, default_value = "webp")]
    format: String,
    #[arg(long, default_value = "80")]
    quality: u8,
    #[arg(long)]
    output: Option<String>,
  },
  Window {
    #[arg(long, short = 'w')]
    handle: Option<i64>,
    #[arg(long, default_value = "webp")]
    format: String,
    #[arg(long, default_value = "80")]
    quality: u8,
    #[arg(long)]
    output: Option<String>,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: ScreenshotAction) -> Result<()> {
  match action {
    ScreenshotAction::Desktop {
      format,
      quality,
      output,
    } => {
      let result = executor.call("screenshot.desktop", json!({"format": format, "quality": quality}))?;
      save_screenshot(&result, output, &format)?;
    }
    ScreenshotAction::Window {
      handle,
      format,
      quality,
      output,
    } => {
      let mut params = json!({"format": format, "quality": quality});
      if let Some(h) = handle {
        params["handle"] = json!(h);
      }
      let result = executor.call("screenshot.window", params)?;
      save_screenshot(&result, output, &format)?;
    }
  }
  Ok(())
}

fn save_screenshot(result: &serde_json::Value, output: Option<String>, format: &str) -> Result<()> {
  let data_b64 = result
    .get("data")
    .and_then(|v| v.as_str())
    .ok_or_else(|| anyhow!("missing 'data' in screenshot response"))?;
  use base64::Engine;
  let bytes = base64::engine::general_purpose::STANDARD.decode(data_b64)?;
  let path = output.unwrap_or_else(|| format!("screenshot.{format}"));
  std::fs::write(&path, &bytes)?;
  eprintln!("saved {} bytes to {path}", bytes.len());
  Ok(())
}
