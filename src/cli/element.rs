use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::{CommandExecutor, print_input_result, scope::WindowSel};

/// Element-related subcommands.
///
/// Design notes (for AI / agent consumers):
/// * Every command that talks to a UI scope accepts the common `-w / -a` selector
///   triple via `WindowSel`. With no selector, the foreground window is used.
/// * `find` is the flat, table-style query — use it to discover what's on screen.
/// * `tree` is the hierarchical view — use it when you need parent/child context.
///   Both honor the SAME filter flags (`--role / --text / --exact`) so the agent
///   only has to learn one set of vocab.
/// * Filters: when no `--role/--text` is given, every visible element is returned.
///   By default, off-screen / zero-size elements are hidden — pass `--include-hidden`
///   to see them.
/// * Hit-test discovery (a slow scan that finds detached AX subtrees, e.g. QQ Music
///   search results) is ON by default. Pass `--no-hit-test` to skip it when speed
///   matters and you trust the target app exposes a normal AX tree.
/// * Click / type / press all accept the same filter vocab; they internally use
///   the FIRST match by default, or `--nth N` to pick the Nth.
#[derive(Subcommand, Debug)]
pub enum ElementAction {
  /// Find UI elements matching a query. Outputs a flat table.
  Find {
    #[command(flatten)]
    sel: WindowSel,
    /// Abstract role: Button / Edit / Text / List / ListItem / MenuItem /
    /// CheckBox / RadioButton / ComboBox / Image / Link / Tab / Group / Window
    #[arg(long)]
    role: Option<String>,
    /// Text substring (case-insensitive) to match against name / value /
    /// description / help / identifier / placeholder
    #[arg(long)]
    text: Option<String>,
    /// Require exact (case-sensitive) match for --text instead of substring
    #[arg(long)]
    exact: bool,
    /// Maximum number of rows to print (0 = unlimited, default = unlimited)
    #[arg(long, default_value_t = 0)]
    max: usize,
    /// Maximum tree depth to search (default: unlimited)
    #[arg(long)]
    max_depth: Option<usize>,
    /// Include off-screen / zero-size elements (default: hide them)
    #[arg(long)]
    include_hidden: bool,
    /// Skip the hit-test grid scan that discovers detached AX subtrees (faster)
    #[arg(long)]
    no_hit_test: bool,
  },
  /// Render a hierarchical tree view (accepts the same filters as `find`).
  Tree {
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long)]
    role: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    exact: bool,
    /// Maximum tree depth (default: unlimited — pipe through `head` if too verbose).
    #[arg(long, default_value_t = usize::MAX)]
    depth: usize,
    #[arg(long)]
    include_hidden: bool,
    #[arg(long)]
    no_hit_test: bool,
  },
  /// Dump every AX attribute of the element at screen coordinates (x, y).
  Probe {
    #[arg(long)]
    x: i32,
    #[arg(long)]
    y: i32,
  },
  /// Click a matched element (real mouse events).
  Click {
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long)]
    role: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    exact: bool,
    /// 0-based index when multiple elements match
    #[arg(long, default_value_t = 0)]
    nth: usize,
    #[arg(long, default_value = "left")]
    button: String,
    #[arg(long)]
    double: bool,
    #[arg(long)]
    no_hit_test: bool,
  },
  /// Type text into a matched element (focuses + sends keystrokes).
  Type {
    /// Text to type
    value: String,
    #[command(flatten)]
    sel: WindowSel,
    /// Default role for typing is "Edit"; override here if needed
    #[arg(long, default_value = "Edit")]
    role: String,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    exact: bool,
    #[arg(long, default_value_t = 0)]
    nth: usize,
    /// Press Enter after typing
    #[arg(long)]
    enter: bool,
    /// Cmd+A / Delete the field before typing
    #[arg(long)]
    clear: bool,
    #[arg(long)]
    no_hit_test: bool,
  },
  /// AXPress a matched element (no mouse — best for menu items, links).
  Press {
    #[command(flatten)]
    sel: WindowSel,
    #[arg(long)]
    role: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    exact: bool,
    #[arg(long, default_value_t = 0)]
    nth: usize,
    #[arg(long)]
    no_hit_test: bool,
  },
}

