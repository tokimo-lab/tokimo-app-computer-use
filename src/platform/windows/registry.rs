use anyhow::{Result, anyhow};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::*;
use windows::core::PCWSTR;

/// Convert &str to null-terminated UTF-16 for Win32 PCWSTR params.
fn to_wide(s: &str) -> Vec<u16> {
  s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Parse a key path like "HKLM\SOFTWARE\Microsoft" into (root_handle, subkey).
fn parse_key_path(path: &str) -> Result<(HKEY, &str)> {
  let (root, rest) = if let Some(rest) = path.strip_prefix("HKLM\\") {
    (HKEY_LOCAL_MACHINE, rest)
  } else if let Some(rest) = path.strip_prefix("HKCU\\") {
    (HKEY_CURRENT_USER, rest)
  } else if let Some(rest) = path.strip_prefix("HKCR\\") {
    (HKEY_CLASSES_ROOT, rest)
  } else if let Some(rest) = path.strip_prefix("HKU\\") {
    (HKEY_USERS, rest)
  } else if let Some(rest) = path.strip_prefix("HKCC\\") {
    (HKEY_CURRENT_CONFIG, rest)
  } else {
    return Err(anyhow!(
      "invalid key path: '{path}' — must start with HKLM\\, HKCU\\, HKCR\\, HKU\\, or HKCC\\"
    ));
  };
  if rest.is_empty() {
    return Err(anyhow!("key path must include a subkey after root"));
  }
  Ok((root, rest))
}

/// Open a registry key with the given access rights.
fn open_key(path: &str, access: REG_SAM_FLAGS) -> Result<HKEY> {
  let (root, subkey) = parse_key_path(path)?;
  let subkey_w = to_wide(subkey);
  let mut hkey = HKEY::default();
  let result = unsafe { RegOpenKeyExW(root, PCWSTR(subkey_w.as_ptr()), None, access, &mut hkey) };
  if result != ERROR_SUCCESS {
    return Err(anyhow!("failed to open registry key '{path}': error {result:?}"));
  }
  Ok(hkey)
}

/// Read a single registry value. Returns (type_name, value_display).
pub fn read_value(key_path: &str, value_name: Option<&str>) -> Result<(String, String)> {
  let hkey = open_key(key_path, KEY_READ)?;
  let name_wide: Option<Vec<u16>> = value_name.map(|s| to_wide(s));

  // First call: get size and type
  let mut data_type = REG_SZ;
  let mut data_size: u32 = 0;
  let name_pcwstr = name_wide.as_ref().map(|v| PCWSTR(v.as_ptr()));
  let result = unsafe {
    RegQueryValueExW(
      hkey,
      name_pcwstr.unwrap_or(PCWSTR::null()),
      None,
      Some(&mut data_type),
      None,
      Some(&mut data_size),
    )
  };
  if result != ERROR_SUCCESS {
    unsafe { let _ = RegCloseKey(hkey); }
    return Err(anyhow!("failed to read value '{:?}': error {result:?}", value_name.unwrap_or("(Default)")));
  }

  // Second call: read data
  let mut buf = vec![0u8; data_size as usize];
  let result = unsafe {
    RegQueryValueExW(
      hkey,
      name_pcwstr.unwrap_or(PCWSTR::null()),
      None,
      None,
      Some(buf.as_mut_ptr()),
      Some(&mut data_size),
    )
  };
  unsafe { let _ = RegCloseKey(hkey); }
  if result != ERROR_SUCCESS {
    return Err(anyhow!("failed to read value data: error {result:?}"));
  }

  let (type_name, value_str) = match data_type {
    REG_SZ | REG_EXPAND_SZ => {
      let s = utf16_from_bytes(&buf);
      let type_name = if data_type == REG_SZ { "REG_SZ" } else { "REG_EXPAND_SZ" };
      (type_name.to_string(), s)
    }
    REG_DWORD => {
      let val = if buf.len() >= 4 {
        u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]])
      } else {
        0
      };
      ("REG_DWORD".to_string(), val.to_string())
    }
    REG_QWORD => {
      let val = if buf.len() >= 8 {
        u64::from_ne_bytes([
          buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        ])
      } else {
        0
      };
      ("REG_QWORD".to_string(), val.to_string())
    }
    REG_BINARY => {
      let hex: Vec<String> = buf.iter().map(|b| format!("{b:02X}")).collect();
      ("REG_BINARY".to_string(), hex.join(" "))
    }
    REG_MULTI_SZ => {
      let mut strings = Vec::new();
      let mut start = 0;
      for (i, window) in buf.chunks_exact(2).enumerate() {
        let val = u16::from_ne_bytes([window[0], window[1]]);
        if val == 0 {
          if i > start {
            let chunk = &buf[start..i * 2];
            strings.push(utf16_from_bytes(chunk));
          }
          start = (i + 1) * 2;
          // Double null = end of list
          if i + 1 < buf.len() / 2 {
            let next = u16::from_ne_bytes([buf[(i + 1) * 2], buf[(i + 1) * 2 + 1]]);
            if next == 0 {
              break;
            }
          }
        }
      }
      ("REG_MULTI_SZ".to_string(), strings.join("\n"))
    }
    _ => {
      let hex: Vec<String> = buf.iter().map(|b| format!("{b:02X}")).collect();
      (format!("REG_UNKNOWN({})", data_type.0), hex.join(" "))
    }
  };

  Ok((type_name, value_str))
}

