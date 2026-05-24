#![allow(non_upper_case_globals)]

use crate::types::{
  AudioDeviceInfo, GpuInfo, PrinterInfo, SoftwareInfo, StartupEntry, SystemInfo, UsbDeviceInfo, WifiNetworkInfo,
};
use anyhow::{Context, Result};
use std::cell::RefCell;
use std::thread;

#[cfg(windows)]
use windows::Win32::System::Com::*;

thread_local! {
    static COM_SCOPE: RefCell<Option<ComScope>> = const { RefCell::new(None) };
}

pub struct ComScope;

impl ComScope {
  pub fn init_mta() -> Result<Self> {
    unsafe {
      let hr = CoInitializeEx(None, COINIT_MULTITHREADED | COINIT_DISABLE_OLE1DDE);
      match hr.0 {
        0 | 1 => Ok(Self),
        -2147417850 => Err(anyhow::anyhow!("Thread already initialized with a different COM model")),
        _ => Err(windows::core::Error::from(hr)).context("CoInitializeEx failed"),
      }
    }
  }
}

impl Drop for ComScope {
  fn drop(&mut self) {
    unsafe { CoUninitialize() };
  }
}

pub fn ensure_com_initialized() {
  COM_SCOPE.with(|scope| {
    let mut scope = scope.borrow_mut();
    if scope.is_none() {
      match ComScope::init_mta() {
        Ok(instance) => {
          *scope = Some(instance);
        }
        Err(e) => {
          eprintln!("[ERROR] COM init failed for thread {:?}: {}", thread::current().id(), e);
        }
      }
    }
  });
}

pub fn get_user_ui_language() -> windows::core::Result<String> {
  use windows::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
  use windows::core::PWSTR;
  unsafe {
    let mut num_languages = 0u32;
    let mut buffer_size = 0u32;
    let result = GetUserPreferredUILanguages(
      MUI_LANGUAGE_NAME,
      &mut num_languages,
      Some(PWSTR::null()),
      &mut buffer_size,
    );
    if result.is_err() {
      return Err(windows::core::Error::from_thread());
    }
    if buffer_size == 0 {
      return Ok("en-US".to_string());
    }
    let mut buffer = vec![0u16; buffer_size as usize];
    let result = GetUserPreferredUILanguages(
      MUI_LANGUAGE_NAME,
      &mut num_languages,
      Some(PWSTR(buffer.as_mut_ptr())),
      &mut buffer_size,
    );
    if result.is_err() {
      return Err(windows::core::Error::from_thread());
    }
    if num_languages > 0 && !buffer.is_empty() {
      let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
      let lang = String::from_utf16_lossy(&buffer[..end]);
      if !lang.is_empty() {
        return Ok(lang);
      }
    }
    Ok("en-US".to_string())
  }
}

pub fn get_system_locale_name() -> windows::core::Result<String> {
  use windows::Win32::Globalization::GetUserDefaultLocaleName;
  unsafe {
    let mut buffer = [0u16; 85];
    let result = GetUserDefaultLocaleName(&mut buffer);
    if result == 0 {
      return Err(windows::core::Error::from_thread());
    }
    Ok(String::from_utf16_lossy(&buffer[..result as usize - 1]))
  }
}

pub fn get_system_version() -> windows::core::Result<(u32, u32, u32, u32, String)> {
  use windows::core::*;
  #[allow(non_snake_case, non_camel_case_types)]
  #[repr(C)]
  struct RTL_OSVERSIONINFOW {
    dwOSVersionInfoSize: u32,
    dwMajorVersion: u32,
    dwMinorVersion: u32,
    dwBuildNumber: u32,
    dwPlatformId: u32,
    szCSDVersion: [u16; 128],
  }
  unsafe {
    unsafe extern "system" {
      fn RtlGetVersion(lpVersionInformation: *mut RTL_OSVERSIONINFOW) -> i32;
    }
    let mut vi = RTL_OSVERSIONINFOW {
      dwOSVersionInfoSize: std::mem::size_of::<RTL_OSVERSIONINFOW>() as u32,
      ..std::mem::zeroed()
    };
    let status = RtlGetVersion(&mut vi);
    if status != 0 {
      return Err(windows::core::Error::from_hresult(HRESULT(status)));
    }
    let vs = format!(
      "Microsoft Windows [Version {}.{}.{}]",
      vi.dwMajorVersion, vi.dwMinorVersion, vi.dwBuildNumber
    );
    Ok((
      vi.dwMajorVersion,
      vi.dwMinorVersion,
      vi.dwBuildNumber,
      vi.dwPlatformId,
      vs,
    ))
  }
}