#[allow(clippy::too_many_arguments)]
fn add_query_params(
  params: &mut serde_json::Value,
  role: Option<String>,
  text: Option<String>,
  exact: bool,
  max_depth: Option<usize>,
  include_hidden: bool,
  no_hit_test: bool,
) {
  if let Some(r) = role {
    params["role"] = json!(r);
  }
  if let Some(t) = text {
    params["text"] = json!(t);
  }
  params["text_exact"] = json!(exact);
  if let Some(d) = max_depth {
    params["max_depth"] = json!(d);
  }
  params["include_hidden"] = json!(include_hidden);
  params["no_hit_test"] = json!(no_hit_test);
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: ElementAction) -> Result<()> {
  match action {
    ElementAction::Find {
      sel,
      role,
      text,
      exact,
      max,
      max_depth,
      include_hidden,
      no_hit_test,
    } => {
      let mut params = sel.to_json_scope();
      add_query_params(&mut params, role, text, exact, max_depth, include_hidden, no_hit_test);
      let r = executor.call("element.query", params)?;
      print_elements_limited(&r, max);
    }
    ElementAction::Tree {
      sel,
      role,
      text,
      exact,
      depth,
      include_hidden,
      no_hit_test,
    } => {
      let mut params = sel.to_json_scope();
      add_query_params(&mut params, role, text, exact, Some(depth), include_hidden, no_hit_test);
      let r = executor.call("element.tree", params)?;
      println!("{}", r.as_str().unwrap_or(""));
    }
    ElementAction::Probe { x, y } => {
      let r = executor.call("element.probe", json!({"x": x, "y": y}))?;
      println!("{}", r.as_str().unwrap_or(""));
    }
    ElementAction::Click {
      sel,
      role,
      text,
      exact,
      nth,
      button,
      double,
      no_hit_test,
    } => {
      let mut params = sel.to_json_scope();
      add_query_params(&mut params, role, text, exact, None, false, no_hit_test);
      params["index"] = json!(nth);
      params["button"] = json!(button);
      params["double"] = json!(double);
      let r = executor.call("element.click", params)?;
      print_input_result(&r);
    }
    ElementAction::Type {
      value,
      sel,
      role,
      text,
      exact,
      nth,
      enter,
      clear,
      no_hit_test,
    } => {
      let mut params = sel.to_json_scope();
      params["value"] = json!(value);
      add_query_params(&mut params, Some(role), text, exact, None, false, no_hit_test);
      params["index"] = json!(nth);
      params["enter"] = json!(enter);
      params["clear"] = json!(clear);
      let r = executor.call("element.type", params)?;
      print_input_result(&r);
    }
    ElementAction::Press {
      sel,
      role,
      text,
      exact,
      nth,
      no_hit_test,
    } => {
      let mut params = sel.to_json_scope();
      add_query_params(&mut params, role, text, exact, None, false, no_hit_test);
      params["index"] = json!(nth);
      executor.call("element.activate", params)?;
      println!("ok");
    }
  }
  Ok(())
}

fn print_elements_limited(value: &serde_json::Value, max: usize) {
  if let Some(arr) = value.as_array() {
    if arr.is_empty() {
      return;
    }
    let mut t = super::Table::new(vec![
      ("TYPE", 14),
      ("NAME", 22),
      ("DESC", 28),
      ("AID", 18),
      ("CLASS", 18),
      ("RECT", 20),
    ]);
    let mut printed = 0usize;
    for e in arr {
      let ctrl = e["control_type"].as_str().unwrap_or("?");
      let name = e["name"].as_str().unwrap_or("");
      let text = e["text"].as_str().unwrap_or("");
      let help = e["help_text"].as_str().unwrap_or("");
      let aid = e["automation_id"].as_str().unwrap_or("");
      let cls = e["class_name"].as_str().unwrap_or("");
      if ctrl == "?" && name.is_empty() && aid.is_empty() && cls.is_empty() && help.is_empty() {
        continue;
      }
      let x = e["x"].as_f64().unwrap_or(0.0) as i64;
      let y = e["y"].as_f64().unwrap_or(0.0) as i64;
      let w = e["width"].as_f64().unwrap_or(0.0) as i64;
      let h = e["height"].as_f64().unwrap_or(0.0) as i64;
      let display_name = if name.is_empty() && !text.is_empty() {
        text
      } else {
        name
      };
      t.row(vec![
        ctrl.to_string(),
        display_name.to_string(),
        help.to_string(),
        aid.to_string(),
        cls.to_string(),
        format!("{x},{y},{w},{h}"),
      ]);
      printed += 1;
      if max > 0 && printed >= max {
        break;
      }
    }
    t.print();
    if max > 0 && arr.len() > max {
      println!(
        "... ({} more elements not shown; raise --max to see them)",
        arr.len() - max
      );
    }
  }
}
