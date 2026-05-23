use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum SoftwareAction {
  /// List installed software
  List {
    /// Filter by name or publisher
    #[arg(long, short)]
    filter: Option<String>,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: SoftwareAction) -> Result<()> {
  match action {
    SoftwareAction::List { filter } => {
      let mut params = json!({});
      if let Some(f) = filter {
        params["filter"] = json!(f);
      }
      let r = executor.call("software.get_installed", params)?;
      print_software(&r);
    }
  }
  Ok(())
}

fn print_software(value: &serde_json::Value) {
  if let Some(arr) = value.as_array() {
    if arr.is_empty() {
      println!("No installed software found.");
      return;
    }
    let mut t = super::Table::new(vec![("NAME", 36), ("VERSION", 14), ("PUBLISHER", 24), ("INSTALL_LOCATION", 40), ("INSTALL_DATE", 12), ("SIZE_KB", 10)]);
    for s in arr {
      let name = s["name"].as_str().unwrap_or("");
      let version = s["version"].as_str().unwrap_or("");
      let publisher = s["publisher"].as_str().unwrap_or("");
      let location = s["installLocation"].as_str().unwrap_or("");
      let date = s["installDate"].as_str().unwrap_or("");
      let size = s["estimatedSizeKB"].as_u64().map(|v| v.to_string()).unwrap_or_default();
      t.row(vec![name.to_string(), version.to_string(), publisher.to_string(), location.to_string(), date.to_string(), size]);
    }
    t.print();
  }
}