fn enumerate_gpus() -> Vec<GpuInfo> {
  use windows::Win32::Graphics::Dxgi::*;

  let factory: Option<IDXGIFactory1> = match unsafe { CreateDXGIFactory1() } {
    Ok(f) => Some(f),
    Err(_) => return Vec::new(),
  };

  let factory = match factory {
    Some(f) => f,
    None => return Vec::new(),
  };

  let mut gpus = Vec::new();
  let mut i = 0u32;
  loop {
    let adapter = match unsafe { factory.EnumAdapters1(i) } {
      Ok(a) => a,
      Err(_) => break,
    };
    i += 1;

    let desc = match unsafe { adapter.GetDesc1() } {
      Ok(d) => d,
      Err(_) => continue,
    };

    let name = String::from_utf16_lossy(
      &desc.Description[..desc
        .Description
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(desc.Description.len())],
    );

    let vendor_id = desc.VendorId;
    let device_id = desc.DeviceId;
    let dedicated_video_memory = desc.DedicatedVideoMemory as u64;
    let shared_system_memory = desc.SharedSystemMemory as u64;
    let vram_bytes = dedicated_video_memory;
    let is_software = (desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32)) != 0;

    gpus.push(GpuInfo {
      name,
      driver_version: None,
      provider_name: None,
      driver_date: None,
      vendor_id,
      device_id,
      dedicated_video_memory,
      shared_system_memory,
      vram_bytes,
      is_software,
      is_remote: false,
    });
  }

  gpus
}

fn enumerate_audio_devices() -> Vec<AudioDeviceInfo> {
  use windows::Win32::Media::Audio::*;
  use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};

  ensure_com_initialized();

  let enumerator: IMMDeviceEnumerator = match unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) } {
    Ok(e) => e,
    Err(_) => return Vec::new(),
  };

  let mut devices = Vec::new();

  // Get default render and capture device IDs for marking
  let default_render_id = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
    .ok()
    .and_then(|d| unsafe { d.GetId() }.ok())
    .map(|p| unsafe { p.to_string() }.unwrap_or_default());
  let default_capture_id = unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eConsole) }
    .ok()
    .and_then(|d| unsafe { d.GetId() }.ok())
    .map(|p| unsafe { p.to_string() }.unwrap_or_default());

  for flow in [eRender, eCapture] {
    let collection = match unsafe { enumerator.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE) } {
      Ok(c) => c,
      Err(_) => continue,
    };
    let count = match unsafe { collection.GetCount() } {
      Ok(c) => c,
      Err(_) => continue,
    };
    for i in 0..count {
      let device = match unsafe { collection.Item(i) } {
        Ok(d) => d,
        Err(_) => continue,
      };
      let device_id = unsafe { device.GetId() }
        .map(|p| unsafe { p.to_string() }.unwrap_or_default())
        .unwrap_or_default();
      let device_type = if flow == eRender { "output" } else { "input" };
      let is_default = if flow == eRender {
        default_render_id.as_deref() == Some(device_id.as_str())
      } else {
        default_capture_id.as_deref() == Some(device_id.as_str())
      };

      // Try to get friendly name from property store
      let name = unsafe { device.OpenPropertyStore(windows::Win32::System::Com::STGM(0)) }
        .ok()
        .and_then(|ps| {
          use windows::Win32::Foundation::PROPERTYKEY;
          // PKEY_Device_FriendlyName: {A45C254E-DF1C-4EFD-8020-67D146A850E0}, PID 14
          let pkey = PROPERTYKEY {
            fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
            pid: 14,
          };
          unsafe { ps.GetValue(&pkey) }.ok().and_then(|pv| {
            // PROPVARIANT with VT_LPWSTR: the anonymous field contains a PWSTR
            unsafe {
              let pstr = pv.Anonymous.Anonymous.Anonymous.pwszVal;
              if pstr.is_null() {
                None
              } else {
                Some(pstr.to_string().unwrap_or_default())
              }
            }
          })
        })
        .unwrap_or_else(|| device_id.clone());

      // Try to read volume and mute state
      let (volume, muted) = {
        use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
        let vol: Option<IAudioEndpointVolume> = unsafe { device.Activate(CLSCTX_ALL, None) }.ok();
        vol
          .map(|v| {
            let scalar = unsafe { v.GetMasterVolumeLevelScalar() }.unwrap_or(0.0);
            let m = unsafe { v.GetMute() }.map(|b| b.as_bool()).unwrap_or(false);
            ((scalar * 100.0).round() as u32, m)
          })
          .unwrap_or((0, false))
      };

      devices.push(AudioDeviceInfo {
        name,
        device_id,
        device_type: device_type.to_string(),
        is_default,
        volume,
        muted,
      });
    }
  }

  devices
}

