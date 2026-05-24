use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::{CommandExecutor, scope::WindowSel};

#[derive(Subcommand, Debug)]
pub enum WindowAction {
  /// List windows (--all includes hidden/minimized)
  List {
    #[arg(long)]
    all: bool,
    #[arg(long)]
    app: Option<String>,
  },
  /// Find windows by title, process name, or PID
  Find {
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    process: Option<String>,
    #[arg(long)]
    pid: Option<u32>,
  },
  /// Show details of a specific window
  Info { handle: i64 },
  /// Bring a window to the foreground
  Focus {
    #[command(flatten)]
    sel: WindowSel,
  },
  /// Move a window to a screen position
  Move { handle: i64, x: i32, y: i32 },
  /// Resize a window
  Resize { handle: i64, w: i32, h: i32 },
  /// Move and resize a window in one call
  Rect {
    handle: i64,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
  },
  /// Change window state (minimize / maximize / restore)
  State { handle: i64, op: String },
  /// Show the currently focused window
  Foreground,
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: WindowAction) -> Result<()> {
  match action {
    WindowAction::List { all, app } => {
      let mut params = json!({"all": all});
      if let Some(name) = app {
        params["app"] = json!(name);
      }
      let mut r = executor.call("window.list", params)?;
      if all {
        if let Some(arr) = r.as_array_mut() {
          arr.retain(|w| {
            let title = w["title"].as_str().unwrap_or("");
            let proc = w["processName"].as_str().unwrap_or("");
            let width = w["width"].as_i64().unwrap_or(0);
            let height = w["height"].as_i64().unwrap_or(0);
            // Drop sub-100px noise and known system IME/overlay windows.
            // Do NOT require non-empty title: many real macOS app windows
            // (Qt, Electron, some Cocoa apps including QQ音乐) legitimately
            // have empty titles. process_name is the meaningful identifier.
            !is_system_window_title(title, proc) && width >= 100 && height >= 100
          });
        }
      }
      print_windows_tree(&r, "");
    }
    WindowAction::Find { title, process, pid } => {
      let mut params = json!({});
      if let Some(t) = title {
        params["title"] = json!(t);
      }
      if let Some(p) = process {
        params["process"] = json!(p);
      }
      if let Some(id) = pid {
        params["pid"] = json!(id);
      }
      let r = executor.call("window.find", params)?;
      print_windows_tree(&r, "");
    }
    WindowAction::Info { handle } => {
      let r = executor.call("window.info", json!({"handle": handle}))?;
      if let Some(obj) = r.as_object() {
        for (k, v) in obj {
          let val = match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            other => other.to_string(),
          };
          println!("{k}={val}");
        }
      }
    }
    WindowAction::Focus { sel } => {
      let mut params = sel.to_json_scope();
      executor.call("window.focus", params.take())?;
      println!("ok");
    }
    WindowAction::Move { handle, x, y } => {
      executor.call("window.move", json!({"handle": handle, "x": x, "y": y}))?;
      println!("ok");
    }
    WindowAction::Resize { handle, w, h } => {
      executor.call("window.resize", json!({"handle": handle, "w": w, "h": h}))?;
      println!("ok");
    }
    WindowAction::Rect { handle, x, y, w, h } => {
      executor.call("window.rect", json!({"handle": handle, "x": x, "y": y, "w": w, "h": h}))?;
      println!("ok");
    }
    WindowAction::State { handle, op } => {
      executor.call("window.state", json!({"handle": handle, "op": op}))?;
      println!("ok");
    }
    WindowAction::Foreground => {
      let r = executor.call("window.foreground", json!({}))?;
      if let Some(obj) = r.as_object() {
        let hwnd = obj["hwnd"].as_i64().unwrap_or(0);
        let pid = obj["processId"].as_u64().unwrap_or(0);
        let proc = obj["processName"].as_str().unwrap_or("?");
        let title = obj["title"].as_str().unwrap_or("");
        println!("{hwnd}  {pid}  {proc}  {title}");
      } else {
        println!("{r}");
      }
    }
  }
  Ok(())
}

fn is_system_window_title(title: &str, process_name: &str) -> bool {
  let t = title.to_lowercase();
  let p = process_name.to_lowercase();
  if t == "default ime" || t == "msctfime ui" || t == "sogou_tsf_ui" {
    return true;
  }
  if t.contains("gdi+ window") {
    return true;
  }
  if p == "textinputhost.exe" || t.contains("windows input experience") {
    return true;
  }
  if t.contains("nvidia geforce overlay") || t.contains("nvidia overlay") {
    return true;
  }
  if t == "program manager" {
    return true;
  }
  false
}

