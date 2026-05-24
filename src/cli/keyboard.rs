use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json::json;

use super::{CommandExecutor, print_input_result, scope::WindowSel};

#[derive(Subcommand, Debug)]
pub enum KeyboardAction {
  /// Type text into a window or element
  Type {
    text: String,
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long)]
    x: Option<f64>,
    #[arg(long)]
    y: Option<f64>,
    /// Press Enter after typing
    #[arg(long)]
    enter: bool,
    /// Clear the field before typing
    #[arg(long)]
    clear: bool,
  },
  /// Press a key combination (e.g. "Ctrl+C", "Alt+F4")
  Press { combo: String },
  /// Hold a key down
  Down { key: String },
  /// Release a key
  Up { key: String },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: KeyboardAction) -> Result<()> {
  match action {
    KeyboardAction::Type {
      text,
      sel,
      x,
      y,
      enter,
      clear,
    } => {
      let mut params = sel.to_json_scope();
      params["text"] = json!(text);
      params["enter"] = json!(enter);
      params["clear"] = json!(clear);
      if let (Some(px), Some(py)) = (x, y) {
        params["x"] = json!(px);
        params["y"] = json!(py);
      }
      let r = executor.call("keyboard.type", params)?;
      print_input_result(&r);
    }
    KeyboardAction::Press { combo } => {
      executor.call("keyboard.press", json!({"combo": combo}))?;
      println!("ok");
    }
    KeyboardAction::Down { key } => {
      let key_val: serde_json::Value = serde_json::from_str(&format!("\"{key}\"")).context("invalid key name")?;
      executor.call("keyboard.key_down", json!({"key": key_val}))?;
      println!("ok");
    }
    KeyboardAction::Up { key } => {
      let key_val: serde_json::Value = serde_json::from_str(&format!("\"{key}\"")).context("invalid key name")?;
      executor.call("keyboard.key_up", json!({"key": key_val}))?;
      println!("ok");
    }
  }
  Ok(())
}
