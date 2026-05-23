use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::{CommandExecutor, format_bytes};

#[derive(Subcommand, Debug)]
pub enum SystemAction {
  Info,
  ScreenSize,
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: SystemAction) -> Result<()> {
  match action {
    SystemAction::Info => {
      let r = executor.call("system.info", json!({}))?;

      // Basic info
      println!("Computer: {}", r["computer_name"].as_str().unwrap_or("?"));
      println!("User:     {}", r["username"].as_str().unwrap_or("?"));
      println!("OS:       {}", r["os_version"].as_str().unwrap_or("?"));
      println!("Locale:   {}", r["locale"].as_str().unwrap_or("?"));
      println!("Language: {}", r["ui_language"].as_str().unwrap_or("?"));
      println!("Screen:   {}x{}", r["screen_width"].as_i64().unwrap_or(0), r["screen_height"].as_i64().unwrap_or(0));

      // CPU
      if let Some(cpu) = r.get("cpu") {
        println!();
        println!("CPU:      {}", cpu["name"].as_str().unwrap_or("?"));
        println!("Cores:    {} physical, {} logical", cpu["cores"].as_u64().unwrap_or(0), cpu["logical_processors"].as_u64().unwrap_or(0));
      }

      // Memory
      if let Some(mem) = r.get("memory") {
        let total = mem["total_bytes"].as_u64().unwrap_or(0);
        let used = mem["used_bytes"].as_u64().unwrap_or(0);
        let avail = mem["available_bytes"].as_u64().unwrap_or(0);
        let pct = mem["usage_percent"].as_u64().unwrap_or(0);
        println!();
        println!("Memory:    {} / {} ({}% used)", format_bytes(used), format_bytes(total), pct);
        println!("Available: {}", format_bytes(avail));
      }

      // Disks
      if let Some(disks) = r["disks"].as_array() {
        if !disks.is_empty() {
          println!();
          println!("Disks:");
          for d in disks {
            let drive = d["drive"].as_str().unwrap_or("?");
            let total = d["total_bytes"].as_u64().unwrap_or(0);
            let used = d["used_bytes"].as_u64().unwrap_or(0);
            let free = d["free_bytes"].as_u64().unwrap_or(0);
            let pct = if total > 0 { used * 100 / total } else { 0 };
            println!("  {drive}:  {} / {} ({}% used)  free: {}", format_bytes(used), format_bytes(total), pct, format_bytes(free));
          }
        }
      }

      // Networks
      if let Some(nets) = r["networks"].as_array() {
        if !nets.is_empty() {
          println!();
          println!("Network:");
          for n in nets {
            let name = n["name"].as_str().unwrap_or("?");
            let up = n["is_up"].as_bool().unwrap_or(false);
            let mac = n["mac_address"].as_str().unwrap_or("");
            let status = if up { "UP" } else { "DOWN" };
            let ips: Vec<String> = n["ip_addresses"].as_array().unwrap_or(&vec![]).iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
            let ip_str = if ips.is_empty() { String::from("-") } else { ips.join(", ") };
            println!("  {name} [{status}]  MAC: {mac}  IP: {ip_str}");
          }
        }
      }

      // Battery
      if let Some(bat) = r.get("battery") {
        if !bat.is_null() {
          println!();
          println!("Battery:");
          println!("  Power:    {}", if bat["ac_power"].as_bool().unwrap_or(false) { "AC" } else { "Battery" });
          println!("  Level:    {}%", bat["battery_percent"].as_u64().unwrap_or(0));
          let life = bat["battery_life_seconds"].as_u64().unwrap_or(0);
          if life > 0 && life != 0xFFFFFFFF { println!("  Remaining: {}s ({:.1}h)", life, life as f64 / 3600.0); }
        }
      }

      // GPUs
      if let Some(gpus) = r["gpus"].as_array() {
        if !gpus.is_empty() {
          println!();
          println!("GPU:");
          for g in gpus {
            let name = g["name"].as_str().unwrap_or("?");
            let driver = g["driver_version"].as_str().unwrap_or("");
            let dedicated = g["dedicated_video_memory"].as_u64().unwrap_or(0);
            let shared = g["shared_system_memory"].as_u64().unwrap_or(0);
            let vram = g["vram_bytes"].as_u64().unwrap_or(0);
            let mem = if dedicated > 0 { format_bytes(dedicated) } else if vram > 0 { format_bytes(vram) } else { "N/A".to_string() };
            println!("  {name}");
            if !driver.is_empty() { println!("    Driver:  {driver}"); }
            println!("    VRAM:    {mem}");
            if shared > 0 { println!("    Shared:  {}", format_bytes(shared)); }
          }
        }
      }

      // USB
      if let Some(usbs) = r["usbs"].as_array() {
        if !usbs.is_empty() {
          println!();
          println!("USB Devices ({}):", usbs.len());
          for u in usbs {
            let name = u["name"].as_str().unwrap_or("?");
            let mfr = u["manufacturer"].as_str().unwrap_or("");
            let vid = u["vid"].as_str().unwrap_or("");
            let pid = u["pid"].as_str().unwrap_or("");
            let serial = u["serial_number"].as_str().unwrap_or("");
            println!("  {name}");
            if !mfr.is_empty() { println!("    Mfr:  {mfr}"); }
            if !vid.is_empty() { print!("    ID:   {vid}:{pid}"); if !serial.is_empty() { print!(" S/N: {serial}"); } println!(); }
          }
        }
      }

      // Bluetooth
      if let Some(bts) = r["bluetooth_devices"].as_array() {
        if !bts.is_empty() {
          println!();
          println!("Bluetooth ({}):", bts.len());
          for b in bts {
            let name = b["name"].as_str().unwrap_or("?");
            let addr = b["address"].as_str().unwrap_or("");
            let status = if b["is_connected"].as_bool().unwrap_or(false) { "Connected" } else if b["is_paired"].as_bool().unwrap_or(false) { "Paired" } else { "Remembered" };
            println!("  {name}  {addr}  [{status}]");
          }
        }
      }

      // WiFi
      if let Some(wifis) = r["wifi_networks"].as_array() {
        if !wifis.is_empty() {
          println!();
          println!("WiFi ({}):", wifis.len());
          for w in wifis {
            let ssid = w["ssid"].as_str().unwrap_or("?");
            let quality = w["signal_quality"].as_u64().unwrap_or(0);
            let connected = w["is_connected"].as_bool().unwrap_or(false);
            let auth = w["auth_type"].as_str().unwrap_or("");
            println!("  [{}] {ssid}  Signal: {quality}%  Auth: {auth}", if connected { "*" } else { " " });
          }
        }
      }

      // Audio
      if let Some(audios) = r["audio_devices"].as_array() {
        if !audios.is_empty() {
          println!();
          println!("Audio ({}):", audios.len());
          for a in audios {
            let name = a["name"].as_str().unwrap_or("?");
            let dtype = a["device_type"].as_str().unwrap_or("");
            let def = if a["is_default"].as_bool().unwrap_or(false) { " [Default]" } else { "" };
            println!("  {name}  ({dtype}){def}");
          }
        }
      }

      // Printers
      if let Some(printers) = r["printers"].as_array() {
        if !printers.is_empty() {
          println!();
          println!("Printers ({}):", printers.len());
          for p in printers {
            let name = p["name"].as_str().unwrap_or("?");
            let driver = p["driver"].as_str().unwrap_or("");
            let def = if p["is_default"].as_bool().unwrap_or(false) { " [Default]" } else { "" };
            println!("  {name}{def}");
            if !driver.is_empty() { println!("    Driver: {driver}"); }
          }
        }
      }

      // Services
      if let Some(services) = r["services"].as_array() {
        if !services.is_empty() {
          println!();
          println!("Services ({}):", services.len());
          for s in services {
            println!("  {}  [{}]  {}", s["name"].as_str().unwrap_or("?"), s["status"].as_str().unwrap_or("?"), s["display_name"].as_str().unwrap_or(""));
          }
        }
      }

      // Startup
      if let Some(startups) = r["startup_entries"].as_array() {
        if !startups.is_empty() {
          println!();
          println!("Startup ({}):", startups.len());
          for s in startups {
            println!("  {}  ({})", s["name"].as_str().unwrap_or("?"), s["location"].as_str().unwrap_or(""));
            println!("    {}", s["command"].as_str().unwrap_or(""));
          }
        }
      }
    }
    SystemAction::ScreenSize => {
      let r = executor.call("system.screen_size", json!({}))?;
      println!("{}x{}", r["width"].as_u64().unwrap_or(0), r["height"].as_u64().unwrap_or(0));
    }
  }
  Ok(())
}
