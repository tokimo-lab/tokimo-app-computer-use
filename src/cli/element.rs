use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::{CommandExecutor, print_input_result, scope::WindowSel};

#[derive(Subcommand, Debug)]
pub enum ElementAction {
  /// List UI elements matching a query
  List {
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long)]
    role: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    text_exact: bool,
    #[arg(long)]
    index: Option<usize>,
    #[arg(long)]
    max_depth: Option<usize>,
  },
  /// Dump the full UI element tree
  Tree {
    #[command(flatten)]
    sel: WindowSel,
  },
  /// Inspect element at screen coordinates
  Probe {
    #[arg(long)]
    x: i32,
    #[arg(long)]
    y: i32,
  },
  /// Click the first matching UI element
  Click {
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long)]
    role: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    text_exact: bool,
    #[arg(long, default_value = "left")]
    button: String,
    #[arg(long)]
    double: bool,
  },
  /// Type text into the first matching UI element
  Type {
    value: String,
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long)]
    role: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    text_exact: bool,
    #[arg(long)]
    enter: bool,
    #[arg(long)]
    clear: bool,
  },
  /// Activate (confirm/press) the first matching UI element
  Activate {
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long)]
    role: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    text_exact: bool,
  },
  /// Find elements by XPath expression
  Xpath {
    expr: String,
    #[command(flatten)]
    sel: WindowSel,
  },
  /// Click the first element matching an XPath expression
  XpathClick {
    expr: String,
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long, default_value = "left")]
    button: String,
    #[arg(long)]
    double: bool,
  },
  /// Type text into the first element matching an XPath expression
  XpathType {
    expr: String,
    value: String,
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long)]
    enter: bool,
    #[arg(long)]
    clear: bool,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: ElementAction) -> Result<()> {
  match action {
    ElementAction::List { sel, role, text, text_exact, index, max_depth } => {
      let mut params = sel.to_json_scope();
      if let Some(r) = role {
        params["role"] = json!(r);
      }
      if let Some(t) = text {
        params["text"] = json!(t);
      }
      params["text_exact"] = json!(text_exact);
      if let Some(i) = index {
        params["index"] = json!(i);
      }
      if let Some(d) = max_depth {
        params["max_depth"] = json!(d);
      }
      let r = executor.call("element.query", params)?;
      print_elements(&r);
    }
    ElementAction::Tree { sel } => {
      let params = sel.to_json_scope();
      let r = executor.call("element.tree", params)?;
      let raw = r.as_str().unwrap_or("").to_string();
      println!("{}", filter_page_source(&raw));
    }
    ElementAction::Probe { x, y } => {
      let r = executor.call("element.probe", json!({"x": x, "y": y}))?;
      println!("{}", r.as_str().unwrap_or(""));
    }
    ElementAction::Click { sel, role, text, text_exact, button, double } => {
      let mut params = sel.to_json_scope();
      if let Some(r) = role {
        params["role"] = json!(r);
      }
      if let Some(t) = text {
        params["text"] = json!(t);
      }
      params["text_exact"] = json!(text_exact);
      params["button"] = json!(button);
      params["double"] = json!(double);
      let r = executor.call("element.click", params)?;
      print_input_result(&r);
    }
    ElementAction::Type { value, sel, role, text, text_exact, enter, clear } => {
      let mut params = sel.to_json_scope();
      params["value"] = json!(value);
      if let Some(r) = role {
        params["role"] = json!(r);
      }
      if let Some(t) = text {
        params["text"] = json!(t);
      }
      params["text_exact"] = json!(text_exact);
      params["enter"] = json!(enter);
      params["clear"] = json!(clear);
      let r = executor.call("element.type", params)?;
      print_input_result(&r);
    }
    ElementAction::Activate { sel, role, text, text_exact } => {
      let mut params = sel.to_json_scope();
      if let Some(r) = role {
        params["role"] = json!(r);
      }
      if let Some(t) = text {
        params["text"] = json!(t);
      }
      params["text_exact"] = json!(text_exact);
      executor.call("element.activate", params)?;
      println!("ok");
    }
    ElementAction::Xpath { expr, sel } => {
      let mut params = sel.to_json_scope();
      params["xpath"] = json!(expr);
      let r = executor.call("element.xpath_query", params)?;
      print_elements(&r);
    }
    ElementAction::XpathClick { expr, sel, button, double } => {
      let mut params = sel.to_json_scope();
      params["xpath"] = json!(expr);
      params["button"] = json!(button);
      params["double"] = json!(double);
      let r = executor.call("element.xpath_click", params)?;
      print_input_result(&r);
    }
    ElementAction::XpathType { expr, value, sel, enter, clear } => {
      let mut params = sel.to_json_scope();
      params["xpath"] = json!(expr);
      params["value"] = json!(value);
      params["enter"] = json!(enter);
      params["clear"] = json!(clear);
      let r = executor.call("element.xpath_type", params)?;
      print_input_result(&r);
    }
  }
  Ok(())
}

fn print_elements(value: &serde_json::Value) {
  if let Some(arr) = value.as_array() {
    if arr.is_empty() {
      return;
    }
    let mut t = super::Table::new(vec![
      ("TYPE", 16),
      ("NAME", 28),
      ("AID", 20),
      ("CLASS", 24),
      ("RECT", 20),
    ]);
    for e in arr {
      let ctrl = e["control_type"].as_str().unwrap_or("?");
      let name = e["name"].as_str().unwrap_or("");
      let text = e["text"].as_str().unwrap_or("");
      let aid = e["automation_id"].as_str().unwrap_or("");
      let cls = e["class_name"].as_str().unwrap_or("");
      if ctrl == "?" && name.is_empty() && aid.is_empty() && cls.is_empty() {
        continue;
      }
      let display_name = if name.is_empty() && !text.is_empty() { text } else { name };
      let x = e["x"].as_f64().unwrap_or(0.0) as i64;
      let y = e["y"].as_f64().unwrap_or(0.0) as i64;
      let w = e["width"].as_f64().unwrap_or(0.0) as i64;
      let h = e["height"].as_f64().unwrap_or(0.0) as i64;
      t.row(vec![
        ctrl.to_string(),
        display_name.to_string(),
        aid.to_string(),
        cls.to_string(),
        format!("{x},{y},{w},{h}"),
      ]);
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
      if trimmed.ends_with("/>") {
        i += 1;
        continue;
      }
      if i + 1 < lines.len() {
        let next = lines[i + 1].trim_start();
        if next.starts_with("</") && next.ends_with('>') {
          i += 2;
          continue;
        }
      }
    }
    out.push_str(lines[i]);
    out.push('\n');
    i += 1;
  }
  out
}

fn is_empty_structural_tag(s: &str) -> bool {
  if !s.starts_with("<Pane ") && !s.starts_with("<Window ") {
    return false;
  }
  !s.contains("Name=\"") && !s.contains("AutomationId=\"") && !s.contains("ClassName=\"")
}
