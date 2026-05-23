use crate::error::Result;
use crate::types::BluetoothDeviceInfo;
use std::collections::HashMap;
use windows::Win32::Devices::Bluetooth::*;

/// Classic Bluetooth scan via BluetoothFindFirstDevice (takes ~12s with inquiry).
pub fn scan_classic() -> Result<Vec<BluetoothDeviceInfo>> {
  let mut devices = Vec::new();

  unsafe {
    let mut sp: BLUETOOTH_DEVICE_SEARCH_PARAMS = std::mem::zeroed();
    sp.dwSize = std::mem::size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32;
    sp.fReturnAuthenticated = true.into();
    sp.fReturnRemembered = true.into();
    sp.fReturnUnknown = true.into();
    sp.fReturnConnected = true.into();
    sp.fIssueInquiry = true.into();
    sp.cTimeoutMultiplier = 10;

    let mut info: BLUETOOTH_DEVICE_INFO = std::mem::zeroed();
    info.dwSize = std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32;

    let handle = match BluetoothFindFirstDevice(&sp, &mut info) {
      Ok(h) => h,
      Err(e) if e.code().0 as u32 == 0x80070103 => return Ok(devices),
      Err(e) => return Err(e.into()),
    };

    loop {
      let name = bt_name(&info);
      let address = format_bt_address(&info.Address);
      devices.push(BluetoothDeviceInfo {
        name,
        address,
        is_connected: info.fConnected.as_bool(),
        is_paired: info.fRemembered.as_bool(),
        source: "classic".to_string(),
        rssi: None,
      });

      info = std::mem::zeroed();
      info.dwSize = std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32;
      if !BluetoothFindNextDevice(handle, &mut info).is_ok() {
        break;
      }
    }
    let _ = BluetoothFindDeviceClose(handle);
  }

  Ok(devices)
}

/// BLE scan via WinRT BluetoothLEAdvertisementWatcher (listens for advertisements).
pub fn scan_ble(duration_ms: u64) -> Result<Vec<BluetoothDeviceInfo>> {
  use windows::Devices::Bluetooth::Advertisement::*;
  use windows::Foundation::*;

  let watcher = BluetoothLEAdvertisementWatcher::new()?;
  watcher.SetScanningMode(BluetoothLEScanningMode(1))?; // Active

  let found: std::sync::Arc<std::sync::Mutex<HashMap<u64, BluetoothDeviceInfo>>> =
    std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
  let found_clone = found.clone();

  let handler = TypedEventHandler::<BluetoothLEAdvertisementWatcher, BluetoothLEAdvertisementReceivedEventArgs>::new(
    move |_sender, args| {
      if let Some(args) = args.as_ref() {
        let addr_u64 = args.BluetoothAddress().unwrap_or(0);
        if addr_u64 == 0 {
          return Ok(());
        }

        let address = format_bt_addr_u64(addr_u64);
        let rssi = args.RawSignalStrengthInDBm().ok();
        let name = args
          .Advertisement()
          .and_then(|adv| adv.LocalName())
          .map(|s| s.to_string())
          .unwrap_or_default();
        let name = if name.is_empty() { address.clone() } else { name };

        let dev = BluetoothDeviceInfo {
          name,
          address,
          is_connected: false,
          is_paired: false,
          source: "ble".to_string(),
          rssi,
        };

        if let Ok(mut map) = found_clone.lock() {
          map.entry(addr_u64).or_insert(dev);
        }
      }
      Ok(())
    },
  );

  watcher.Received(&handler)?;
  watcher.Start()?;

  // Spin until duration expires
  let deadline = std::time::Instant::now() + std::time::Duration::from_millis(duration_ms);
  while std::time::Instant::now() < deadline {
    std::thread::sleep(std::time::Duration::from_millis(200));
  }

  let _ = watcher.Stop();

  let map = found.lock().unwrap();
  Ok(map.values().cloned().collect())
}

/// List all known Bluetooth devices from PnP (includes BLE devices the OS knows about).
pub fn list_pnp() -> Result<Vec<BluetoothDeviceInfo>> {
  use super::terminal::execute_command;

  let result = execute_command(
    "ps",
    "Get-PnpDevice -Class Bluetooth -Status OK,Error,Degraded,Unknown | Select-Object Status, FriendlyName, InstanceId | ConvertTo-Json -Compress",
  )?;

  let stdout = result.stdout.trim();
  if stdout.is_empty() {
    return Ok(Vec::new());
  }

  let mut devices = Vec::new();

  // Handle both single object and array
  let json_val: serde_json::Value = serde_json::from_str(stdout).unwrap_or(serde_json::Value::Null);

  let arr = match &json_val {
    serde_json::Value::Array(a) => a.clone(),
    serde_json::Value::Object(_) => vec![json_val],
    _ => return Ok(Vec::new()),
  };

  for item in &arr {
    let friendly = item["FriendlyName"].as_str().unwrap_or("").to_string();
    let status = item["Status"].as_str().unwrap_or("");
    let instance_id = item["InstanceId"].as_str().unwrap_or("");

    // Extract address from instance ID if present (format: ...DEV_XXXXXXXXXXXX)
    let address = if let Some(pos) = instance_id.rfind("DEV_") {
      let hex = &instance_id[pos + 4..pos + 16];
      if hex.len() == 12 {
        format!(
          "{}:{}:{}:{}:{}:{}",
          &hex[0..2],
          &hex[2..4],
          &hex[4..6],
          &hex[6..8],
          &hex[8..10],
          &hex[10..12]
        )
      } else {
        String::new()
      }
    } else {
      String::new()
    };

    if friendly.is_empty() {
      continue;
    }

    devices.push(BluetoothDeviceInfo {
      name: friendly,
      address,
      is_connected: status.eq_ignore_ascii_case("OK"),
      is_paired: true,
      source: "pnp".to_string(),
      rssi: None,
    });
  }

  Ok(devices)
}

fn bt_name(info: &BLUETOOTH_DEVICE_INFO) -> String {
  if !info.szName.is_empty() {
    String::from_utf16_lossy(&info.szName)
      .trim_end_matches('\0')
      .to_string()
  } else {
    format_bt_address(&info.Address)
  }
}

fn format_bt_address(addr: &BLUETOOTH_ADDRESS) -> String {
  let b = unsafe { addr.Anonymous.rgBytes };
  format!(
    "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
    b[5], b[4], b[3], b[2], b[1], b[0]
  )
}

fn format_bt_addr_u64(addr: u64) -> String {
  let b = addr.to_le_bytes();
  format!(
    "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
    b[5], b[4], b[3], b[2], b[1], b[0]
  )
}
