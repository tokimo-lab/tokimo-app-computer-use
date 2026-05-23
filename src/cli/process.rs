use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum ProcessAction {
  List { #[arg(long)] filter: Option<String> },
  Info { pid: u32 },
  Launch { path: String, #[arg(long, default_value = "5000")] timeout: u32 },
  Terminate { pid: u32 },
  TerminateByName { name: String },
  GetPids { name: String },
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
        let mut t = super::Table::new(vec![("PID", 8), ("NAME", 32), ("THREADS", 8), ("PARENT", 8), ("MEMORY", 12)]);
        for p in arr {
          let pid = p["pid"].as_u64().unwrap_or(0);
          let name = p["name"].as_str().unwrap_or("?");
          let threads = p["thread_count"].as_u64().unwrap_or(0);
          let parent = p["parent_pid"].as_u64().unwrap_or(0);
          let mem = p["memory_bytes"].as_u64().unwrap_or(0);
          t.row(vec![pid.to_string(), name.to_string(), threads.to_string(), parent.to_string(), format!("{}KB", mem / 1024)]);
        }
        t.print();
      }
    }
    ProcessAction::Info { pid } => {
      let r = executor.call("process.info", json!({"pid": pid}))?;
      if let Some(obj) = r.as_object() {
        for (k, v) in obj {
          let val = match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            other => other.to_string(),
          };
          println!("{k}={val}");
        }
      }
    }
    ProcessAction::Launch { path, timeout } => {
      let r = executor.call("process.launch", json!({"path": path, "wait_timeout_ms": timeout}))?;
      if let Some(pid) = r.as_u64() { println!("{pid}"); } else { println!("{r}"); }
    }
    ProcessAction::Terminate { pid } => { executor.call("process.terminate", json!({"pid": pid}))?; println!("ok"); }
    ProcessAction::TerminateByName { name } => { executor.call("process.terminate_by_name", json!({"name": name}))?; println!("ok"); }
    ProcessAction::GetPids { name } => {
      let r = executor.call("process.get_pids", json!({"name": name}))?;
      if let Some(arr) = r.as_array() {
        for pid in arr { if let Some(n) = pid.as_u64() { println!("{n}"); } }
      }
    }
  }
  Ok(())
}