/// List subkey names under a key path.
pub fn list_subkeys(key_path: &str) -> Result<Vec<String>> {
  let hkey = open_key(key_path, KEY_READ)?;
  let mut subkeys = Vec::new();
  let mut index: u32 = 0;
  let mut name_buf = [0u16; 256];

  loop {
    let mut name_len = name_buf.len() as u32;
    let result = unsafe {
      RegEnumKeyExW(
        hkey,
        index,
        Some(windows::core::PWSTR(name_buf.as_mut_ptr())),
        &mut name_len,
        None,
        None,
        None,
        None,
      )
    };
    if result != ERROR_SUCCESS {
      break;
    }
    let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
    subkeys.push(name);
    index += 1;
  }

  unsafe { let _ = RegCloseKey(hkey); }
  Ok(subkeys)
}

/// List value names under a key path.
pub fn list_values(key_path: &str) -> Result<Vec<String>> {
  let hkey = open_key(key_path, KEY_READ)?;
  let mut values = Vec::new();
  let mut index: u32 = 0;
  let mut name_buf = [0u16; 256];

  loop {
    let mut name_len = name_buf.len() as u32;
    let result = unsafe {
      RegEnumValueW(
        hkey,
        index,
        Some(windows::core::PWSTR(name_buf.as_mut_ptr())),
        &mut name_len,
        None,
        None,
        None,
        None,
      )
    };
    if result != ERROR_SUCCESS {
      break;
    }
    let name = if name_len == 0 {
      "(Default)".to_string()
    } else {
      String::from_utf16_lossy(&name_buf[..name_len as usize])
    };
    values.push(name);
    index += 1;
  }

  unsafe { let _ = RegCloseKey(hkey); }
  Ok(values)
}

