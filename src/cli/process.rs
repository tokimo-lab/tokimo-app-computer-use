use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum ProcessAction {
  /// List all running processes
  List {
    #[arg(long)]
    filter: Option<String>,
  },
  /// Find processes by name or PID
  Find {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    pid: Option<u32>,
  },
  /// Launch an application
  Launch {
    #[arg(long)]
    path: Option<String>,
    #[arg(long)]
    app: Option<String>,
    #[arg(long)]
    wait: bool,
  },
  /// Terminate a process by PID or name
  Terminate {
    #[arg(long)]
    pid: Option<u32>,
    #[arg(long)]
    name: Option<String>,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: ProcessAction) -> Result<()> {
  match action {
    ProcessAction::List { filter } => {
      let mut r = executor.call("process.list", json!({}))?;
      if let Some(arr) = r.as_array_mut() {
        if let Some(f) = &filter {
          let pat = f.to_lowercase();
          arr.retain(|p| p["name"].as_str().unwrap_or("").to_lowercase().contains(&pat));
        }
        let mut t = super::Table::new(vec![
          ("PID", 8),
          ("NAME", 32),
          ("THREADS", 8),
          ("PARENT", 8),
          ("MEMORY", 12),
        ]);
        for p in arr {
          let pid_v = p["pid"].as_u64().unwrap_or(0);
          let nm = p["name"].as_str().unwrap_or("?");
          let threads = p["thread_count"].as_u64().unwrap_or(0);
          let parent = p["parent_pid"].as_u64().unwrap_or(0);
          let mem = p["memory_bytes"].as_u64().unwrap_or(0);
          t.row(vec![
            pid_v.to_string(),
            nm.to_string(),
            threads.to_string(),
            parent.to_string(),
            format!("{}KB", mem / 1024),
          ]);
        }
        t.print();
      }
    }
    ProcessAction::Find { name, pid } => {
      let mut params = json!({});
      if let Some(n) = name {
        params["name"] = json!(n);
      }
      if let Some(p) = pid {
        params["pid"] = json!(p);
      }
      let r = executor.call("process.find", params)?;
      if let Some(arr) = r.as_array() {
        let mut t = super::Table::new(vec![
          ("PID", 8),
          ("NAME", 32),
          ("THREADS", 8),
          ("PARENT", 8),
          ("MEMORY", 12),
        ]);
        for p in arr {
          let pid_v = p["pid"].as_u64().unwrap_or(0);
          let nm = p["name"].as_str().unwrap_or("?");
          let threads = p["thread_count"].as_u64().unwrap_or(0);
          let parent = p["parent_pid"].as_u64().unwrap_or(0);
          let mem = p["memory_bytes"].as_u64().unwrap_or(0);
          t.row(vec![
            pid_v.to_string(),
            nm.to_string(),
            threads.to_string(),
            parent.to_string(),
            format!("{}KB", mem / 1024),
          ]);
        }
        t.print();
      }
    }
    ProcessAction::Launch { path, app, wait } => {
      let mut params = json!({"wait": wait});
      if let Some(p) = path {
        params["path"] = json!(p);
      }
      if let Some(a) = app {
        params["app"] = json!(a);
      }
      let r = executor.call("process.launch", params)?;
      if let Some(pid_v) = r["pid"].as_u64() {
        println!("{pid_v}");
      } else {
        println!("{r}");
      }
    }
    ProcessAction::Terminate { pid, name } => {
      let mut params = json!({});
      if let Some(p) = pid {
        params["pid"] = json!(p);
      }
      if let Some(n) = name {
        params["name"] = json!(n);
      }
      executor.call("process.kill", params)?;
      println!("ok");
    }
  }
  Ok(())
}