fn enumerate_usb_devices() -> Vec<UsbDeviceInfo> {
  use windows::Win32::Devices::DeviceAndDriverInstallation::*;

  let mut devices = Vec::new();

  // DIGCF_PRESENT | DIGCF_ALLCLASSES
  let flags = DIGCF_PRESENT | DIGCF_ALLCLASSES;
  let hdev = match unsafe { SetupDiGetClassDevsW(None, windows::core::w!("USB"), None, flags) } {
    Ok(h) => h,
    Err(_) => return devices,
  };

  let mut index = 0u32;
  loop {
    let mut devinfo = SP_DEVINFO_DATA {
      cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
      ..unsafe { std::mem::zeroed() }
    };
    if unsafe { SetupDiEnumDeviceInfo(hdev, index, &mut devinfo) }.is_err() {
      break;
    }
    index += 1;

    let name = get_device_registry_string(hdev, &devinfo, SPDRP_FRIENDLYNAME)
      .or_else(|| get_device_registry_string(hdev, &devinfo, SPDRP_DEVICEDESC))
      .unwrap_or_default();
    let description = get_device_registry_string(hdev, &devinfo, SPDRP_DEVICEDESC).unwrap_or_default();
    let manufacturer = get_device_registry_string(hdev, &devinfo, SPDRP_MFG).unwrap_or_default();
    let hardware_id = get_device_registry_string(hdev, &devinfo, SPDRP_HARDWAREID).unwrap_or_default();

    let (vid, pid) = parse_vid_pid(&hardware_id);
    // Serial number: extract from device instance ID (last segment after last '\')
    let serial_number = get_device_instance_id(hdev, &devinfo)
      .and_then(|id| id.rsplit_once('\\').map(|(_, s)| s.to_string()))
      .filter(|s| !s.is_empty() && s != &format!("USB#VID_{}&PID_{}", vid, pid))
      .unwrap_or_default();

    if !name.is_empty() {
      devices.push(UsbDeviceInfo {
        name,
        description,
        manufacturer,
        vid,
        pid,
        serial_number,
      });
    }
  }

  unsafe {
    let _ = SetupDiDestroyDeviceInfoList(hdev);
  }
  devices
}

fn get_device_instance_id(
  hdev: windows::Win32::Devices::DeviceAndDriverInstallation::HDEVINFO,
  devinfo: &windows::Win32::Devices::DeviceAndDriverInstallation::SP_DEVINFO_DATA,
) -> Option<String> {
  let mut buf = [0u16; 512];
  let mut needed = 0u32;
  let result = unsafe {
    windows::Win32::Devices::DeviceAndDriverInstallation::SetupDiGetDeviceInstanceIdW(
      hdev,
      devinfo,
      Some(&mut buf),
      Some(&mut needed),
    )
  };
  if result.is_ok() {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..len]);
    if s.is_empty() { None } else { Some(s) }
  } else {
    None
  }
}

