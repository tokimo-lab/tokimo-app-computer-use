use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum BluetoothAction {
  /// List all known Bluetooth devices (from PnP)
  List,
  /// Classic Bluetooth scan (inquiry, takes ~12s)
  Scan,
  /// BLE scan (advertisement watcher, passive)
  Ble {
    /// Scan duration in seconds (default: 5)
    #[arg(long, default_value = "5")]
    duration: u64,
  },
  /// List all PnP Bluetooth devices (OS-enumerated, includes BLE)
  Pnp,
  /// Get details of a Bluetooth device by index
  Detail { index: usize },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: BluetoothAction) -> Result<()> {
  match action {
    BluetoothAction::List => {
      let r = executor.call("bluetooth.list_pnp", json!({}))?;
      let Some(arr) = r.as_array() else {
        println!("No Bluetooth devices found.");
        return Ok(());
      };
      if arr.is_empty() {
        println!("No Bluetooth devices found.");
        return Ok(());
      }
      print_bt_list(arr);
    }
    BluetoothAction::Scan => {
      println!("Scanning for Classic Bluetooth devices (~12s)...");
      let r = executor.call("bluetooth.scan", json!({}))?;
      let Some(arr) = r.as_array() else {
        println!("No Bluetooth devices found.");
        return Ok(());
      };
      if arr.is_empty() {
        println!("No Bluetooth devices found.");
        return Ok(());
      }
      print_bt_list(arr);
    }
    BluetoothAction::Ble { duration } => {
      println!("Scanning for BLE devices ({duration}s)...");
      let r = executor.call("bluetooth.scan_ble", json!({"duration_ms": duration * 1000}))?;
      let Some(arr) = r.as_array() else {
        println!("No BLE devices found.");
        return Ok(());
      };
      if arr.is_empty() {
        println!("No BLE devices found.");
        return Ok(());
      }
      print_bt_list(arr);
    }
    BluetoothAction::Pnp => {
      let r = executor.call("bluetooth.list_pnp", json!({}))?;
      let Some(arr) = r.as_array() else {
        println!("No Bluetooth devices found.");
        return Ok(());
      };
      if arr.is_empty() {
        println!("No Bluetooth devices found.");
        return Ok(());
      }
      print_bt_list(arr);
    }
    BluetoothAction::Detail { index } => {
      let r = executor.call("bluetooth.list_pnp", json!({}))?;
      let Some(arr) = r.as_array() else {
        anyhow::bail!("No Bluetooth devices found")
      };
      let b = arr
        .get(index)
        .ok_or_else(|| anyhow::anyhow!("Bluetooth index {index} out of range"))?;
      let rssi_str;
      let mut pairs = vec![
        ("Name:", b["name"].as_str().unwrap_or("?")),
        ("Address:", b["address"].as_str().unwrap_or("")),
        ("Source:", b["source"].as_str().unwrap_or("")),
        (
          "Connected:",
          if b["is_connected"].as_bool().unwrap_or(false) {
            "Yes"
          } else {
            "No"
          },
        ),
        (
          "Paired:",
          if b["is_paired"].as_bool().unwrap_or(false) {
            "Yes"
          } else {
            "No"
          },
        ),
      ];
      if let Some(rssi) = b["rssi"].as_i64() {
        rssi_str = format!("{rssi} dBm");
        pairs.push(("RSSI:", &rssi_str));
      }
      super::kv_print(&pairs);
    }
  }
  Ok(())
}

fn print_bt_list(arr: &[serde_json::Value]) {
  let mut t = super::Table::new(vec![
    ("IDX", 4),
    ("NAME", 30),
    ("ADDRESS", 19),
    ("SOURCE", 7),
    ("STATUS", 12),
  ]);
  for (i, b) in arr.iter().enumerate() {
    let name = b["name"].as_str().unwrap_or("?");
    let addr = b["address"].as_str().unwrap_or("");
    let source = b["source"].as_str().unwrap_or("");
    let status = if b["is_connected"].as_bool().unwrap_or(false) {
      "Connected"
    } else if b["is_paired"].as_bool().unwrap_or(false) {
      "Paired"
    } else {
      "Nearby"
    };
    let rssi = b["rssi"].as_i64().map(|r| format!(" {r}dBm")).unwrap_or_default();
    t.row(vec![
      i.to_string(),
      name.to_string(),
      addr.to_string(),
      source.to_string(),
      format!("{status}{rssi}"),
    ]);
  }
  t.print();
}
