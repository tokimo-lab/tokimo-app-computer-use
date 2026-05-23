use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum UsbAction {
  /// List all USB devices
  List,
  /// Get details of a USB device by index
  Detail { index: usize },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: UsbAction) -> Result<()> {
  let r = executor.call("system.info", json!({}))?;
  let usbs = r["usbs"].as_array();

  match action {
    UsbAction::List => {
      let Some(arr) = usbs else {
        println!("No USB devices found.");
        return Ok(());
      };
      if arr.is_empty() {
        println!("No USB devices found.");
        return Ok(());
      }
      let mut t = super::Table::new(vec![("IDX", 4), ("NAME", 36), ("VID:PID", 9), ("MANUFACTURER", 28)]);
      for (i, u) in arr.iter().enumerate() {
        let name = u["name"].as_str().unwrap_or("?");
        let vid = u["vid"].as_str().unwrap_or("");
        let pid = u["pid"].as_str().unwrap_or("");
        let mfr = u["manufacturer"].as_str().unwrap_or("");
        let id = if !vid.is_empty() {
          format!("{vid}:{pid}")
        } else {
          "-".to_string()
        };
        t.row(vec![i.to_string(), name.to_string(), id, mfr.to_string()]);
      }
      t.print();
    }
    UsbAction::Detail { index } => {
      let Some(arr) = usbs else {
        anyhow::bail!("No USB devices found")
      };
      let u = arr
        .get(index)
        .ok_or_else(|| anyhow::anyhow!("USB device index {index} out of range (0..{})", arr.len()))?;
      super::kv_print(&[
        ("Name:", u["name"].as_str().unwrap_or("?")),
        ("Description:", u["description"].as_str().unwrap_or("")),
        ("Manufacturer:", u["manufacturer"].as_str().unwrap_or("")),
        ("VID:", u["vid"].as_str().unwrap_or("")),
        ("PID:", u["pid"].as_str().unwrap_or("")),
        ("Serial:", u["serial_number"].as_str().unwrap_or("")),
      ]);
    }
  }
  Ok(())
}
