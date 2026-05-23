use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum WindowAction {
  List {
    #[arg(long)]
    all: bool,
  },
  Find {
    title: String,
  },
  Search {
    pattern: String,
    #[arg(long)]
    process_name: Option<String>,
  },
  Title {
    handle: i64,
  },
  Focus {
    handle: i64,
  },
  Move {
    handle: i64,
    x: i32,
    y: i32,
  },
  Resize {
    handle: i64,
    width: i32,
    height: i32,
  },
  SetRect {
    handle: i64,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
  },
  Minimize {
    handle: i64,
  },
  Maximize {
    handle: i64,
  },
  Restore {
    handle: i64,
  },
  Children {
    handle: i64,
  },
  ByPid {
    pid: u32,
  },
  Info {
    handle: i64,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: WindowAction) -> Result<()> {
  match action {
    WindowAction::List { all } => {
      let method = if all { "window.list" } else { "window.list_visible" };
      let mut r = executor.call(method, json!({}))?;
      if all {
        if let Some(arr) = r.as_array_mut() {
          arr.retain(|w| {
            let title = w["title"].as_str().unwrap_or("");
            let proc = w["processName"].as_str().unwrap_or("");
            let width = w["width"].as_i64().unwrap_or(0);
            let height = w["height"].as_i64().unwrap_or(0);
            !title.is_empty() && !is_system_window_title(title, proc) && width >= 100 && height >= 100
          });
        }
      }
      print_windows_tree(&r, "");
    }
    WindowAction::Find { title } => {
      let r = executor.call("window.find_windows_by_title", json!({"pattern": title}))?;
      print_windows_tree(&r, &title);
    }
    WindowAction::Search { pattern, process_name } => {
      let mut params = json!({"pattern": pattern});
      if let Some(pn) = process_name {
        params["process_name"] = json!(pn);
      }
      let r = executor.call("window.find_windows_by_title", params)?;
      print_windows_tree(&r, &pattern);
    }
    WindowAction::Title { handle } => {
      let r = executor.call("window.title", json!({"handle": handle}))?;
      println!("{}", r.as_str().unwrap_or(&r.to_string()));
    }
    WindowAction::Focus { handle } => {
      executor.call("window.focus", json!({"handle": handle}))?;
      println!("ok");
    }
    WindowAction::Move { handle, x, y } => {
      executor.call("window.move", json!({"handle": handle, "x": x, "y": y}))?;
      println!("ok");
    }
    WindowAction::Resize { handle, width, height } => {
      executor.call(
        "window.resize",
        json!({"handle": handle, "width": width, "height": height}),
      )?;
      println!("ok");
    }
    WindowAction::SetRect {
      handle,
      x,
      y,
      width,
      height,
    } => {
      executor.call(
        "window.set_rect",
        json!({"handle": handle, "x": x, "y": y, "width": width, "height": height}),
      )?;
      println!("ok");
    }
    WindowAction::Minimize { handle } => {
      executor.call("window.minimize", json!({"handle": handle}))?;
      println!("ok");
    }
    WindowAction::Maximize { handle } => {
      executor.call("window.maximize", json!({"handle": handle}))?;
      println!("ok");
    }
    WindowAction::Restore { handle } => {
      executor.call("window.restore", json!({"handle": handle}))?;
      println!("ok");
    }
    WindowAction::Children { handle } => {
      let r = executor.call("window.children", json!({"handle": handle}))?;
      print_windows(&r);
    }
    WindowAction::ByPid { pid } => {
      let r = executor.call("window.by_process_id", json!({"pid": pid}))?;
      print_windows(&r);
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
