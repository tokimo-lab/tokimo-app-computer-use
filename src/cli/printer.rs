use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum PrinterAction {
  /// List all printers
  List,
  /// Get printer details by index
  Detail { index: usize },
  /// Print a file to a printer
  Print {
    /// Printer index or name (use index from `printer list`)
    printer: String,
    /// File path to print
    file: String,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: PrinterAction) -> Result<()> {
  match action {
    PrinterAction::List => {
      let r = executor.call("printer.list", json!({}))?;
      let arr = r.as_array();
      let Some(arr) = arr else {
        println!("No printers found.");
        return Ok(());
      };
      if arr.is_empty() {
        println!("No printers found.");
        return Ok(());
      }
      let mut t = super::Table::new(vec![
        ("IDX", 4),
        ("NAME", 36),
        ("DRIVER", 25),
        ("DEFAULT", 8),
        ("SHARED", 6),
      ]);
      for (i, p) in arr.iter().enumerate() {
        let name = p["name"].as_str().unwrap_or("?");
        let driver = p["driver"].as_str().unwrap_or("");
        let is_default = if p["is_default"].as_bool().unwrap_or(false) {
          "Yes"
        } else {
          ""
        };
        let is_shared = if p["is_shared"].as_bool().unwrap_or(false) {
          "Yes"
        } else {
          ""
        };
        t.row(vec![
          i.to_string(),
          name.to_string(),
          driver.to_string(),
          is_default.to_string(),
          is_shared.to_string(),
        ]);
      }
      t.print();
    }
    PrinterAction::Detail { index } => {
      let r = executor.call("printer.list", json!({}))?;
      let arr = r.as_array().ok_or_else(|| anyhow::anyhow!("No printers found"))?;
      let p = arr
        .get(index)
        .ok_or_else(|| anyhow::anyhow!("Printer index {index} out of range"))?;
      super::kv_print(&[
        ("Name:", p["name"].as_str().unwrap_or("?")),
        ("Driver:", p["driver"].as_str().unwrap_or("")),
        ("Port:", p["port"].as_str().unwrap_or("")),
        (
          "Default:",
          if p["is_default"].as_bool().unwrap_or(false) {
            "Yes"
          } else {
            "No"
          },
        ),
        (
          "Shared:",
          if p["is_shared"].as_bool().unwrap_or(false) {
            "Yes"
          } else {
            "No"
          },
        ),
      ]);
    }
    PrinterAction::Print { printer, file } => {
      let path = std::path::Path::new(&file);
      if !path.exists() {
        anyhow::bail!("File not found: {file}");
      }
      // Resolve printer: if it's a numeric index, look up the name
      let printer_name = if let Ok(idx) = printer.parse::<usize>() {
        let r = executor.call("printer.list", json!({}))?;
        let arr = r.as_array().ok_or_else(|| anyhow::anyhow!("No printers found"))?;
        let p = arr
          .get(idx)
          .ok_or_else(|| anyhow::anyhow!("Printer index {idx} out of range"))?;
        p["name"]
          .as_str()
          .ok_or_else(|| anyhow::anyhow!("Invalid printer name"))?
          .to_string()
      } else {
        printer
      };
      let abs_path = path.canonicalize().context("invalid file path")?;
      executor.call(
        "printer.print",
        json!({
          "file_path": abs_path.to_string_lossy(),
          "printer_name": printer_name
        }),
      )?;
      println!("ok");
    }
  }
  Ok(())
}
