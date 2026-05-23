use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum StartupAction {
  /// List all startup entries
  List,
  /// Add a startup entry
  Add {
    /// Entry name
    name: String,
    /// Command to execute
    command: String,
    /// Registry location: HKLM or HKCU (default: HKCU)
    #[arg(long, default_value = "HKCU")]
    location: String,
  },
  /// Remove a startup entry
  Remove {
    /// Entry name
    name: String,
    /// Registry location: HKLM or HKCU (default: HKCU)
    #[arg(long, default_value = "HKCU")]
    location: String,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: StartupAction) -> Result<()> {
  match action {
    StartupAction::List => {
      let r = executor.call("system.info", json!({}))?;
      let startups = r["startup_entries"].as_array();
      let Some(arr) = startups else { println!("No startup entries found."); return Ok(()) };
      if arr.is_empty() { println!("No startup entries found."); return Ok(()); }
      let mut t = super::Table::new(vec![("NAME", 42), ("LOCATION", 10), ("COMMAND", 40)]);
      for s in arr {
        let name = s["name"].as_str().unwrap_or("?");
        let loc = s["location"].as_str().unwrap_or("");
        let cmd = s["command"].as_str().unwrap_or("");
        t.row(vec![name.to_string(), loc.to_string(), cmd.to_string()]);
      }
      t.print();
    }
    StartupAction::Add { name, command, location } => {
      executor.call("startup.add", json!({
        "name": name,
        "command": command,
        "location": location,
      }))?;
      println!("ok");
    }
    StartupAction::Remove { name, location } => {
      executor.call("startup.remove", json!({
        "name": name,
        "location": location,
      }))?;
      println!("ok");
    }
  }
  Ok(())
}
