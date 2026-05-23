use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

use crate::types::TerminalResult;

pub fn execute_command(shell_type: &str, command: &str) -> Result<TerminalResult> {
  match shell_type {
    "ps" => {
      let pwsh = find_pwsh().unwrap_or_else(|| "powershell".to_string());
      run_shell(&pwsh, &["-NoProfile", "-Command", command])
    }
    "cmd" => run_shell("cmd", &["/c", command]),
    other => Err(anyhow::anyhow!("unsupported shell type: {other} (use 'ps' or 'cmd')")),
  }
}

fn find_pwsh() -> Option<String> {
  // 1. Registry: App Paths
  if let Some(path) = reg_app_paths("pwsh.exe") {
    if PathBuf::from(&path).exists() {
      return Some(path);
    }
  }

  // 2. Known install paths
  let candidates = [
    r"C:\Program Files\PowerShell\7\pwsh.exe",
    r"C:\Program Files (x86)\PowerShell\7\pwsh.exe",
  ];
  for p in &candidates {
    if PathBuf::from(p).exists() {
      return Some(p.to_string());
    }
  }

  // 3. PATH fallback
  if Command::new("pwsh").arg("--version")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status().map(|s| s.success()).unwrap_or(false)
  {
    return Some("pwsh".to_string());
  }

  None
}

fn reg_app_paths(exe_name: &str) -> Option<String> {
  use windows::Win32::System::Registry::*;
  use windows::Win32::Foundation::*;

  let key_path = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe_name}");
  let key_w: Vec<u16> = key_path.encode_utf16().chain(std::iter::once(0)).collect();
  let default_w: Vec<u16> = "\0".encode_utf16().collect();
  let mut hkey = HKEY::default();

  let status = unsafe {
    RegOpenKeyExW(
      HKEY_LOCAL_MACHINE,
      windows::core::PCWSTR(key_w.as_ptr()),
      Some(0),
      KEY_READ,
      &mut hkey,
    )
  };
  if status != ERROR_SUCCESS {
    // Try HKCU
    let status = unsafe {
      RegOpenKeyExW(
        HKEY_CURRENT_USER,
        windows::core::PCWSTR(key_w.as_ptr()),
        Some(0),
        KEY_READ,
        &mut hkey,
      )
    };
    if status != ERROR_SUCCESS {
      return None;
    }
  }

  let mut buf = [0u16; 512];
  let mut buf_size = (buf.len() * 2) as u32;
  let mut data_type = REG_SZ;

  let status = unsafe {
    RegQueryValueExW(
      hkey,
      windows::core::PCWSTR(default_w.as_ptr()),
      None,
      Some(&mut data_type),
      Some(buf.as_mut_ptr() as *mut u8),
      Some(&mut buf_size),
    )
  };

  unsafe { let _ = RegCloseKey(hkey); };

  if status != ERROR_SUCCESS || data_type != REG_SZ {
    return None;
  }

  let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
  let s = String::from_utf16_lossy(&buf[..len]);
  if s.is_empty() { None } else { Some(s) }
}

fn run_shell(program: &str, args: &[&str]) -> Result<TerminalResult> {
  let output = Command::new(program)
    .args(args)
    .output()
    .map_err(|e| anyhow::anyhow!("failed to run {program}: {e}"))?;

  Ok(TerminalResult {
    exit_code: output.status.code().unwrap_or(-1),
    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
  })
}