fn get_device_registry_string(
  hdev: windows::Win32::Devices::DeviceAndDriverInstallation::HDEVINFO,
  devinfo: &windows::Win32::Devices::DeviceAndDriverInstallation::SP_DEVINFO_DATA,
  property: windows::Win32::Devices::DeviceAndDriverInstallation::SETUP_DI_REGISTRY_PROPERTY,
) -> Option<String> {
  let mut buf = [0u16; 512];
  let buf_bytes = unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, buf.len() * 2) };
  let result = unsafe {
    windows::Win32::Devices::DeviceAndDriverInstallation::SetupDiGetDeviceRegistryPropertyW(
      hdev,
      devinfo,
      property,
      None,
      Some(buf_bytes),
      None,
    )
  };
  if result.is_ok() {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..len]);
    if s.is_empty() { None } else { Some(s) }
  } else {
    None
  }
}

fn parse_vid_pid(hardware_id: &str) -> (String, String) {
  let upper = hardware_id.to_uppercase();
  let vid = upper
    .find("VID_")
    .and_then(|pos| {
      let rest = &upper[pos + 4..];
      let end = rest.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(rest.len());
      if end >= 4 { Some(rest[..end].to_string()) } else { None }
    })
    .unwrap_or_default();
  let pid = upper
    .find("PID_")
    .and_then(|pos| {
      let rest = &upper[pos + 4..];
      let end = rest.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(rest.len());
      if end >= 4 { Some(rest[..end].to_string()) } else { None }
    })
    .unwrap_or_default();
  (vid, pid)
}

fn enumerate_startup_entries() -> Vec<StartupEntry> {
  use windows::Win32::Foundation::WIN32_ERROR;
  use windows::Win32::System::Registry::*;

  let mut entries = Vec::new();
  let keys = [
    (
      HKEY_CURRENT_USER,
      r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
      "HKCU",
    ),
    (
      HKEY_LOCAL_MACHINE,
      r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
      "HKLM",
    ),
  ];

  for (root, subkey, location) in &keys {
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = HKEY::default();
    let status = unsafe {
      RegOpenKeyExW(
        *root,
        windows::core::PCWSTR(subkey_w.as_ptr()),
        Some(0),
        KEY_READ,
        &mut hkey,
      )
    };
    if status != WIN32_ERROR(0) {
      continue;
    }

    let mut index = 0u32;
    loop {
      let mut name_buf = [0u16; 256];
      let mut name_len = name_buf.len() as u32;
      let mut data_type: u32 = 0;
      let mut value_buf = [0u16; 1024];
      let mut value_len = (value_buf.len() * 2) as u32;

      let status = unsafe {
        RegEnumValueW(
          hkey,
          index,
          Some(windows::core::PWSTR(name_buf.as_mut_ptr())),
          &mut name_len,
          None,
          Some(&mut data_type),
          Some(value_buf.as_mut_ptr() as *mut u8),
          Some(&mut value_len),
        )
      };
      if status != WIN32_ERROR(0) {
        break;
      }
      index += 1;

      // REG_SZ = 1, REG_EXPAND_SZ = 2
      if data_type != 1 && data_type != 2 {
        continue;
      }

      let name_len = name_buf.iter().position(|&c| c == 0).unwrap_or(name_buf.len());
      let name = String::from_utf16_lossy(&name_buf[..name_len]);
      let val_len = value_buf.iter().position(|&c| c == 0).unwrap_or(value_buf.len());
      let command = String::from_utf16_lossy(&value_buf[..val_len]);

      if !name.is_empty() {
        entries.push(StartupEntry {
          name,
          command,
          location: location.to_string(),
        });
      }
    }

    unsafe {
      let _ = RegCloseKey(hkey);
    };
  }

  entries
}

