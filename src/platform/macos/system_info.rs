use core_graphics::display::CGDisplay;

use crate::error::Result;
use crate::types::*;

// ---------------------------------------------------------------------------
// CLI helper (kept for services/printers which are hard to fully native-ify)
// ---------------------------------------------------------------------------

fn run_cmd(cmd: &str, args: &[&str]) -> Result<String> {
  let output = std::process::Command::new(cmd).args(args).output()?;
  Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ---------------------------------------------------------------------------
// sysctl (native — already uses sysctlbyname under the hood)
// ---------------------------------------------------------------------------

fn sysctl_str(key: &str) -> String {
  run_cmd("sysctl", &["-n", key]).unwrap_or_default().trim().to_string()
}

fn sysctl_u64(key: &str) -> u64 {
  sysctl_str(key).parse().unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Plist helper (replaces sw_vers)
// ---------------------------------------------------------------------------

fn read_plist_string(path: &str, key: &str) -> Option<String> {
  let content = std::fs::read_to_string(path).ok()?;
  let key_tag = format!("<key>{key}</key>");
  let pos = content.find(&key_tag)?;
  let after = &content[pos + key_tag.len()..];
  let start = after.find("<string>")?;
  let val_start = start + 8;
  let end = after[val_start..].find("</string>")?;
  Some(after[val_start..val_start + end].to_string())
}

// ---------------------------------------------------------------------------
// CF type helpers (used by IOKit dict access and audio)
// ---------------------------------------------------------------------------

use core_foundation_sys::base::{CFRelease, CFTypeRef};
use core_foundation_sys::string::{CFStringCreateWithCString, CFStringGetCString, CFStringRef, kCFStringEncodingUTF8};

/// Read a global preference via `defaults` command.
/// Note: CFPreferencesCopyAppValue can throw ObjC exceptions in edge cases,
/// so we use the `defaults` CLI which is a thin wrapper and always safe.
fn defaults_read_global(key: &str) -> Option<String> {
  let output = std::process::Command::new("defaults")
    .args(["read", "-g", key])
    .output()
    .ok()?;
  if !output.status.success() {
    return None;
  }
  let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
  if val.is_empty() { None } else { Some(val) }
}

// ---------------------------------------------------------------------------
// libc FFI helpers
// ---------------------------------------------------------------------------

/// Replaces `df -k /`
fn get_disks() -> Vec<DiskInfo> {
  unsafe {
    let mut stat: libc::statvfs = std::mem::zeroed();
    let path = std::ffi::CString::new("/").unwrap();
    if libc::statvfs(path.as_ptr(), &mut stat) != 0 {
      return Vec::new();
    }
    let block_size = stat.f_frsize as u64;
    let total = stat.f_blocks as u64 * block_size;
    let free = stat.f_bfree as u64 * block_size;
    let available = stat.f_bavail as u64 * block_size;
    let used = total - free;
    vec![DiskInfo {
      drive: "/".to_string(),
      total_bytes: total,
      used_bytes: used,
      free_bytes: available,
    }]
  }
}

/// Replaces `ifconfig`
fn get_networks() -> Vec<NetworkInfo> {
  unsafe {
    let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
    if libc::getifaddrs(&mut ifap) != 0 {
      return Vec::new();
    }
    let mut networks: Vec<NetworkInfo> = Vec::new();
    let mut current = ifap;
    while !current.is_null() {
      let ifa = &*current;
      let name = std::ffi::CStr::from_ptr(ifa.ifa_name).to_string_lossy().to_string();
      if name != "lo0" {
        let is_up = (ifa.ifa_flags & 1) != 0; // IFF_UP = 1
        let mut mac = String::new();
        let mut ips = Vec::new();

        if !ifa.ifa_addr.is_null() {
          let addr = &*ifa.ifa_addr;
          match addr.sa_family as i32 {
            18 => {
              // AF_LINK = 18 on macOS — extract MAC from sockaddr_dl
              let dl = ifa.ifa_addr as *const libc::sockaddr_dl;
              if !dl.is_null() {
                let sdl = &*dl;
                let nlen = sdl.sdl_nlen as usize;
                let alen = sdl.sdl_alen as usize;
                if alen == 6 && nlen + 6 <= sdl.sdl_data.len() {
                  let data = &sdl.sdl_data[nlen..nlen + 6];
                  mac = format!(
                    "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                    data[0], data[1], data[2], data[3], data[4], data[5]
                  );
                }
              }
            }
            2 => {
              // AF_INET = 2 — extract IPv4
              let sa = ifa.ifa_addr as *const libc::sockaddr_in;
              if !sa.is_null() {
                let sin = &*sa;
                let ip = u32::from_be(sin.sin_addr.s_addr);
                ips.push(format!(
                  "{}.{}.{}.{}",
                  (ip >> 24) & 0xff,
                  (ip >> 16) & 0xff,
                  (ip >> 8) & 0xff,
                  ip & 0xff
                ));
              }
            }
            _ => {}
          }
        }

        if let Some(existing) = networks.iter_mut().find(|n| n.name == name) {
          existing.ip_addresses.extend(ips);
          if !mac.is_empty() {
            existing.mac_address = mac;
          }
          if is_up {
            existing.is_up = true;
          }
        } else {
          networks.push(NetworkInfo {
            name,
            is_up,
            mac_address: mac,
            ip_addresses: ips,
          });
        }
      }
      current = ifa.ifa_next;
    }
    libc::freeifaddrs(ifap);
    networks
  }
}

/// Replaces `memory_pressure -Q` using Mach host_statistics64
#[allow(deprecated)]
fn get_memory_pressure() -> u32 {
  unsafe {
    let mut vm_stat: libc::vm_statistics64 = std::mem::zeroed();
    let mut count = (std::mem::size_of::<libc::vm_statistics64>() / std::mem::size_of::<u32>()) as u32;
    let port = libc::mach_host_self();
    let kr = libc::host_statistics64(port, libc::HOST_VM_INFO64, &mut vm_stat as *mut _ as *mut libc::integer_t, &mut count);
    if kr != 0 {
      return 50;
    }
    let page_size = 4096u64;
    let free = vm_stat.free_count as u64 * page_size;
    let speculative = vm_stat.speculative_count as u64 * page_size;
    let total = sysctl_u64("hw.memsize");
    if total == 0 {
      return 50;
    }
    let available = free + speculative;
    let usage = ((total - available) * 100 / total) as u32;
    usage.min(100)
  }
}

// ---------------------------------------------------------------------------
// IOKit helpers (replaces system_profiler, pmset)
// ---------------------------------------------------------------------------

use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
use core_foundation_sys::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
use core_foundation_sys::number::CFNumberGetValue;
use core_foundation_sys::string::CFStringGetTypeID;
use io_kit_sys::types::io_iterator_t;
use io_kit_sys::{IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperties, IOServiceGetMatchingServices, IOServiceMatching};

// KIO_MAIN_PORT_DEFAULT is deprecated since macOS 12, replaced by kIOMainPortDefault.
// io_kit_sys crate hasn't updated yet; define locally (same value: MACH_PORT_NULL).
const KIO_MAIN_PORT_DEFAULT: u32 = 0;

const KERN_SUCCESS: i32 = 0;

/// Read a C-string value from a CFDictionary by key name.
fn dict_get_string(dict: CFDictionaryRef, key_name: &str) -> Option<String> {
  let c_key = std::ffi::CString::new(key_name).ok()?;
  unsafe {
    let cf_key = CFStringCreateWithCString(std::ptr::null(), c_key.as_ptr(), kCFStringEncodingUTF8);
    if cf_key.is_null() {
      return None;
    }
    let val = CFDictionaryGetValue(dict, cf_key as *const _);
    CFRelease(cf_key as CFTypeRef);
    if val.is_null() {
      return None;
    }
    let type_id = core_foundation_sys::base::CFGetTypeID(val);
    if type_id == CFStringGetTypeID() {
      let mut buf = [0u8; 512];
      let ok = CFStringGetCString(val as CFStringRef, buf.as_mut_ptr().cast(), buf.len() as _, kCFStringEncodingUTF8);
      if ok != 0 {
        let cstr = std::ffi::CStr::from_ptr(buf.as_ptr().cast());
        return Some(cstr.to_string_lossy().into_owned());
      }
    }
    None
  }
}

/// Read a Data/Number value from a CFDictionary that might be a raw byte count.
fn dict_get_data_u64(dict: CFDictionaryRef, key_name: &str) -> Option<u64> {
  let c_key = std::ffi::CString::new(key_name).ok()?;
  unsafe {
    let cf_key = CFStringCreateWithCString(std::ptr::null(), c_key.as_ptr(), kCFStringEncodingUTF8);
    if cf_key.is_null() {
      return None;
    }
    let val = CFDictionaryGetValue(dict, cf_key as *const _);
    CFRelease(cf_key as CFTypeRef);
    if val.is_null() {
      return None;
    }
    let type_id = core_foundation_sys::base::CFGetTypeID(val);
    // Try as CFNumber first
    if type_id == core_foundation_sys::number::CFNumberGetTypeID() {
      let mut result: i64 = 0;
      let ok = CFNumberGetValue(val as _, core_foundation_sys::number::kCFNumberSInt64Type as _, &mut result as *mut i64 as *mut _);
      if ok && result >= 0 {
        return Some(result as u64);
      }
    }
    // Try as CFData
    if type_id == core_foundation_sys::data::CFDataGetTypeID() {
      let data = val as core_foundation_sys::data::CFDataRef;
      let len = core_foundation_sys::data::CFDataGetLength(data);
      if len > 0 && len <= 8 {
        let mut bytes = [0u8; 8];
        core_foundation_sys::data::CFDataGetBytes(data, core_foundation_sys::base::CFRange { location: 0, length: len }, bytes.as_mut_ptr());
        // Read as big-endian
        let mut v: u64 = 0;
        for i in 0..len as usize {
          v = (v << 8) | bytes[i] as u64;
        }
        return Some(v);
      }
    }
    None
  }
}

/// Replaces `pmset -g batt` with IOKit Power Sources
fn get_battery() -> Option<BatteryInfo> {
  unsafe {
    use io_kit_sys::ps::power_sources::{IOPSCopyPowerSourcesInfo, IOPSCopyPowerSourcesList, IOPSGetPowerSourceDescription};

    let info = IOPSCopyPowerSourcesInfo();
    if info.is_null() {
      return None;
    }
    let sources = IOPSCopyPowerSourcesList(info);
    if sources.is_null() {
      CFRelease(info);
      return None;
    }

    let mut battery_percent = 0u32;
    let mut ac_power = false;
    let mut time_remaining = 0u64;

    let count = CFArrayGetCount(sources);
    for i in 0..count {
      let ps = CFArrayGetValueAtIndex(sources, i);
      let desc = IOPSGetPowerSourceDescription(info, ps);
      if desc.is_null() {
        continue;
      }

      if let Some(state) = dict_get_string(desc, "Power Source State") {
        ac_power = state == "AC Power";
      }
      if let Some(pct) = dict_get_i64(desc, "Current Capacity") {
        battery_percent = pct as u32;
      }
      if let Some(time) = dict_get_i64(desc, "Time to Empty") {
        if time > 0 {
          time_remaining = time as u64 * 60;
        }
      }
    }

    CFRelease(sources as CFTypeRef);
    CFRelease(info);

    Some(BatteryInfo {
      ac_power,
      battery_percent,
      battery_life_seconds: time_remaining,
    })
  }
}

/// Read an i64 from a CFDictionary value, checking type first to avoid ObjC exceptions.
fn dict_get_i64_checked(val: *const std::ffi::c_void) -> Option<i64> {
  if val.is_null() {
    return None;
  }
  unsafe {
    let type_id = core_foundation_sys::base::CFGetTypeID(val);
    if type_id != core_foundation_sys::number::CFNumberGetTypeID() {
      return None;
    }
    let mut result: i64 = 0;
    let ok = CFNumberGetValue(val as _, core_foundation_sys::number::kCFNumberSInt64Type as _, &mut result as *mut i64 as *mut _);
    if ok { Some(result) } else { None }
  }
}

/// Read an integer value from a CFDictionary by key name (type-safe).
fn dict_get_i64(dict: CFDictionaryRef, key_name: &str) -> Option<i64> {
  let c_key = std::ffi::CString::new(key_name).ok()?;
  unsafe {
    let cf_key = CFStringCreateWithCString(std::ptr::null(), c_key.as_ptr(), kCFStringEncodingUTF8);
    if cf_key.is_null() {
      return None;
    }
    let val = CFDictionaryGetValue(dict, cf_key as *const _);
    CFRelease(cf_key as CFTypeRef);
    dict_get_i64_checked(val)
  }
}

/// Replaces `system_profiler SPDisplaysDataType` with IOKit registry query
fn get_gpus() -> Vec<GpuInfo> {
  unsafe {
    let matching = IOServiceMatching(b"IOPCIDevice\0".as_ptr() as *const _);
    if matching.is_null() {
      return Vec::new();
    }

    let mut iterator: io_iterator_t = 0;
    let kr = IOServiceGetMatchingServices(KIO_MAIN_PORT_DEFAULT, matching, &mut iterator);
    if kr != KERN_SUCCESS {
      return Vec::new();
    }

    let mut gpus = Vec::new();
    loop {
      let service = IOIteratorNext(iterator);
      if service == 0 {
        break;
      }

      let mut props: core_foundation_sys::dictionary::CFMutableDictionaryRef = std::ptr::null_mut();
      let kr = IORegistryEntryCreateCFProperties(service, &mut props, core_foundation_sys::base::kCFAllocatorDefault, 0);
      if kr != KERN_SUCCESS || props.is_null() {
        IOObjectRelease(service);
        continue;
      }

      // Check class-code: display controllers have base class 0x03
      // class-code is CFData (4 bytes), so use dict_get_data_u64
      let is_display = if let Some(class_code) = dict_get_data_u64(props, "class-code") {
        ((class_code >> 16) & 0xff) == 0x03
      } else {
        false
      };

      if !is_display {
        CFRelease(props as CFTypeRef);
        IOObjectRelease(service);
        continue;
      }

      let name = dict_get_string(props, "model")
        .or_else(|| dict_get_string(props, "AAPL,slot-name"))
        .unwrap_or_else(|| "Unknown GPU".to_string());

      let vram = dict_get_data_u64(props, "VRAM,totalsize")
        .or_else(|| dict_get_i64(props, "VRAM,totalsize").map(|v| v as u64))
        .unwrap_or(0);

      let vendor_id = dict_get_i64(props, "vendor-id").unwrap_or(0) as u32;
      let device_id = dict_get_i64(props, "device-id").unwrap_or(0) as u32;

      gpus.push(GpuInfo {
        name,
        driver_version: None,
        provider_name: None,
        driver_date: None,
        vendor_id,
        device_id,
        dedicated_video_memory: vram,
        shared_system_memory: 0,
        vram_bytes: vram,
        is_software: false,
        is_remote: false,
      });

      CFRelease(props as CFTypeRef);
      IOObjectRelease(service);
    }
    IOObjectRelease(iterator);
    gpus
  }
}

/// Replaces `system_profiler SPUSBDataType` with IOKit registry query
fn get_usb_devices_io_kit() -> Vec<UsbDeviceInfo> {
  unsafe {
    let matching = IOServiceMatching(b"IOUSBDevice\0".as_ptr() as *const _);
    if matching.is_null() {
      return Vec::new();
    }

    let mut iterator: io_iterator_t = 0;
    let kr = IOServiceGetMatchingServices(KIO_MAIN_PORT_DEFAULT, matching, &mut iterator);
    if kr != KERN_SUCCESS {
      return Vec::new();
    }

    let mut devices = Vec::new();
    loop {
      let service = IOIteratorNext(iterator);
      if service == 0 {
        break;
      }

      let mut props: core_foundation_sys::dictionary::CFMutableDictionaryRef = std::ptr::null_mut();
      let kr = IORegistryEntryCreateCFProperties(service, &mut props, core_foundation_sys::base::kCFAllocatorDefault, 0);
      if kr != KERN_SUCCESS || props.is_null() {
        IOObjectRelease(service);
        continue;
      }

      let name = dict_get_string(props, "USB Product Name")
        .or_else(|| dict_get_string(props, "kUSBString"))
        .unwrap_or_else(|| "Unknown USB Device".to_string());

      let manufacturer = dict_get_string(props, "USB Vendor Name")
        .or_else(|| dict_get_string(props, "kUSBVendorString"))
        .unwrap_or_default();

      let vid = dict_get_i64(props, "idVendor")
        .map(|v| format!("0x{v:04x}"))
        .unwrap_or_default();

      let pid = dict_get_i64(props, "idProduct")
        .map(|v| format!("0x{v:04x}"))
        .unwrap_or_default();

      let serial = dict_get_string(props, "USB Serial Number")
        .or_else(|| dict_get_string(props, "kUSBSerialNumberString"))
        .unwrap_or_default();

      if !vid.is_empty() || !pid.is_empty() {
        devices.push(UsbDeviceInfo {
          name,
          description: String::new(),
          manufacturer,
          vid,
          pid,
          serial_number: serial,
        });
      }

      CFRelease(props as CFTypeRef);
      IOObjectRelease(service);
    }
    IOObjectRelease(iterator);
    devices
  }
}

// ---------------------------------------------------------------------------
// Audio device enumeration via CoreAudio (replaces stub)
// ---------------------------------------------------------------------------

use std::ffi::c_void;

type OSStatus = i32;
type AudioObjectID = u32;

#[repr(C)]
#[allow(non_snake_case)]
struct AudioObjectPropertyAddress {
  mSelector: u32,
  mScope: u32,
  mElement: u32,
}

const K_AUDIO_OBJECT_SYSTEM_OBJECT: u32 = 1;
const K_AUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE: u32 = 0x646F7574; // 'dout'
const K_AUDIO_HARDWARE_PROPERTY_DEVICES: u32 = 0x64657623; // 'dev#'
const K_AUDIO_DEVICE_PROPERTY_DEVICE_NAME_CFSTRING: u32 = 0x6C6E616D; // 'lnam'
const K_AUDIO_DEVICE_PROPERTY_STREAM_CONFIGURATION: u32 = 0x736C6179; // 'slay'
const K_AUDIO_DEVICE_PROPERTY_VOLUME_SCALAR: u32 = 0x766F6C6D; // 'volm'
const K_AUDIO_DEVICE_PROPERTY_MUTE: u32 = 0x6D757465; // 'mute'
const K_AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT: u32 = 0x6F757470; // 'outp'
const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = 0x676C6F62; // 'glob'
const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
  fn AudioObjectGetPropertyData(
    objectID: AudioObjectID,
    address: *const AudioObjectPropertyAddress,
    qualifierDataSize: u32,
    qualifierData: *const c_void,
    dataSize: *mut u32,
    data: *mut c_void,
  ) -> OSStatus;
  fn AudioObjectSetPropertyData(
    objectID: AudioObjectID,
    address: *const AudioObjectPropertyAddress,
    qualifierDataSize: u32,
    qualifierData: *const c_void,
    dataSize: u32,
    data: *const c_void,
  ) -> OSStatus;
  fn AudioObjectGetPropertyDataSize(
    objectID: AudioObjectID,
    address: *const AudioObjectPropertyAddress,
    qualifierDataSize: u32,
    qualifierData: *const c_void,
    outDataSize: *mut u32,
  ) -> OSStatus;
}

fn default_output_device() -> Result<u32> {
  let address = AudioObjectPropertyAddress {
    mSelector: K_AUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
    mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
  };
  let mut device_id: u32 = 0;
  let mut data_size = std::mem::size_of::<u32>() as u32;
  let status = unsafe {
    AudioObjectGetPropertyData(
      K_AUDIO_OBJECT_SYSTEM_OBJECT,
      &address,
      0,
      std::ptr::null(),
      &mut data_size,
      &mut device_id as *mut u32 as *mut c_void,
    )
  };
  if status != 0 {
    return Err(anyhow::anyhow!("failed to get default output device: OSStatus {status}"));
  }
  Ok(device_id)
}

/// Enumerate audio output devices via CoreAudio
fn get_audio_devices() -> Vec<AudioDeviceInfo> {
  let mut devices = Vec::new();

  // Get list of all audio devices
  let address = AudioObjectPropertyAddress {
    mSelector: K_AUDIO_HARDWARE_PROPERTY_DEVICES,
    mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
  };

  let mut data_size: u32 = 0;
  let status = unsafe {
    AudioObjectGetPropertyDataSize(K_AUDIO_OBJECT_SYSTEM_OBJECT, &address, 0, std::ptr::null(), &mut data_size)
  };
  if status != 0 || data_size == 0 {
    return devices;
  }

  let num_devices = data_size as usize / std::mem::size_of::<u32>();
  let mut device_ids = vec![0u32; num_devices];
  let status = unsafe {
    AudioObjectGetPropertyData(
      K_AUDIO_OBJECT_SYSTEM_OBJECT,
      &address,
      0,
      std::ptr::null(),
      &mut data_size,
      device_ids.as_mut_ptr() as *mut c_void,
    )
  };
  if status != 0 {
    return devices;
  }

  let default_id = default_output_device().unwrap_or(0);

  for &dev_id in &device_ids {
    // Check if device has output channels
    let stream_addr = AudioObjectPropertyAddress {
      mSelector: K_AUDIO_DEVICE_PROPERTY_STREAM_CONFIGURATION,
      mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT,
      mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut stream_size: u32 = 0;
    let st = unsafe {
      AudioObjectGetPropertyDataSize(dev_id, &stream_addr, 0, std::ptr::null(), &mut stream_size)
    };
    if st != 0 || stream_size == 0 {
      continue; // No output channels — skip
    }

    // Get device name
    let name_addr = AudioObjectPropertyAddress {
      mSelector: K_AUDIO_DEVICE_PROPERTY_DEVICE_NAME_CFSTRING,
      mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
      mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut cf_str: CFStringRef = std::ptr::null();
    let mut name_size = std::mem::size_of::<CFStringRef>() as u32;
    let st = unsafe {
      AudioObjectGetPropertyData(
        dev_id,
        &name_addr,
        0,
        std::ptr::null(),
        &mut name_size,
        &mut cf_str as *mut CFStringRef as *mut c_void,
      )
    };
    let name = if st == 0 && !cf_str.is_null() {
      let mut cbuf = [0u8; 256];
      let ok = unsafe { CFStringGetCString(cf_str, cbuf.as_mut_ptr().cast(), cbuf.len() as _, kCFStringEncodingUTF8) };
      if ok != 0 {
        unsafe { std::ffi::CStr::from_ptr(cbuf.as_ptr().cast()) }.to_string_lossy().into_owned()
      } else {
        format!("Device {dev_id}")
      }
    } else {
      format!("Device {dev_id}")
    };

    // Get volume
    let vol_addr = AudioObjectPropertyAddress {
      mSelector: K_AUDIO_DEVICE_PROPERTY_VOLUME_SCALAR,
      mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
      mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut vol: f32 = 0.0;
    let mut vol_size = std::mem::size_of::<f32>() as u32;
    let _ = unsafe {
      AudioObjectGetPropertyData(dev_id, &vol_addr, 0, std::ptr::null(), &mut vol_size, &mut vol as *mut f32 as *mut c_void)
    };

    // Get mute
    let mute_addr = AudioObjectPropertyAddress {
      mSelector: K_AUDIO_DEVICE_PROPERTY_MUTE,
      mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
      mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut muted: u32 = 0;
    let mut mute_size = std::mem::size_of::<u32>() as u32;
    let _ = unsafe {
      AudioObjectGetPropertyData(dev_id, &mute_addr, 0, std::ptr::null(), &mut mute_size, &mut muted as *mut u32 as *mut c_void)
    };

    devices.push(AudioDeviceInfo {
      name,
      device_id: dev_id.to_string(),
      device_type: "output".to_string(),
      is_default: dev_id == default_id,
      volume: (vol * 100.0) as u32,
      muted: muted != 0,
    });
  }

  devices
}

// ---------------------------------------------------------------------------
// Startup entries (replaces stub)
// ---------------------------------------------------------------------------

fn get_startup_entries() -> Vec<StartupEntry> {
  let mut entries = Vec::new();
  let dirs = [
    "/Library/LaunchAgents",
    "/Library/LaunchDaemons",
  ];
  let home_dir = std::env::var("HOME").unwrap_or_default();
  let home_agents = format!("{home_dir}/Library/LaunchAgents");

  for dir in dirs.iter().chain(std::iter::once(&home_agents.as_str())) {
    let Ok(read_dir) = std::fs::read_dir(dir) else { continue };
    for entry in read_dir.flatten() {
      let path = entry.path();
      if path.extension().and_then(|e| e.to_str()) != Some("plist") {
        continue;
      }
      let Some(label) = read_plist_string(&path.to_string_lossy(), "Label") else {
        continue;
      };
      // Extract ProgramArguments to get the command
      let command = read_plist_string(&path.to_string_lossy(), "Program")
        .unwrap_or_else(|| label.clone());
      entries.push(StartupEntry {
        name: label,
        command,
        location: path.to_string_lossy().into_owned(),
      });
    }
  }
  entries
}

// ---------------------------------------------------------------------------
// Installed software (replaces system_profiler SPApplicationsDataType)
// ---------------------------------------------------------------------------

fn scan_app_dir(dir: &str, software: &mut Vec<SoftwareInfo>) {
  let Ok(entries) = std::fs::read_dir(dir) else { return };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.extension().and_then(|e| e.to_str()) == Some("app") {
      let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
      if name.is_empty() {
        continue;
      }
      let plist_path = path.join("Contents/Info.plist");
      let version = read_plist_string(&plist_path.to_string_lossy(), "CFBundleShortVersionString");
      software.push(SoftwareInfo {
        name,
        version,
        publisher: None,
        install_location: Some(path.to_string_lossy().into_owned()),
        uninstall_string: None,
        install_date: None,
        estimated_size_kb: None,
      });
    }
  }
}

pub fn get_installed_software() -> Result<Vec<SoftwareInfo>> {
  let mut software = Vec::new();
  scan_app_dir("/Applications", &mut software);
  let home_apps = format!("{}/Applications", std::env::var("HOME").unwrap_or_default());
  scan_app_dir(&home_apps, &mut software);
  scan_app_dir("/System/Applications", &mut software);
  software.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
  Ok(software)
}

// ---------------------------------------------------------------------------
// Screen (native CGDisplay — already done)
// ---------------------------------------------------------------------------

pub fn get_screen_size() -> Result<(i32, i32)> {
  let display = CGDisplay::main();
  let w = display.pixels_wide() as i32;
  let h = display.pixels_high() as i32;
  if w > 0 && h > 0 {
    Ok((w, h))
  } else {
    Err(anyhow::anyhow!("could not determine screen size from CGDisplay"))
  }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub fn get_system_info() -> Result<SystemInfo> {
  let computer_name = sysctl_str("kern.hostname");
  let username = std::env::var("USER").unwrap_or_default();

  // OS version from SystemVersion.plist (replaces sw_vers)
  let os_version = read_plist_string("/System/Library/CoreServices/SystemVersion.plist", "ProductVersion")
    .unwrap_or_default();
  let build_str = read_plist_string("/System/Library/CoreServices/SystemVersion.plist", "ProductBuildVersion")
    .unwrap_or_default();
  let version_string = format!("macOS {os_version} ({build_str})");

  // Locale
  let locale = defaults_read_global("AppleLocale").unwrap_or_default();

  // CPU
  let cpu_name = sysctl_str("machdep.cpu.brand_string");
  let physical_cores = sysctl_u64("hw.physicalcpu") as u32;
  let logical_cores = sysctl_u64("hw.logicalcpu") as u32;

  // Memory
  let total_mem = sysctl_u64("hw.memsize");
  let mem_pressure = get_memory_pressure();
  let available_mem = total_mem * (100 - mem_pressure as u64) / 100;
  let used_mem = total_mem - available_mem;

  // Screen
  let (screen_w, screen_h) = get_screen_size().unwrap_or((0, 0));

  // Disks
  let disks = get_disks();

  // Network
  let networks = get_networks();

  // Battery
  let battery = get_battery();

  // GPU
  let gpus = get_gpus();

  // UI language
  let ui_language = defaults_read_global("AppleLanguages")
    .map(|lang| {
      lang
        .lines()
        .next()
        .unwrap_or("en-US")
        .trim_matches(|c| c == '"' || c == '(' || c == ')' || c == ' ')
        .to_string()
    })
    .unwrap_or_else(|| "en-US".to_string());

  // Audio devices
  let audio_devices = get_audio_devices();

  // Startup entries
  let startup_entries = get_startup_entries();

  // Printers
  let printers = list_printers().unwrap_or_default();

  // USB devices
  let usbs = get_usb_devices_io_kit();

  Ok(SystemInfo {
    computer_name,
    username,
    os_version,
    locale_name: locale.clone(),
    locale,
    ui_language,
    major_version: 0,
    minor_version: 0,
    build_number: 0,
    platform_id: 0,
    version_string,
    screen_width: screen_w,
    screen_height: screen_h,
    cpu: Some(CpuInfo {
      name: cpu_name,
      cores: physical_cores,
      logical_processors: logical_cores,
    }),
    memory: Some(MemoryInfo {
      total_bytes: total_mem,
      available_bytes: available_mem,
      used_bytes: used_mem,
      usage_percent: mem_pressure,
    }),
    disks,
    networks,
    battery,
    gpus,
    audio_devices,
    wifi_networks: get_wifi_networks(),
    startup_entries,
    printers,
    usbs,
    bluetooth_devices: Vec::new(),
    services: Vec::new(),
  })
}

// ---------------------------------------------------------------------------
// Printers (uses lpstat — no native API equivalent readily available)
// ---------------------------------------------------------------------------

pub fn list_printers() -> Result<Vec<PrinterInfo>> {
  let output = run_cmd("lpstat", &["-p", "-d"])?;
  let mut printers = Vec::new();
  let mut default_printer = String::new();
  for line in output.lines() {
    let line = line.trim();
    if line.starts_with("printer ") {
      let name = line.split_whitespace().nth(1).unwrap_or("").to_string();
      printers.push(PrinterInfo {
        name,
        driver: String::new(),
        port: String::new(),
        is_default: false,
        is_shared: false,
      });
    } else if line.contains("system default destination:") {
      default_printer = line.split(':').last().unwrap_or("").trim().to_string();
    }
  }
  for p in &mut printers {
    if p.name == default_printer {
      p.is_default = true;
    }
  }
  Ok(printers)
}

pub fn print_document(file_path: &str, printer_name: &str) -> Result<()> {
  let status = std::process::Command::new("lpr")
    .args(["-P", printer_name, file_path])
    .status()?;
  if status.success() {
    Ok(())
  } else {
    Err(anyhow::anyhow!("print failed"))
  }
}

// ---------------------------------------------------------------------------
// Bluetooth (kept as CLI — IOBluetooth API is Objective-C heavy)
// ---------------------------------------------------------------------------

/// List paired Bluetooth devices via IOKit (replaces system_profiler SPBluetoothDataType).
pub fn list_bluetooth_devices() -> Result<Vec<BluetoothDeviceInfo>> {
  unsafe {
    let matching = IOServiceMatching(b"IOBluetoothDevice\0".as_ptr() as *const _);
    if matching.is_null() {
      return Ok(Vec::new());
    }
    let mut iterator: u32 = 0;
    let kr = IOServiceGetMatchingServices(KIO_MAIN_PORT_DEFAULT, matching, &mut iterator);
    if kr != 0 {
      return Ok(Vec::new());
    }
    let mut devices = Vec::new();
    loop {
      let service = IOIteratorNext(iterator);
      if service == 0 {
        break;
      }
      let mut props: core_foundation_sys::dictionary::CFMutableDictionaryRef = std::ptr::null_mut();
      let kr = IORegistryEntryCreateCFProperties(
        service,
        &mut props,
        core_foundation_sys::base::kCFAllocatorDefault,
        0,
      );
      if kr == 0 && !props.is_null() {
        let dict = props as core_foundation_sys::dictionary::CFDictionaryRef;
        let name = dict_get_string(dict, "Name")
          .or_else(|| dict_get_string(dict, "DefaultName"))
          .unwrap_or_else(|| "Unknown".to_string());
        let addr = dict_get_string(dict, "DeviceAddress")
          .or_else(|| dict_get_string(dict, "BD_ADDR"))
          .unwrap_or_default();
        let is_connected = dict_get_i64(dict, "IsConnected").unwrap_or(0) != 0;
        core_foundation_sys::base::CFRelease(props as *const _);
        devices.push(BluetoothDeviceInfo {
          name,
          address: addr,
          is_connected,
          is_paired: true,
          source: "Bluetooth".to_string(),
          rssi: None,
        });
      }
      IOObjectRelease(service);
    }
    IOObjectRelease(iterator);
    Ok(devices)
  }
}

// ---------------------------------------------------------------------------
// Services (uses launchctl — no pure-native equivalent)
// ---------------------------------------------------------------------------

pub fn list_services() -> Result<Vec<ServiceInfo>> {
  let output = run_cmd("launchctl", &["list"])?;
  let mut services = Vec::new();
  for line in output.lines().skip(1) {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 3 {
      continue;
    }
    let pid_str = parts[0];
    let status_str = parts[1];
    let name = parts[2].to_string();
    let status = if pid_str == "-" {
      "stopped".to_string()
    } else if status_str == "0" {
      "running".to_string()
    } else {
      format!("error({status_str})")
    };
    services.push(ServiceInfo {
      name,
      display_name: String::new(),
      status,
      service_type: "launchd".to_string(),
    });
  }
  Ok(services)
}

pub fn get_service_detail(name: &str) -> Result<ServiceInfo> {
  let services = list_services()?;
  services
    .into_iter()
    .find(|s| s.name == name)
    .ok_or_else(|| anyhow::anyhow!("service not found: {name}"))
}

pub fn start_service(name: &str) -> Result<()> {
  let status = std::process::Command::new("launchctl").args(["start", name]).status()?;
  if status.success() {
    Ok(())
  } else {
    Err(anyhow::anyhow!("failed to start service: {name}"))
  }
}

pub fn stop_service(name: &str) -> Result<()> {
  let status = std::process::Command::new("launchctl").args(["stop", name]).status()?;
  if status.success() {
    Ok(())
  } else {
    Err(anyhow::anyhow!("failed to stop service: {name}"))
  }
}

// ---------------------------------------------------------------------------
// Audio control (existing CoreAudio FFI — unchanged)
// ---------------------------------------------------------------------------

pub fn set_volume(level: u32) -> Result<()> {
  let vol = (level.min(100) as f32) / 100.0;
  let device_id = default_output_device()?;
  let address = AudioObjectPropertyAddress {
    mSelector: K_AUDIO_DEVICE_PROPERTY_VOLUME_SCALAR,
    mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
  };
  let status = unsafe {
    AudioObjectSetPropertyData(
      device_id,
      &address,
      0,
      std::ptr::null(),
      std::mem::size_of::<f32>() as u32,
      &vol as *const f32 as *const c_void,
    )
  };
  if status != 0 {
    return Err(anyhow::anyhow!("set volume failed: OSStatus {status}"));
  }
  Ok(())
}

pub fn get_volume() -> Result<u32> {
  let device_id = default_output_device()?;
  let address = AudioObjectPropertyAddress {
    mSelector: K_AUDIO_DEVICE_PROPERTY_VOLUME_SCALAR,
    mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
  };
  let mut vol: f32 = 0.0;
  let mut data_size = std::mem::size_of::<f32>() as u32;
  let status = unsafe {
    AudioObjectGetPropertyData(
      device_id,
      &address,
      0,
      std::ptr::null(),
      &mut data_size,
      &mut vol as *mut f32 as *mut c_void,
    )
  };
  if status != 0 {
    return Err(anyhow::anyhow!("get volume failed: OSStatus {status}"));
  }
  Ok((vol * 100.0) as u32)
}

pub fn set_mute(muted: bool) -> Result<()> {
  let device_id = default_output_device()?;
  let address = AudioObjectPropertyAddress {
    mSelector: K_AUDIO_DEVICE_PROPERTY_MUTE,
    mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
  };
  let muted_val: u32 = if muted { 1 } else { 0 };
  let status = unsafe {
    AudioObjectSetPropertyData(
      device_id,
      &address,
      0,
      std::ptr::null(),
      std::mem::size_of::<u32>() as u32,
      &muted_val as *const u32 as *const c_void,
    )
  };
  if status != 0 {
    return Err(anyhow::anyhow!("set mute failed: OSStatus {status}"));
  }
  Ok(())
}

pub fn get_mute() -> Result<bool> {
  let device_id = default_output_device()?;
  let address = AudioObjectPropertyAddress {
    mSelector: K_AUDIO_DEVICE_PROPERTY_MUTE,
    mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
  };
  let mut muted: u32 = 0;
  let mut data_size = std::mem::size_of::<u32>() as u32;
  let status = unsafe {
    AudioObjectGetPropertyData(
      device_id,
      &address,
      0,
      std::ptr::null(),
      &mut data_size,
      &mut muted as *mut u32 as *mut c_void,
    )
  };
  if status != 0 {
    return Err(anyhow::anyhow!("get mute failed: OSStatus {status}"));
  }
  Ok(muted != 0)
}

pub fn set_default_device(device_id_str: &str) -> Result<()> {
  let device_id: u32 = device_id_str
    .parse()
    .map_err(|_| anyhow::anyhow!("invalid device id: {device_id_str}"))?;
  let address = AudioObjectPropertyAddress {
    mSelector: K_AUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
    mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
  };
  let status = unsafe {
    AudioObjectSetPropertyData(
      K_AUDIO_OBJECT_SYSTEM_OBJECT,
      &address,
      0,
      std::ptr::null(),
      std::mem::size_of::<u32>() as u32,
      &device_id as *const u32 as *const c_void,
    )
  };
  if status != 0 {
    return Err(anyhow::anyhow!("set default device failed: OSStatus {status}"));
  }
  Ok(())
}

// ---------------------------------------------------------------------------
// WiFi (uses networksetup + airport — no pure-native alternative easily)
// ---------------------------------------------------------------------------

pub fn get_wifi_networks() -> Vec<WifiNetworkInfo> {
  // Combine CoreWLAN scan (for RSSI) with system_profiler (for SSID)
  // macOS 15+ hides SSID from CoreWLAN without entitlements, so we need both sources.

  // Step 1: Get RSSI from CoreWLAN scan (BSSID obtained via arp for connected network)
  let corewlan_scan = get_corewlan_scan();

  // Step 2: Get SSID and security from system_profiler JSON
  let Ok(output) = std::process::Command::new("system_profiler")
    .args(["SPAirPortDataType", "-json"])
    .output()
  else {
    return Vec::new();
  };

  let mut networks = Vec::new();
  let json_str = String::from_utf8_lossy(&output.stdout);
  let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) else {
    return networks;
  };

  // Navigate to the WiFi interface data
  let Some(interfaces) = json["SPAirPortDataType"][0]["spairport_airport_interfaces"].as_array() else {
    return networks;
  };

  for iface in interfaces {
    // Get WiFi device name (e.g., "en1")
    let device_name = iface["_name"].as_str().unwrap_or("en1");

    // Get BSSID for connected network using arp table (works without Location Services)
    let connected_bssid = get_connected_bssid(device_name);

    // Connected network
    if let Some(current) = iface.get("spairport_current_network_information") {
      let ssid = current["_name"].as_str().unwrap_or("").to_string();
      let security = map_security_mode(current["spairport_security_mode"].as_str().unwrap_or(""));
      let signal = parse_signal_noise(current["spairport_signal_noise"].as_str().unwrap_or("")).unwrap_or(100);
      if !ssid.is_empty() {
        networks.push(WifiNetworkInfo {
          ssid,
          signal_quality: signal,
          bssid: connected_bssid,
          auth_type: Some(security),
          is_connected: true,
        });
      }
    }

    // Other visible networks
    if let Some(others) = iface["spairport_airport_other_local_wireless_networks"].as_array() {
      for (i, net) in others.iter().enumerate() {
        let ssid = net["_name"].as_str().unwrap_or("").to_string();
        let security = map_security_mode(net["spairport_security_mode"].as_str().unwrap_or(""));

        // Try to get signal from system_profiler first, then fall back to CoreWLAN RSSI
        let signal = parse_signal_noise(net["spairport_signal_noise"].as_str().unwrap_or(""))
          .or_else(|| corewlan_scan.get(i).map(|e| ((e.rssi + 100).max(0) as u32).min(100)))
          .unwrap_or(0);

        // Get BSSID and SSID from CoreWLAN scan (may be available with Location Services)
        let bssid = corewlan_scan.get(i).and_then(|e| e.bssid.clone());
        let ssid = corewlan_scan.get(i).and_then(|e| e.ssid.clone()).unwrap_or(ssid);

        if !ssid.is_empty() {
          networks.push(WifiNetworkInfo {
            ssid,
            signal_quality: signal,
            bssid,
            auth_type: Some(security),
            is_connected: false,
          });
        }
      }
    }
  }

  networks
}

/// CoreWLAN scan result for a single network
struct CoreWlanScanEntry {
  rssi: i64,
  bssid: Option<String>,
  ssid: Option<String>,
}

/// Get BSSID of the connected WiFi network using arp table (gateway MAC address).
/// This doesn't require Location Services authorization on macOS 14+.
fn get_connected_bssid(wifi_device: &str) -> Option<String> {
  // Get default gateway IP
  let output = std::process::Command::new("netstat").args(["-rn"]).output().ok()?;
  let stdout = String::from_utf8_lossy(&output.stdout);
  let gateway_ip = stdout.lines()
    .find(|line| line.starts_with("default") && line.contains(wifi_device))
    .and_then(|line| {
      line.split_whitespace().nth(1)
    })?;

  // Get MAC address of gateway from arp table
  let output = std::process::Command::new("arp").args(["-a", "-n"]).output().ok()?;
  let stdout = String::from_utf8_lossy(&output.stdout);
  for line in stdout.lines() {
    if line.contains(gateway_ip) && line.contains(wifi_device) {
      // Format: "? (10.0.0.1) at 0:f0:cb:ee:b9:b1 on en1 ifscope [ethernet]"
      if let Some(at_pos) = line.find(" at ") {
        let rest = &line[at_pos + 4..];
        if let Some(on_pos) = rest.find(" on ") {
          let mac = rest[..on_pos].trim();
          if mac != "(incomplete)" && !mac.is_empty() {
            return Some(mac.to_string());
          }
        }
      }
    }
  }
  None
}

/// Get RSSI, BSSID and SSID from CoreWLAN scan.
/// On macOS 14+, BSSID/SSID require Location Services authorization.
fn get_corewlan_scan() -> Vec<CoreWlanScanEntry> {
  #[cfg(target_os = "macos")]
  {
    use objc2::runtime::{AnyObject, Class};

    unsafe {
      // Try to use CWWiFiClient (modern API) for better macOS 14+ compatibility
      let Some(wifi_client_cls) = Class::get(c"CWWiFiClient") else {
        return get_corewlan_scan_legacy();
      };

      let shared_client: *mut AnyObject = objc2::msg_send![wifi_client_cls, sharedWiFiClient];
      if shared_client.is_null() {
        return get_corewlan_scan_legacy();
      }

      let interface: *mut AnyObject = objc2::msg_send![shared_client, interface];
      if interface.is_null() {
        return Vec::new();
      }

      // Scan for networks
      let nil: *mut AnyObject = std::ptr::null_mut();
      let err: *mut AnyObject = std::ptr::null_mut();
      let networks_set: *mut AnyObject = objc2::msg_send![interface, scanForNetworksWithName: nil, error: &err];
      if networks_set.is_null() {
        return Vec::new();
      }

      let count: usize = objc2::msg_send![networks_set, count];
      let enumerator: *mut AnyObject = objc2::msg_send![networks_set, objectEnumerator];
      let mut entries = Vec::with_capacity(count);

      for _ in 0..count {
        let network: *mut AnyObject = objc2::msg_send![enumerator, nextObject];
        if network.is_null() {
          break;
        }
        let rssi: i64 = objc2::msg_send![network, rssiValue];

        // BSSID - may be nil without Location Services authorization
        let bssid_obj: *mut AnyObject = objc2::msg_send![network, bssid];
        let bssid = nsstring_to_string(bssid_obj);

        // SSID - may be nil without Location Services authorization
        let ssid_obj: *mut AnyObject = objc2::msg_send![network, ssid];
        let ssid = nsstring_to_string(ssid_obj);

        entries.push(CoreWlanScanEntry { rssi, bssid, ssid });
      }

      entries
    }
  }
  #[cfg(not(target_os = "macos"))]
  Vec::new()
}

/// Fallback using legacy CWInterface class method
fn get_corewlan_scan_legacy() -> Vec<CoreWlanScanEntry> {
  #[cfg(target_os = "macos")]
  {
    use objc2::runtime::{AnyObject, Class};

    unsafe {
      let Some(cls) = Class::get(c"CWInterface") else {
        return Vec::new();
      };
      let interface: *mut AnyObject = objc2::msg_send![cls, interface];
      if interface.is_null() {
        return Vec::new();
      }

      let nil: *mut AnyObject = std::ptr::null_mut();
      let err: *mut AnyObject = std::ptr::null_mut();
      let networks_set: *mut AnyObject = objc2::msg_send![interface, scanForNetworksWithName: nil, error: &err];
      if networks_set.is_null() {
        return Vec::new();
      }

      let count: usize = objc2::msg_send![networks_set, count];
      let enumerator: *mut AnyObject = objc2::msg_send![networks_set, objectEnumerator];
      let mut entries = Vec::with_capacity(count);

      for _ in 0..count {
        let network: *mut AnyObject = objc2::msg_send![enumerator, nextObject];
        if network.is_null() {
          break;
        }
        let rssi: i64 = objc2::msg_send![network, rssiValue];

        let bssid_obj: *mut AnyObject = objc2::msg_send![network, bssid];
        let bssid = nsstring_to_string(bssid_obj);

        let ssid_obj: *mut AnyObject = objc2::msg_send![network, ssid];
        let ssid = nsstring_to_string(ssid_obj);

        entries.push(CoreWlanScanEntry { rssi, bssid, ssid });
      }

      entries
    }
  }
  #[cfg(not(target_os = "macos"))]
  Vec::new()
}

/// Convert NSString to Rust String, returns None if nil or empty
#[cfg(target_os = "macos")]
unsafe fn nsstring_to_string(obj: *mut objc2::runtime::AnyObject) -> Option<String> {
  use objc2::msg_send;
  if obj.is_null() {
    return None;
  }
  let utf8: *const std::ffi::c_char = msg_send![obj, UTF8String];
  if utf8.is_null() {
    return None;
  }
  let s = std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned();
  if s.is_empty() { None } else { Some(s) }
}

/// Parse "Signal / Noise: -44 dBm / -84 dBm" format, returns signal quality 0-100
fn parse_signal_noise(s: &str) -> Option<u32> {
  let dbm_str = s.trim().split_whitespace().next()?;
  let dbm: i32 = dbm_str.parse().ok()?;
  Some(((dbm + 100).max(0) as u32).min(100))
}

/// Map system_profiler security mode enum to human-readable string
fn map_security_mode(mode: &str) -> String {
  match mode {
    "spairport_security_mode_none" => "None".to_string(),
    "spairport_security_mode_wep" => "WEP".to_string(),
    "spairport_security_mode_wpa_personal" => "WPA Personal".to_string(),
    "spairport_security_mode_wpa_enterprise" => "WPA Enterprise".to_string(),
    "spairport_security_mode_wpa2_personal" => "WPA2 Personal".to_string(),
    "spairport_security_mode_wpa2_enterprise" => "WPA2 Enterprise".to_string(),
    "spairport_security_mode_wpa3_personal" => "WPA3 Personal".to_string(),
    "spairport_security_mode_wpa3_enterprise" => "WPA3 Enterprise".to_string(),
    "spairport_security_mode_wpa2_personal_mixed" => "WPA/WPA2 Personal".to_string(),
    "spairport_security_mode_wpa2_enterprise_mixed" => "WPA/WPA2 Enterprise".to_string(),
    other => other.to_string(),
  }
}
