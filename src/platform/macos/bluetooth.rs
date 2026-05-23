use std::time::Duration;

use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::Manager;

use crate::error::Result;
use crate::types::*;

pub fn scan_ble(duration_ms: u64) -> Result<Vec<BluetoothDeviceInfo>> {
  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_time()
    .build()
    .map_err(|e| anyhow::anyhow!("failed to create tokio runtime: {e}"))?;

  rt.block_on(async {
    let manager = Manager::new()
      .await
      .map_err(|e| anyhow::anyhow!("BLE manager init failed: {e}"))?;

    let adapters = manager
      .adapters()
      .await
      .map_err(|e| anyhow::anyhow!("BLE adapter list failed: {e}"))?;

    let adapter = adapters
      .into_iter()
      .next()
      .ok_or_else(|| anyhow::anyhow!("no Bluetooth adapter found"))?;

    adapter
      .start_scan(ScanFilter::default())
      .await
      .map_err(|e| anyhow::anyhow!("BLE scan start failed: {e}"))?;

    tokio::time::sleep(Duration::from_millis(duration_ms)).await;

    let peripherals = adapter
      .peripherals()
      .await
      .map_err(|e| anyhow::anyhow!("BLE peripheral list failed: {e}"))?;

    let mut devices = Vec::new();
    for peripheral in peripherals {
      let props = match peripheral.properties().await {
        Ok(Some(p)) => p,
        _ => continue,
      };

      let name = props
        .local_name
        .unwrap_or_else(|| format!("Unknown ({})", props.address));

      let is_connected = peripheral.is_connected().await.unwrap_or(false);

      devices.push(BluetoothDeviceInfo {
        name,
        address: props.address.to_string(),
        is_connected,
        is_paired: false,
        source: "BLE".to_string(),
        rssi: props.rssi,
      });
    }

    adapter
      .stop_scan()
      .await
      .map_err(|e| anyhow::anyhow!("BLE scan stop failed: {e}"))?;

    Ok(devices)
  })
}