fn enumerate_wifi_networks() -> Vec<WifiNetworkInfo> {
  use std::os::windows::process::CommandExt;
  let no_window = 0x08000000u32; // CREATE_NO_WINDOW

  // Get currently connected SSID
  let connected_ssid = std::process::Command::new("netsh")
    .args(["wlan", "show", "interfaces"])
    .creation_flags(no_window)
    .output()
    .ok()
    .filter(|o| o.status.success())
    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
    .and_then(|output| {
      let mut ssid = String::new();
      let mut connected = false;
      for line in output.lines() {
        let line = line.trim();
        if line.starts_with("State") && line.contains(':') {
          let val = line.splitn(2, ':').nth(1).unwrap_or("").trim().to_lowercase();
          connected = val.contains("connected");
        }
        if connected && line.starts_with("SSID") && !line.starts_with("BSSID") && line.contains(':') {
          ssid = line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string();
          break;
        }
      }
      if ssid.is_empty() { None } else { Some(ssid) }
    });

  // Get available networks
  let output = std::process::Command::new("netsh")
    .args(["wlan", "show", "networks", "mode=bssid"])
    .creation_flags(no_window)
    .output();
  let output = match output {
    Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
    _ => return Vec::new(),
  };

  let mut networks = Vec::new();
  let mut current_ssid = String::new();
  let mut current_auth = String::new();
  let mut current_signal = 0u32;
  let mut current_bssid = None;
  let mut in_network = false;

  for line in output.lines() {
    let line = line.trim();
    if line.starts_with("SSID ") && line.contains(':') {
      if in_network && !current_ssid.is_empty() {
        networks.push(WifiNetworkInfo {
          ssid: current_ssid.clone(),
          signal_quality: current_signal,
          bssid: current_bssid.clone(),
          auth_type: if current_auth.is_empty() {
            None
          } else {
            Some(current_auth.clone())
          },
          is_connected: connected_ssid.as_deref() == Some(current_ssid.as_str()),
        });
      }
      current_ssid = line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string();
      current_auth.clear();
      current_signal = 0;
      current_bssid = None;
      in_network = true;
    } else if line.starts_with("Authentication") && line.contains(':') {
      current_auth = line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string();
    } else if line.starts_with("BSSID") && line.contains(':') {
      current_bssid = Some(line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string());
    } else if line.starts_with("Signal") && line.contains(':') {
      let sig_str = line.splitn(2, ':').nth(1).unwrap_or("").trim().replace('%', "");
      current_signal = sig_str.parse().unwrap_or(0);
    }
  }
  if in_network && !current_ssid.is_empty() {
    networks.push(WifiNetworkInfo {
      ssid: current_ssid.clone(),
      signal_quality: current_signal,
      bssid: current_bssid,
      auth_type: if current_auth.is_empty() {
        None
      } else {
        Some(current_auth)
      },
      is_connected: connected_ssid.as_deref() == Some(current_ssid.as_str()),
    });
  }

  networks
}

fn get_endpoint_volume(
  device_index: Option<usize>,
) -> Result<windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume> {
  use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
  use windows::Win32::Media::Audio::*;
  use windows::Win32::System::Com::CoCreateInstance;

  ensure_com_initialized();

  let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
    .context("Failed to create IMMDeviceEnumerator")?;

  let device = match device_index {
    Some(idx) => {
      let collection = unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }
        .context("Failed to enumerate audio endpoints")?;
      let count = unsafe { collection.GetCount() }.context("Failed to get device count")?;
      if idx >= count as usize {
        anyhow::bail!("Audio device index {idx} out of range (0..{count})");
      }
      unsafe { collection.Item(idx as u32) }.context("Failed to get audio device")?
    }
    None => unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
      .context("No default audio output device found")?,
  };

  let vol: IAudioEndpointVolume =
    unsafe { device.Activate(CLSCTX_ALL, None) }.context("Failed to activate IAudioEndpointVolume")?;

  Ok(vol)
}

pub fn set_audio_volume(device_index: Option<usize>, level: u32) -> Result<()> {
  let vol = get_endpoint_volume(device_index)?;
  let scalar = (level.min(100) as f32) / 100.0;
  unsafe { vol.SetMasterVolumeLevelScalar(scalar, std::ptr::null()) }.context("SetMasterVolumeLevelScalar failed")?;
  Ok(())
}

pub fn get_audio_volume(device_index: Option<usize>) -> Result<u32> {
  let vol = get_endpoint_volume(device_index)?;
  let scalar = unsafe { vol.GetMasterVolumeLevelScalar() }.context("GetMasterVolumeLevelScalar failed")?;
  Ok((scalar * 100.0).round() as u32)
}

