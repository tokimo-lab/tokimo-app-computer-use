use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::{CommandExecutor, print_input_result};

#[derive(Subcommand, Debug)]
pub enum MouseAction {
  Move {
    x: i32,
    y: i32,
  },
  Position,
  Click {
    x: f64,
    y: f64,
    #[arg(long, short = 'w')]
    handle: Option<i64>,
    #[arg(long, default_value = "left")]
    button: String,
    #[arg(long)]
    double_click: bool,
  },
  ClickXpath {
    xpath: String,
    #[arg(long, short = 'w')]
    handle: Option<i64>,
    #[arg(long, default_value = "left")]
    button: String,
    #[arg(long)]
    double_click: bool,
  },
  Drag {
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
    #[arg(long, short = 'w')]
    handle: Option<i64>,
    #[arg(long, default_value = "left")]
    button: String,
  },
  Scroll {
    x: f64,
    y: f64,
    #[arg(long, short = 'w')]
    handle: Option<i64>,
    #[arg(long, default_value = "0")]
    delta_x: i32,
    #[arg(long, default_value = "0")]
    delta_y: i32,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: MouseAction) -> Result<()> {
  match action {
    MouseAction::Move { x, y } => {
      executor.call("mouse.move_cursor", json!({"x": x, "y": y}))?;
      println!("ok");
    }
    MouseAction::Position => {
      let r = executor.call("mouse.get_position", json!({}))?;
      println!("{},{}", r["x"].as_i64().unwrap_or(0), r["y"].as_i64().unwrap_or(0));
    }
    MouseAction::Click {
      handle,
      x,
      y,
      button,
      double_click,
    } => {
      let mut params = json!({"x": x, "y": y, "button": button, "double_click": double_click});
      if let Some(h) = handle {
        params["handle"] = json!(h);
      }
      let r = executor.call("mouse.click", params)?;
      print_input_result(&r);
    }
    MouseAction::ClickXpath {
      handle,
      xpath,
      button,
      double_click,
    } => {
      let mut params = json!({"xpath": xpath, "button": button, "double_click": double_click});
      if let Some(h) = handle {
        params["handle"] = json!(h);
      }
      let r = executor.call("mouse.click_by_xpath", params)?;
      print_input_result(&r);
    }
    MouseAction::Drag {
      handle,
      from_x,
      from_y,
      to_x,
      to_y,
      button,
    } => {
      let mut params = json!({"from_x": from_x, "from_y": from_y, "to_x": to_x, "to_y": to_y, "button": button});
      if let Some(h) = handle {
        params["handle"] = json!(h);
      }
      let r = executor.call("mouse.drag", params)?;
      print_input_result(&r);
    }
    MouseAction::Scroll {
      handle,
      x,
      y,
      delta_x,
      delta_y,
    } => {
      let mut params = json!({"x": x, "y": y, "delta_x": delta_x, "delta_y": delta_y});
      if let Some(h) = handle {
        params["handle"] = json!(h);
      }
      executor.call("mouse.scroll", params)?;
      println!("ok");
    }
  }
  Ok(())
}
