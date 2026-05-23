use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::{CommandExecutor, print_input_result};

#[derive(Subcommand, Debug)]
pub enum ElementAction {
  Find { xpath: String, #[arg(long, short = 'w')] handle: Option<i64> },
  PageSource { #[arg(long, short = 'w')] handle: Option<i64> },
  Click { xpath: String, #[arg(long, short = 'w')] handle: Option<i64>, #[arg(long, default_value = "left")] button: String, #[arg(long)] double_click: bool },
  Type { xpath: String, text: String, #[arg(long, short = 'w')] handle: Option<i64>, #[arg(long, short = 'e')] enter: bool },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: ElementAction) -> Result<()> {
  match action {
    ElementAction::Find { handle, xpath } => {
      let mut params = json!({"xpath": xpath});
      if let Some(h) = handle { params["handle"] = json!(h); }
      let r = executor.call("element.find", params)?;
      print_elements(&r);
    }
    ElementAction::PageSource { handle } => {
      let mut params = json!({});
      if let Some(h) = handle { params["handle"] = json!(h); }
      let r = executor.call("element.page_source", params)?;
      let raw = r.as_str().unwrap_or("").to_string();
      println!("{}", filter_page_source(&raw));
    }
    ElementAction::Click { xpath, handle, button, double_click } => {
      let mut params = json!({"xpath": xpath, "button": button, "double_click": double_click});
      if let Some(h) = handle { params["handle"] = json!(h); }
      let r = executor.call("mouse.click_by_xpath", params)?;
      print_input_result(&r);
    }
    ElementAction::Type { xpath, text, handle, enter } => {
      let mut params = json!({"xpath": xpath, "text": text});
      if let Some(h) = handle { params["handle"] = json!(h); }
      let r = executor.call("keyboard.type_text_by_xpath", params)?;
      if enter { executor.call("keyboard.send_keys", json!({"keys": ["Enter"]}))?; }
      print_input_result(&r);
    }
  }
  Ok(())
}

fn print_elements(value: &serde_json::Value) {
  if let Some(arr) = value.as_array() {
    if arr.is_empty() { return; }
    let mut t = super::Table::new(vec![("TYPE", 16), ("NAME", 28), ("AID", 20), ("CLASS", 24), ("RECT", 20)]);
    for e in arr {
      let ctrl = e["control_type"].as_str().unwrap_or("?");
      let name = e["name"].as_str().unwrap_or("");
      let aid = e["automation_id"].as_str().unwrap_or("");
      let cls = e["class_name"].as_str().unwrap_or("");
      if name.is_empty() && aid.is_empty() && cls.is_empty() { continue; }
      let x = e["x"].as_f64().unwrap_or(0.0) as i64;
      let y = e["y"].as_f64().unwrap_or(0.0) as i64;
      let w = e["width"].as_f64().unwrap_or(0.0) as i64;
      let h = e["height"].as_f64().unwrap_or(0.0) as i64;
      t.row(vec![ctrl.to_string(), name.to_string(), aid.to_string(), cls.to_string(), format!("{x},{y},{w},{h}")]);
    }
    t.print();
  }
}

fn filter_page_source(xml: &str) -> String {
  let lines: Vec<&str> = xml.lines().collect();
  let mut out = String::with_capacity(xml.len());
  let mut i = 0;
  while i < lines.len() {
    let trimmed = lines[i].trim_start();
    if is_empty_structural_tag(trimmed) {
      if trimmed.ends_with("/>") { i += 1; continue; }
      if i + 1 < lines.len() {
        let next = lines[i + 1].trim_start();
        if next.starts_with("</") && next.ends_with('>') { i += 2; continue; }
      }
    }
    out.push_str(lines[i]);
    out.push('\n');
    i += 1;
  }
  out
}

fn is_empty_structural_tag(s: &str) -> bool {
  if !s.starts_with("<Pane ") && !s.starts_with("<Window ") { return false; }
  !s.contains("Name=\"") && !s.contains("AutomationId=\"") && !s.contains("ClassName=\"")
}