pub fn set_audio_mute(device_index: Option<usize>, muted: bool) -> Result<()> {
  let vol = get_endpoint_volume(device_index)?;
  unsafe { vol.SetMute(muted, std::ptr::null()) }.context("SetMute failed")?;
  Ok(())
}

pub fn get_audio_mute(device_index: Option<usize>) -> Result<bool> {
  let vol = get_endpoint_volume(device_index)?;
  let muted = unsafe { vol.GetMute() }.context("GetMute failed")?;
  Ok(muted.as_bool())
}

pub fn set_default_audio_device(device_id: &str) -> Result<()> {
  use windows::Win32::System::Com::CLSCTX_ALL;

  ensure_com_initialized();

  // IPolicyConfig COM interface (undocumented, Windows 10/11)
  // CLSID_CPolicyConfigClient
  let clsid = windows::core::GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
  // IID_IPolicyConfig
  let iid = windows::core::GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);

  type SetDefaultEndpointFn = unsafe extern "system" fn(
    this: *mut core::ffi::c_void,
    device_id: windows::core::PCWSTR,
    role: u32,
  ) -> windows::core::HRESULT;

  type ReleaseFn = unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32;

  #[repr(C)]
  struct IPolicyConfigVtbl {
    // IUnknown
    _query_interface: usize,
    _add_ref: usize,
    release: ReleaseFn,
    // IPolicyConfig methods (10 before SetDefaultEndpoint)
    _get_mix_format: usize,
    _get_device_format: usize,
    _reset_device_format: usize,
    _set_device_format: usize,
    _get_processing_period: usize,
    _set_processing_period: usize,
    _get_share_mode: usize,
    _set_share_mode: usize,
    _get_property_value: usize,
    _set_property_value: usize,
    // The one we need
    set_default_endpoint: SetDefaultEndpointFn,
  }

  #[repr(C)]
  struct IPolicyConfig {
    vtable: *const IPolicyConfigVtbl,
  }

  #[link(name = "ole32")]
  unsafe extern "system" {
    fn CoCreateInstance(
      rclsid: *const windows::core::GUID,
      punkouter: *mut core::ffi::c_void,
      dwclsctx: u32,
      riid: *const windows::core::GUID,
      ppv: *mut *mut core::ffi::c_void,
    ) -> windows::core::HRESULT;
  }

  let mut obj = core::ptr::null_mut();
  let hr = unsafe { CoCreateInstance(&clsid, core::ptr::null_mut(), CLSCTX_ALL.0, &iid, &mut obj) };
  if hr.is_err() {
    anyhow::bail!("CoCreateInstance for IPolicyConfig failed: {:?}", hr);
  }
  if obj.is_null() {
    anyhow::bail!("IPolicyConfig is null");
  }

  let policy = unsafe { &*(obj as *const IPolicyConfig) };
  let vtbl = unsafe { &*policy.vtable };
  let device_id_w: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();

  // Set all 3 roles: eConsole(0), eMultimedia(1), eCommunications(2)
  for role in 0u32..3 {
    let hr = unsafe { (vtbl.set_default_endpoint)(obj, windows::core::PCWSTR(device_id_w.as_ptr()), role) };
    if hr.is_err() {
      unsafe { (vtbl.release)(obj) };
      anyhow::bail!("SetDefaultEndpoint failed for role {role}: {:?}", hr);
    }
  }

  unsafe { (vtbl.release)(obj) };
  Ok(())
}

pub fn get_system_info() -> Result<SystemInfo> {
  let locale_name = get_system_locale_name().unwrap_or_default();
  let ui_language = get_user_ui_language().unwrap_or_else(|_| "en-US".to_string());
  let (major, minor, build, platform, version_string) = get_system_version().unwrap_or((0, 0, 0, 0, String::new()));
  Ok(SystemInfo {
    locale_name,
    ui_language,
    major_version: major,
    minor_version: minor,
    build_number: build,
    platform_id: platform,
    version_string,
    gpus: enumerate_gpus(),
    audio_devices: enumerate_audio_devices(),
    wifi_networks: enumerate_wifi_networks(),
    startup_entries: enumerate_startup_entries(),
    printers: list_printers().unwrap_or_default(),
    usbs: enumerate_usb_devices(),
    ..Default::default()
  })
}

