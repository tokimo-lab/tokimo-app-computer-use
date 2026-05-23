use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json::json;

use super::{CommandExecutor, print_input_result};

#[derive(Subcommand, Debug)]
pub enum KeyboardAction {
  Type {
    text: String,
    #[arg(long, short = 'w')]
    handle: Option<i64>,
    #[arg(long)]
    x: Option<f64>,
    #[arg(long)]
    y: Option<f64>,
  },
  TypeXpath {
    xpath: String,
    text: String,
    #[arg(long, short = 'w')]
    handle: Option<i64>,
  },
  TypeRaw {
    text: String,
    #[arg(long, short = 'w')]
    handle: Option<i64>,
  },
  SendKeys {
    keys: String,
  },
  KeyDown {
    key: String,
  },
  KeyUp {
    key: String,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: KeyboardAction) -> Result<()> {
  match action {
    KeyboardAction::Type { handle, text, x, y } => {
      let mut params = json!({"text": text});
      if let Some(h) = handle {
        params["handle"] = json!(h);
      }
      if let (Some(px), Some(py)) = (x, y) {
        params["position"] = json!({"x": px, "y": py});
      }
      let r = executor.call("keyboard.type_text", params)?;
      print_input_result(&r);
    }
    KeyboardAction::TypeXpath { handle, xpath, text } => {
      let mut params = json!({"xpath": xpath, "text": text});
      if let Some(h) = handle {
        params["handle"] = json!(h);
      }
      let r = executor.call("keyboard.type_text_by_xpath", params)?;
      print_input_result(&r);
    }
    KeyboardAction::TypeRaw { handle, text } => {
      let mut params = json!({"text": text});
      if let Some(h) = handle {
        params["handle"] = json!(h);
      }
      executor.call("keyboard.type_raw", params)?;
      println!("ok");
    }
    KeyboardAction::SendKeys { keys } => {
      let (key_codes, modifiers) = parse_key_combo(&keys)?;
      let mut params = json!({"keys": key_codes});
      if !modifiers.is_empty() {
        params["modifiers"] = json!(modifiers);
      }
      executor.call("keyboard.send_keys", params)?;
      println!("ok");
    }
    KeyboardAction::KeyDown { key } => {
      let key_val: serde_json::Value = serde_json::from_str(&format!("\"{key}\"")).context("invalid key name")?;
      executor.call("keyboard.key_down", json!({"key": key_val}))?;
      println!("ok");
    }
    KeyboardAction::KeyUp { key } => {
      let key_val: serde_json::Value = serde_json::from_str(&format!("\"{key}\"")).context("invalid key name")?;
      executor.call("keyboard.key_release", json!({"key": key_val}))?;
      println!("ok");
    }
  }
  Ok(())
}

fn parse_key_combo(input: &str) -> Result<(Vec<String>, Vec<String>)> {
  let parts: Vec<&str> = input.split('+').map(|s| s.trim()).collect();
  let mut modifiers = Vec::new();
  let mut main_keys = Vec::new();
  for part in &parts {
    let normalized = normalize_key_name(part);
    if is_modifier(&normalized) {
      modifiers.push(normalized);
    } else {
      main_keys.push(normalized);
    }
  }
  if main_keys.is_empty() {
    if let Some(last) = modifiers.pop() {
      main_keys.push(last);
    }
  }
  Ok((main_keys, modifiers))
}

fn normalize_key_name(name: &str) -> String {
  match name.to_lowercase().as_str() {
    "ctrl" | "control" => "Ctrl".to_string(),
    "shift" => "Shift".to_string(),
    "alt" => "Alt".to_string(),
    "win" | "meta" | "super" => "Win".to_string(),
    "enter" | "return" => "Enter".to_string(),
    "esc" | "escape" => "Escape".to_string(),
    "tab" => "Tab".to_string(),
    "space" | "spacebar" => "Space".to_string(),
    "backspace" | "back" => "Backspace".to_string(),
    "del" | "delete" => "Delete".to_string(),
    "up" | "arrowup" => "Up".to_string(),
    "down" | "arrowdown" => "Down".to_string(),
    "left" | "arrowleft" => "Left".to_string(),
    "right" | "arrowright" => "Right".to_string(),
    "home" => "Home".to_string(),
    "end" => "End".to_string(),
    "pageup" | "pgup" => "PageUp".to_string(),
    "pagedown" | "pgdn" => "PageDown".to_string(),
    "insert" | "ins" => "Insert".to_string(),
    "capslock" | "caps" => "CapsLock".to_string(),
    "numlock" => "NumLock".to_string(),
    "scrolllock" => "ScrollLock".to_string(),
    "printscreen" | "prtsc" => "PrintScreen".to_string(),
    "pause" | "break" => "Pause".to_string(),
    "apps" | "menu" => "Apps".to_string(),
    s if s.starts_with('f') && s[1..].parse::<u32>().is_ok() => format!("F{}", &s[1..]),
    s if s.len() == 1 => s.to_uppercase(),
    other => {
      let mut chars = other.chars();
      match chars.next() {
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
        None => other.to_string(),
      }
    }
  }
}

fn is_modifier(name: &str) -> bool {
  matches!(
    name,
    "Ctrl" | "LCtrl" | "RCtrl" | "Shift" | "LShift" | "RShift" | "Alt" | "LAlt" | "RAlt" | "Win" | "LWin" | "RWin"
  )
}