fn print_windows(value: &serde_json::Value) {
  if let Some(arr) = value.as_array() {
    if arr.is_empty() {
      return;
    }
    let mut t = super::Table::new(vec![
      ("HWND", 12),
      ("PID", 8),
      ("PROCESS", 24),
      ("SIZE", 12),
      ("STATE", 12),
      ("TITLE", 40),
    ]);
    for w in arr {
      let hwnd = w["hwnd"].as_i64().unwrap_or(0);
      let pid = w["processId"].as_u64().unwrap_or(0);
      let proc = w["processName"].as_str().unwrap_or("?");
      let title = w["title"].as_str().unwrap_or("");
      let width = w["width"].as_i64().unwrap_or(0);
      let height = w["height"].as_i64().unwrap_or(0);
      let is_visible = w["isVisible"].as_bool().unwrap_or(true);
      let is_min = w["isMinimized"].as_bool().unwrap_or(false);
      let is_max = w["isMaximized"].as_bool().unwrap_or(false);
      let is_top = w["isTopmost"].as_bool().unwrap_or(false);
      let mut state = String::new();
      if !is_visible {
        state.push_str("hidden ");
      }
      if is_min {
        state.push_str("min ");
      }
      if is_max {
        state.push_str("max ");
      }
      if is_top {
        state.push_str("top ");
      }
      let state = state.trim_end();
      t.row(vec![
        hwnd.to_string(),
        pid.to_string(),
        proc.to_string(),
        format!("{width}x{height}"),
        state.to_string(),
        title.to_string(),
      ]);
    }
    t.print();
  }
}

fn print_windows_tree(value: &serde_json::Value, pattern: &str) {
  let Some(arr) = value.as_array() else { return };
  if arr.is_empty() {
    return;
  }

  let entries: Vec<(i64, i64, String)> = arr
    .iter()
    .map(|w| {
      let hwnd = w["hwnd"].as_i64().unwrap_or(0);
      let parent = w["parentHwnd"].as_i64().unwrap_or(0);
      let pid = w["processId"].as_u64().unwrap_or(0);
      let proc = w["processName"].as_str().unwrap_or("?");
      let title = w["title"].as_str().unwrap_or("");
      let width = w["width"].as_i64().unwrap_or(0);
      let height = w["height"].as_i64().unwrap_or(0);
      let is_visible = w["isVisible"].as_bool().unwrap_or(true);
      let is_min = w["isMinimized"].as_bool().unwrap_or(false);
      let is_max = w["isMaximized"].as_bool().unwrap_or(false);
      let mut state = Vec::new();
      if !is_visible {
        state.push("hidden");
      }
      if is_min {
        state.push("min");
      }
      if is_max {
        state.push("max");
      }
      let state_str = if state.is_empty() {
        String::new()
      } else {
        format!(" [{}]", state.join(" "))
      };
      let line = format!("{hwnd}  {pid:<6} {proc:<20} {width}x{height}{state_str}  {title}");
      (hwnd, parent, line)
    })
    .collect();

  let hwnd_set: std::collections::HashSet<i64> = entries.iter().map(|e| e.0).collect();
  let mut children: std::collections::HashMap<i64, Vec<usize>> = std::collections::HashMap::new();
  let mut roots: Vec<usize> = Vec::new();

  for (i, &(_, parent, _)) in entries.iter().enumerate() {
    if parent == 0 || !hwnd_set.contains(&parent) {
      roots.push(i);
    } else {
      children.entry(parent).or_default().push(i);
    }
  }

  let pat_lower = pattern.to_lowercase();
  let match_rank = |proc: &str| -> u8 {
    let p = proc.to_lowercase();
    if p == pat_lower || p == format!("{pat_lower}.exe") {
      3
    } else if p.starts_with(&pat_lower) {
      2
    } else {
      1
    }
  };
  roots.sort_by(|&a, &b| {
    let pa = entries[a].2.split_whitespace().nth(2).unwrap_or("");
    let pb = entries[b].2.split_whitespace().nth(2).unwrap_or("");
    match_rank(pb).cmp(&match_rank(pa))
  });

  fn render(
    entries: &[(i64, i64, String)],
    children: &std::collections::HashMap<i64, Vec<usize>>,
    idx: usize,
    prefix: &str,
    is_last: bool,
  ) {
    let connector = if is_last { "└── " } else { "├── " };
    println!("{prefix}{connector}{}", entries[idx].2);
    let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
    if let Some(child_idxs) = children.get(&entries[idx].0) {
      let len = child_idxs.len();
      for (i, &ci) in child_idxs.iter().enumerate() {
        render(entries, children, ci, &child_prefix, i == len - 1);
      }
    }
  }

  println!("HWND       PID    PROCESS              SIZE       TITLE");
  let len = roots.len();
  for (i, &ri) in roots.iter().enumerate() {
    render(&entries, &children, ri, "", i == len - 1);
  }
}