pub fn get_screen_size() -> Result<(i32, i32)> {
  use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
  unsafe {
    let w = GetSystemMetrics(SM_CXSCREEN);
    let h = GetSystemMetrics(SM_CYSCREEN);
    if w <= 0 || h <= 0 {
      return Err(anyhow::anyhow!("Failed to get screen dimensions"));
    }
    Ok((w, h))
  }
}

pub fn get_installed_software() -> Result<Vec<SoftwareInfo>> {
  use windows::Win32::Foundation::*;
  use windows::Win32::System::Registry::*;

  let mut software = Vec::new();

  let keys = [
    (
      HKEY_LOCAL_MACHINE,
      r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
    (
      HKEY_LOCAL_MACHINE,
      r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
    (
      HKEY_CURRENT_USER,
      r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
  ];

  for (root, subkey) in &keys {
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = HKEY::default();

    let status = unsafe {
      RegOpenKeyExW(
        *root,
        windows::core::PCWSTR(subkey_w.as_ptr()),
        Some(0),
        KEY_READ,
        &mut hkey,
      )
    };

    if status != ERROR_SUCCESS {
      continue;
    }

    let mut index = 0u32;
    loop {
      let mut name_buf = [0u16; 256];
      let mut name_len = name_buf.len() as u32;

      let status = unsafe {
        RegEnumKeyExW(
          hkey,
          index,
          Some(windows::core::PWSTR(name_buf.as_mut_ptr())),
          &mut name_len,
          None,
          Some(windows::core::PWSTR::null()),
          None,
          None,
        )
      };

      if status != ERROR_SUCCESS {
        break;
      }

      let subkey_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);

      if let Some(info) = read_uninstall_entry(hkey, &subkey_name) {
        if !info.name.is_empty() {
          software.push(info);
        }
      }

      index += 1;
    }

    unsafe {
      let _ = RegCloseKey(hkey);
    };
  }

  software.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
  Ok(software)
}

fn read_uninstall_entry(parent: windows::Win32::System::Registry::HKEY, subkey_name: &str) -> Option<SoftwareInfo> {
  use windows::Win32::Foundation::*;
  use windows::Win32::System::Registry::*;

  let subkey_w: Vec<u16> = subkey_name.encode_utf16().chain(std::iter::once(0)).collect();
  let mut hkey = HKEY::default();

  let status = unsafe {
    RegOpenKeyExW(
      parent,
      windows::core::PCWSTR(subkey_w.as_ptr()),
      Some(0),
      KEY_READ,
      &mut hkey,
    )
  };

  if status != ERROR_SUCCESS {
    return None;
  }

  let name = read_reg_string(hkey, "DisplayName");
  let info = SoftwareInfo {
    name: name.unwrap_or_default(),
    version: read_reg_string(hkey, "DisplayVersion"),
    publisher: read_reg_string(hkey, "Publisher"),
    install_location: read_reg_string(hkey, "InstallLocation"),
    uninstall_string: read_reg_string(hkey, "UninstallString"),
    install_date: read_reg_string(hkey, "InstallDate"),
    estimated_size_kb: read_reg_dword(hkey, "EstimatedSize"),
  };

  unsafe {
    let _ = RegCloseKey(hkey);
  };
  Some(info)
}

fn read_reg_string(hkey: windows::Win32::System::Registry::HKEY, value_name: &str) -> Option<String> {
  use windows::Win32::Foundation::*;
  use windows::Win32::System::Registry::*;

  let name_w: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
  let mut buf = [0u16; 512];
  let mut buf_size = (buf.len() * 2) as u32;
  let mut data_type = REG_SZ;

  let status = unsafe {
    RegQueryValueExW(
      hkey,
      windows::core::PCWSTR(name_w.as_ptr()),
      None,
      Some(&mut data_type),
      Some(buf.as_mut_ptr() as *mut u8),
      Some(&mut buf_size),
    )
  };

  if status != ERROR_SUCCESS || data_type != REG_SZ {
    return None;
  }

  let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
  let s = String::from_utf16_lossy(&buf[..len]);
  if s.is_empty() { None } else { Some(s) }
}

fn read_reg_dword(hkey: windows::Win32::System::Registry::HKEY, value_name: &str) -> Option<u32> {
  use windows::Win32::Foundation::*;
  use windows::Win32::System::Registry::*;

  let name_w: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
  let mut value = 0u32;
  let mut buf_size = 4u32;
  let mut data_type = REG_DWORD;

  let status = unsafe {
    RegQueryValueExW(
      hkey,
      windows::core::PCWSTR(name_w.as_ptr()),
      None,
      Some(&mut data_type),
      Some(&mut value as *mut u32 as *mut u8),
      Some(&mut buf_size),
    )
  };

  if status != ERROR_SUCCESS || data_type != REG_DWORD {
    return None;
  }

  Some(value)
}

pub fn list_printers() -> Result<Vec<PrinterInfo>> {
  use windows::Win32::Graphics::Printing::*;

  let mut printers = Vec::new();

  unsafe {
    let mut needed = 0u32;
    let mut returned = 0u32;
    let _ = EnumPrintersW(
      PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS,
      None,
      2,
      None,
      &mut needed,
      &mut returned,
    );

    if needed == 0 {
      return Ok(printers);
    }

    let mut buffer = vec![0u8; needed as usize];
    let result = EnumPrintersW(
      PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS,
      None,
      2,
      Some(buffer.as_mut_slice()),
      &mut needed,
      &mut returned,
    );

    if result.is_err() {
      return Ok(printers);
    }

    let info_ptr = buffer.as_ptr() as *const PRINTER_INFO_2W;
    for i in 0..returned as usize {
      let info = &*info_ptr.add(i);
      let name = info.pPrinterName.to_string().unwrap_or_default();
      let driver = info.pDriverName.to_string().unwrap_or_default();
      let port = info.pPortName.to_string().unwrap_or_default();
      let is_default = info.Attributes & PRINTER_ATTRIBUTE_DEFAULT != 0;
      let is_shared = info.Attributes & PRINTER_ATTRIBUTE_SHARED != 0;

      printers.push(PrinterInfo {
        name,
        driver,
        port,
        is_default,
        is_shared,
      });
    }
  }

  Ok(printers)
}

pub fn print_document(file_path: &str, printer_name: &str) -> Result<()> {
  use windows::Win32::Graphics::Printing::*;
  use windows::core::{HSTRING, PWSTR};

  let path = std::path::Path::new(file_path)
    .canonicalize()
    .context("invalid file path")?;
  let file_name = path
    .file_name()
    .and_then(|n| n.to_str())
    .unwrap_or("document")
    .to_string();

  let content = std::fs::read(&path).with_context(|| format!("cannot read file: {}", path.display()))?;

  let printer_w = HSTRING::from(printer_name);
  let file_name_w = HSTRING::from(file_name.as_str());

  unsafe {
    let mut h_printer = Default::default();
    let result = OpenPrinterW(PWSTR(printer_w.as_ptr() as *mut _), &mut h_printer, None);
    if result.is_err() {
      anyhow::bail!("cannot open printer: {printer_name}");
    }

    let doc_info = DOC_INFO_1W {
      pDocName: PWSTR(file_name_w.as_ptr() as *mut _),
      pOutputFile: PWSTR::null(),
      pDatatype: PWSTR(windows::core::w!("RAW").as_ptr() as *mut _),
    };

    let doc_id = StartDocPrinterW(h_printer, 1, &doc_info);
    if doc_id == 0 {
      let _ = ClosePrinter(h_printer);
      anyhow::bail!("StartDocPrinter failed");
    }

    let mut bytes_written = 0u32;
    let ok = WritePrinter(
      h_printer,
      content.as_ptr() as *const _,
      content.len() as u32,
      &mut bytes_written,
    );

    let _ = EndDocPrinter(h_printer);
    let _ = ClosePrinter(h_printer);

    if !ok.as_bool() {
      anyhow::bail!("WritePrinter failed");
    }
  }

  Ok(())
}
