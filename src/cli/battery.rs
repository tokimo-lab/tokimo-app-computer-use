use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum BatteryAction {
  /// Show battery status
  Status,
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: BatteryAction) -> Result<()> {
  match action {
    BatteryAction::Status => {
      let r = executor.call("system.info", json!({}))?;
      match r.get("battery") {
        Some(bat) if !bat.is_null() => {
          let ac = bat["ac_power"].as_bool().unwrap_or(false);
          let pct = bat["battery_percent"].as_u64().unwrap_or(0);
          let life = bat["battery_life_seconds"].as_u64().unwrap_or(0);
          let flag = bat["battery_flag"].as_u64().unwrap_or(0);
          println!("Power:     {}", if ac { "AC" } else { "Battery" });
          println!("Level:     {pct}%");
          println!("Flag:      {flag:#06x}");
          if life > 0 && life != 0xFFFFFFFF {
            let h = life / 3600;
            let m = (life % 3600) / 60;
            println!("Remaining: {h}h {m}m ({life}s)");
          }
          // Visual bar
          let filled = (pct as usize * 20) / 100;
          let bar: String = "█".repeat(filled) + &"░".repeat(20 - filled);
          println!("[{bar}] {pct}%");
        }
        _ => {
          println!("No battery detected (desktop system).");
        }
      }
    }
  }
  Ok(())
}
