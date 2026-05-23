use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum WifiAction {
  /// Scan and list all visible WiFi networks
  Scan,
  /// Get details of a WiFi network by index
  Detail { index: usize },
  /// Connect to a WiFi network
  Connect {
    /// SSID to connect to
    ssid: String,
    /// Password (omit for open networks)
    #[arg(long, short = 'p')]
    password: Option<String>,
  },
  /// Disconnect from current WiFi
  Disconnect,
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: WifiAction) -> Result<()> {
  match action {
    WifiAction::Scan => {
      let r = executor.call("system.info", json!({}))?;
      let wifis = r["wifi_networks"].as_array();
      let Some(arr) = wifis else { println!("No WiFi networks found."); return Ok(()) };
      if arr.is_empty() { println!("No WiFi networks found."); return Ok(()); }
      let mut t = super::Table::new(vec![("IDX", 4), ("SSID", 32), ("SIGNAL", 7), ("AUTH", 10), ("STATUS", 7)]).align_right(2);
      for (i, w) in arr.iter().enumerate() {
        let ssid = w["ssid"].as_str().unwrap_or("?");
        let quality = format!("{}%", w["signal_quality"].as_u64().unwrap_or(0));
        let auth = w["auth_type"].as_str().unwrap_or("");
        let connected = if w["is_connected"].as_bool().unwrap_or(false) { "*" } else { "" };
        t.row(vec![i.to_string(), ssid.to_string(), quality, auth.to_string(), connected.to_string()]);
      }
      t.print();
    }
    WifiAction::Detail { index } => {
      let r = executor.call("system.info", json!({}))?;
      let arr = r["wifi_networks"].as_array().ok_or_else(|| anyhow::anyhow!("No WiFi networks found"))?;
      let w = arr.get(index).ok_or_else(|| anyhow::anyhow!("WiFi index {index} out of range"))?;
      super::kv_print(&[
        ("SSID:", w["ssid"].as_str().unwrap_or("?")),
        ("Signal:", &format!("{}%", w["signal_quality"].as_u64().unwrap_or(0))),
        ("Connected:", if w["is_connected"].as_bool().unwrap_or(false) { "Yes" } else { "No" }),
        ("BSSID:", w["bssid"].as_str().unwrap_or("")),
        ("Auth:", w["auth_type"].as_str().unwrap_or("")),
      ]);
    }
    WifiAction::Connect { ssid, password } => {
      if let Some(pwd) = &password {
        let profile_xml = format!(r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
  <name>{ssid}</name>
  <SSIDConfig><SSID><name>{ssid}</name></SSID></SSIDConfig>
  <connectionType>ESS</connectionType>
  <connectionMode>auto</connectionMode>
  <MSM><security>
    <authEncryption><authentication>WPA2PSK</authentication><encryption>AES</encryption></authEncryption>
    <sharedKey><keyType>passPhrase</keyType><protected>false</protected><keyMaterial>{pwd}</keyMaterial></sharedKey>
  </security></MSM>
</WLANProfile>"#);
        let r = executor.call("terminal.execute", json!({
          "shell_type": "cmd",
          "command": format!("echo {} > \"%TEMP%\\wifi_profile.xml\" & netsh wlan add profile filename=\"%TEMP%\\wifi_profile.xml\" & netsh wlan connect name=\"{ssid}\"", profile_xml.replace('"', "\\\""))
        }))?;
        let exit = r["exit_code"].as_i64().unwrap_or(-1);
        let stdout = r["stdout"].as_str().unwrap_or("");
        let stderr = r["stderr"].as_str().unwrap_or("");
        if !stdout.is_empty() { print!("{stdout}"); }
        if !stderr.is_empty() { eprint!("{stderr}"); }
        if exit != 0 { anyhow::bail!("WiFi connect failed (exit {exit})"); }
      } else {
        let r = executor.call("terminal.execute", json!({"shell_type": "cmd", "command": format!("netsh wlan connect name=\"{ssid}\"")}))?;
        let exit = r["exit_code"].as_i64().unwrap_or(-1);
        let stdout = r["stdout"].as_str().unwrap_or("");
        let stderr = r["stderr"].as_str().unwrap_or("");
        if !stdout.is_empty() { print!("{stdout}"); }
        if !stderr.is_empty() { eprint!("{stderr}"); }
        if exit != 0 { anyhow::bail!("WiFi connect failed (exit {exit})"); }
      }
      println!("ok");
    }
    WifiAction::Disconnect => {
      let r = executor.call("terminal.execute", json!({"shell_type": "cmd", "command": "netsh wlan disconnect"}))?;
      let exit = r["exit_code"].as_i64().unwrap_or(-1);
      let stdout = r["stdout"].as_str().unwrap_or("");
      let stderr = r["stderr"].as_str().unwrap_or("");
      if !stdout.is_empty() { print!("{stdout}"); }
      if !stderr.is_empty() { eprint!("{stderr}"); }
      if exit != 0 { anyhow::bail!("WiFi disconnect failed (exit {exit})"); }
      println!("ok");
    }
  }
  Ok(())
}