/// Set a registry value. value_type is one of: REG_SZ, REG_DWORD, REG_QWORD, REG_BINARY, REG_EXPAND_SZ, REG_MULTI_SZ
pub fn set_value(key_path: &str, value_name: &str, value_type: &str, data: &str) -> Result<()> {
  let (root, subkey) = parse_key_path(key_path)?;
  let subkey_w = to_wide(subkey);
  let mut hkey = HKEY::default();
  let result = unsafe {
    RegCreateKeyExW(
      root,
      PCWSTR(subkey_w.as_ptr()),
      None,
      PCWSTR::null(),
      REG_OPTION_NON_VOLATILE,
      KEY_WRITE,
      None,
      &mut hkey,
      None,
    )
  };
  if result != ERROR_SUCCESS {
    return Err(anyhow!("failed to open/create key '{key_path}': error {result:?}"));
  }

  let name_wide = to_wide(value_name);

  let result = match value_type {
    "REG_SZ" => {
      let data_wide = to_wide(data);
      unsafe { RegSetValueExW(hkey, PCWSTR(name_wide.as_ptr()), None, REG_SZ, Some(slice_to_bytes(&data_wide))) }
    }
    "REG_EXPAND_SZ" => {
      let data_wide = to_wide(data);
      unsafe { RegSetValueExW(hkey, PCWSTR(name_wide.as_ptr()), None, REG_EXPAND_SZ, Some(slice_to_bytes(&data_wide))) }
    }
    "REG_DWORD" => {
      let val: u32 = data.parse().map_err(|_| anyhow!("invalid DWORD value: '{data}'"))?;
      unsafe { RegSetValueExW(hkey, PCWSTR(name_wide.as_ptr()), None, REG_DWORD, Some(&val.to_ne_bytes())) }
    }
    "REG_QWORD" => {
      let val: u64 = data.parse().map_err(|_| anyhow!("invalid QWORD value: '{data}'"))?;
      unsafe { RegSetValueExW(hkey, PCWSTR(name_wide.as_ptr()), None, REG_QWORD, Some(&val.to_ne_bytes())) }
    }
    "REG_BINARY" => {
      let bytes = parse_hex_bytes(data)?;
      unsafe { RegSetValueExW(hkey, PCWSTR(name_wide.as_ptr()), None, REG_BINARY, Some(&bytes)) }
    }
    "REG_MULTI_SZ" => {
      let mut buf = Vec::new();
      for s in data.split('\n') {
        for c in s.encode_utf16() {
          buf.extend_from_slice(&c.to_ne_bytes());
        }
        buf.extend_from_slice(&0u16.to_ne_bytes());
      }
      buf.extend_from_slice(&0u16.to_ne_bytes()); // double null terminator
      unsafe { RegSetValueExW(hkey, PCWSTR(name_wide.as_ptr()), None, REG_MULTI_SZ, Some(&buf)) }
    }
    other => {
      unsafe { let _ = RegCloseKey(hkey); }
      return Err(anyhow!("unsupported value type: '{other}'"));
    }
  };

  unsafe { let _ = RegCloseKey(hkey); }
  if result != ERROR_SUCCESS {
    return Err(anyhow!("failed to set value: error {result:?}"));
  }
  Ok(())
}

/// Create a new subkey.
pub fn create_key(key_path: &str) -> Result<()> {
  let (root, subkey) = parse_key_path(key_path)?;
  let subkey_w = to_wide(subkey);
  let mut hkey = HKEY::default();
  let result = unsafe {
    RegCreateKeyExW(
      root,
      PCWSTR(subkey_w.as_ptr()),
      None,
      PCWSTR::null(),
      REG_OPTION_NON_VOLATILE,
      KEY_WRITE,
      None,
      &mut hkey,
      None,
    )
  };
  if result != ERROR_SUCCESS {
    return Err(anyhow!("failed to create key '{key_path}': error {result:?}"));
  }
  unsafe { let _ = RegCloseKey(hkey); }
  Ok(())
}

/// Delete a registry value.
pub fn delete_value(key_path: &str, value_name: &str) -> Result<()> {
  let hkey = open_key(key_path, KEY_WRITE)?;
  let name_wide = to_wide(value_name);
  let result = unsafe { RegDeleteValueW(hkey, PCWSTR(name_wide.as_ptr())) };
  unsafe { let _ = RegCloseKey(hkey); }
  if result != ERROR_SUCCESS {
    return Err(anyhow!("failed to delete value '{value_name}': error {result:?}"));
  }
  Ok(())
}

/// Delete a registry key (must be empty, no recursive delete).
pub fn delete_key(key_path: &str) -> Result<()> {
  let (root, subkey) = parse_key_path(key_path)?;
  let subkey_w = to_wide(subkey);
  let result = unsafe { RegDeleteKeyW(root, PCWSTR(subkey_w.as_ptr())) };
  if result != ERROR_SUCCESS {
    return Err(anyhow!("failed to delete key '{key_path}': error {result:?}"));
  }
  Ok(())
}

// ── helpers ──

fn utf16_from_bytes(bytes: &[u8]) -> String {
  let utf16: Vec<u16> = bytes
    .chunks_exact(2)
    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
    .take_while(|&v| v != 0)
    .collect();
  String::from_utf16_lossy(&utf16)
}

fn slice_to_bytes(slice: &[u16]) -> &[u8] {
  unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, slice.len() * 2) }
}

fn parse_hex_bytes(s: &str) -> Result<Vec<u8>> {
  s.split_whitespace()
    .map(|hex| u8::from_str_radix(hex, 16).map_err(|_| anyhow!("invalid hex byte: '{hex}'")))
    .collect()
}
