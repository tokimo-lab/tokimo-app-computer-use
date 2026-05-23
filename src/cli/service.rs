use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum ServiceAction {
  /// List all Windows services
  List {
    /// Filter by name pattern
    #[arg(long)]
    filter: Option<String>,
  },
  /// Get service details by name
  Detail { name: String },
  /// Start a service
  Start { name: String },
  /// Stop a service
  Stop { name: String },
  /// Restart a service (stop + start)
  Restart { name: String },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: ServiceAction) -> Result<()> {
  match action {
    ServiceAction::List { filter } => {
      let r = executor.call("service.list", json!({}))?;
      let Some(arr) = r.as_array() else {
        println!("No services found.");
        return Ok(());
      };
      if arr.is_empty() {
        println!("No services found.");
        return Ok(());
      }
      let mut t = super::Table::new(vec![("NAME", 32), ("STATUS", 9), ("DISPLAY NAME", 44)]);
      for s in arr {
        let name = s["name"].as_str().unwrap_or("?");
        let display = s["display_name"].as_str().unwrap_or("");
        let status = s["status"].as_str().unwrap_or("?");
        if let Some(f) = &filter {
          let pat = f.to_lowercase();
          if !name.to_lowercase().contains(&pat) && !display.to_lowercase().contains(&pat) {
            continue;
          }
        }
        t.row(vec![name.to_string(), status.to_string(), display.to_string()]);
      }
      t.print();
    }
    ServiceAction::Detail { name } => {
      let r = executor.call("service.detail", json!({"name": name}))?;
      super::kv_print(&[
        ("Name:", r["name"].as_str().unwrap_or("?")),
        ("Display Name:", r["display_name"].as_str().unwrap_or("")),
        ("Status:", r["status"].as_str().unwrap_or("?")),
        ("Type:", r["service_type"].as_str().unwrap_or("")),
      ]);
    }
    ServiceAction::Start { name } => {
      executor.call("service.start", json!({"name": name}))?;
      println!("ok");
    }
    ServiceAction::Stop { name } => {
      executor.call("service.stop", json!({"name": name}))?;
      println!("ok");
    }
    ServiceAction::Restart { name } => {
      executor.call("service.stop", json!({"name": name}))?;
      executor.call("service.start", json!({"name": name}))?;
      println!("ok");
    }
  }
  Ok(())
}
