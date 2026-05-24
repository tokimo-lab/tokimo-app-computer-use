use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::{CommandExecutor, print_input_result, scope::WindowSel};

#[derive(Subcommand, Debug)]
pub enum MouseAction {
  /// Get current cursor position
  Pos,
  /// Move cursor to absolute screen coordinates
  Move { x: i32, y: i32 },
  /// Click at screen coordinates
  Click {
    x: f64,
    y: f64,
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long, default_value = "left")]
    button: String,
    #[arg(long)]
    double: bool,
  },
  /// Drag from one point to another
  Drag {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long, default_value = "left")]
    button: String,
  },
  /// Scroll at screen coordinates
  Scroll {
    x: f64,
    y: f64,
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long, default_value = "0")]
    dx: i32,
    #[arg(long, default_value = "0")]
    dy: i32,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: MouseAction) -> Result<()> {
  match action {
    MouseAction::Pos => {
      let r = executor.call("mouse.pos", json!({}))?;
      println!("{},{}", r["x"].as_i64().unwrap_or(0), r["y"].as_i64().unwrap_or(0));
    }
    MouseAction::Move { x, y } => {
      executor.call("mouse.move", json!({"x": x, "y": y}))?;
      println!("ok");
    }
    MouseAction::Click {
      x,
      y,
      sel,
      button,
      double,
    } => {
      let mut params = sel.to_json_scope();
      params["x"] = json!(x);
      params["y"] = json!(y);
      params["button"] = json!(button);
      params["double"] = json!(double);
      let r = executor.call("mouse.click", params)?;
      print_input_result(&r);
    }
    MouseAction::Drag {
      x1,
      y1,
      x2,
      y2,
      sel,
      button,
    } => {
      let mut params = sel.to_json_scope();
      params["x1"] = json!(x1);
      params["y1"] = json!(y1);
      params["x2"] = json!(x2);
      params["y2"] = json!(y2);
      params["button"] = json!(button);
      let r = executor.call("mouse.drag", params)?;
      print_input_result(&r);
    }
    MouseAction::Scroll { x, y, sel, dx, dy } => {
      let mut params = sel.to_json_scope();
      params["x"] = json!(x);
      params["y"] = json!(y);
      params["dx"] = json!(dx);
      params["dy"] = json!(dy);
      executor.call("mouse.scroll", params)?;
      println!("ok");
    }
  }
  Ok(())
}
