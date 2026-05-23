use anyhow::{Context, Result};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

use crate::types::ProcessInfo;

pub fn get_process_name(process_id: u32) -> String {
  if process_id == 0 {
    return "System Idle Process".to_string();
  }
  unsafe {
    if let Ok(handle) = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
      let mut buffer = [0u16; 260];
      let len = windows::Win32::System::ProcessStatus::K32GetModuleBaseNameW(handle, None, &mut buffer);
      let _ = CloseHandle(handle);
      if len > 0 {
        return OsString::from_wide(&buffer[..len as usize])
          .to_string_lossy()
          .into_owned();
      }
    }
  }
  String::new()
}

fn extract_process_name(app_path: &str) -> String {
  std::path::Path::new(app_path)
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("unknown")
    .to_string()
}

pub fn get_processes_by_name(name: &str) -> Result<Vec<u32>> {
  use windows::Win32::System::Diagnostics::ToolHelp::*;
  let name_lower = name.to_lowercase();
  let has_ext = name_lower.ends_with(".exe");
  let target = if has_ext {
    name_lower.clone()
  } else {
    format!("{}.exe", name_lower)
  };
  let mut pids = Vec::new();
  unsafe {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).context("CreateToolhelp32Snapshot")?;
    let mut entry = PROCESSENTRY32W {
      dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
      ..Default::default()
    };
    if Process32FirstW(snapshot, &mut entry).is_ok() {
      loop {
        let current = String::from_utf16_lossy(&entry.szExeFile)
          .trim_end_matches('\0')
          .to_lowercase();
        if current == target {
          pids.push(entry.th32ProcessID);
        }
        if Process32NextW(snapshot, &mut entry).is_err() {
          break;
        }
      }
    }
    let _ = CloseHandle(snapshot);
  }
  Ok(pids)
}

pub fn launch_application_and_get_process_id(app_path: &str, wait_timeout_ms: u32) -> Result<u32> {
  let before = get_processes_by_name(&extract_process_name(app_path))?;
  // Also snapshot all PIDs that already have windows, so we can detect
  // spawned child processes (e.g. calc.exe → CalculatorApp.exe)
  let pids_with_windows_before = get_all_pids_with_windows();
  let app_wide: Vec<u16> = app_path.encode_utf16().chain(std::iter::once(0)).collect();
  let dir_wide: Vec<u16> = match std::path::Path::new(app_path).parent() {
    Some(p) if !p.as_os_str().is_empty() => p.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect(),
    _ => Vec::new(),
  };
  let si = STARTUPINFOW {
    cb: std::mem::size_of::<STARTUPINFOW>() as u32,
    ..Default::default()
  };
  let mut pi = PROCESS_INFORMATION::default();
  unsafe {
    let dir_ptr = if dir_wide.is_empty() {
      windows::core::PCWSTR::null()
    } else {
      windows::core::PCWSTR(dir_wide.as_ptr())
    };
    let ok = CreateProcessW(
      windows::core::PCWSTR(app_wide.as_ptr()),
      Some(windows::core::PWSTR::null()),
      Some(&SECURITY_ATTRIBUTES::default()),
      Some(&SECURITY_ATTRIBUTES::default()),
      false,
      CREATE_NEW_CONSOLE,
      None,
      dir_ptr,
      &si,
      &mut pi,
    );
    if ok.is_err() {
      return Err(windows::core::Error::from_thread().into());
    }
    let _ = CloseHandle(pi.hProcess);
    let _ = CloseHandle(pi.hThread);
  }
  let start = std::time::Instant::now();
  let timeout = std::time::Duration::from_millis(wait_timeout_ms as u64);
  let mut found = Vec::new();
  loop {
    // Check for new processes with the same name
    let current = get_processes_by_name(&extract_process_name(app_path))?;
    for &pid in &current {
      if !before.contains(&pid) && !found.contains(&pid) {
        found.push(pid);
      }
    }
    // Check same-name processes first
    for &pid in &found {
      if has_windows_for_process(pid) {
        return Ok(pid);
      }
    }
    // Also check for any new process with windows (handles UWP launchers like calc.exe)
    let pids_now = get_all_pids_with_windows();
    for &pid in &pids_now {
      if !pids_with_windows_before.contains(&pid) && has_windows_for_process(pid) {
        return Ok(pid);
      }
    }
    if start.elapsed() > timeout {
      break;
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
  }
  if !found.is_empty() {
    return Ok(found[0]);
  }
  Err(anyhow::anyhow!("No new process found after launch"))
}

pub fn terminate_application(pid: u32) -> Result<bool> {
  unsafe {
    let handle = OpenProcess(PROCESS_TERMINATE, false, pid).context("OpenProcess")?;
    let ok = TerminateProcess(handle, 0);
    let _ = CloseHandle(handle);
    Ok(ok.is_ok())
  }
}

pub fn terminate_applications_by_name(name: &str) -> Result<(u32, u32)> {
  let pids = get_processes_by_name(name)?;
  let total = pids.len() as u32;
  let mut success = 0u32;
  for pid in pids {
    if terminate_application(pid).unwrap_or(false) {
      success += 1;
    }
  }
  Ok((total, success))
}

/// Get all PIDs that currently have visible, non-tool windows
fn get_all_pids_with_windows() -> Vec<u32> {
  struct Data {
    pids: Vec<u32>,
  }
  unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
      let data = &mut *(lparam.0 as *mut Data);
      if IsWindow(Some(hwnd)).as_bool()
        && IsWindowVisible(hwnd).as_bool()
        && (GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) == 0
      {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != 0 && !data.pids.contains(&pid) {
          data.pids.push(pid);
        }
      }
      true.into()
    }
  }
  let mut data = Data { pids: Vec::new() };
  unsafe {
    let _ = EnumWindows(Some(cb), LPARAM(&mut data as *mut Data as isize));
  }
  data.pids
}

fn has_windows_for_process(pid: u32) -> bool {
  struct Data {
    pid: u32,
    windows: Vec<HWND>,
  }
  unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
      let data = &mut *(lparam.0 as *mut Data);
      let mut pid: u32 = 0;
      GetWindowThreadProcessId(hwnd, Some(&mut pid));
      if pid == data.pid {
        data.windows.push(hwnd);
      }
      true.into()
    }
  }
  let mut data = Data {
    pid,
    windows: Vec::new(),
  };
  unsafe {
    let _ = EnumWindows(Some(cb), LPARAM(&mut data as *mut Data as isize));
  }
  data.windows.iter().any(|&h| unsafe {
    IsWindow(Some(h)).as_bool()
      && IsWindowVisible(h).as_bool()
      && (GetWindowLongPtrW(h, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) == 0
  })
}

pub fn list_processes() -> Result<Vec<ProcessInfo>> {
  use windows::Win32::System::Diagnostics::ToolHelp::*;
  let mut processes = Vec::new();
  unsafe {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).context("CreateToolhelp32Snapshot")?;
    let mut entry = PROCESSENTRY32W {
      dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
      ..Default::default()
    };
    if Process32FirstW(snapshot, &mut entry).is_ok() {
      loop {
        let name = String::from_utf16_lossy(&entry.szExeFile)
          .trim_end_matches('\0')
          .to_string();
        processes.push(ProcessInfo {
          pid: entry.th32ProcessID,
          name,
          thread_count: entry.cntThreads,
          parent_pid: entry.th32ParentProcessID,
          memory_bytes: 0,
        });
        if Process32NextW(snapshot, &mut entry).is_err() {
          break;
        }
      }
    }
    let _ = CloseHandle(snapshot);
  }
  Ok(processes)
}

pub fn get_process_info(pid: u32) -> Result<ProcessInfo> {
  use windows::Win32::System::Diagnostics::ToolHelp::*;
  unsafe {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).context("CreateToolhelp32Snapshot")?;
    let mut entry = PROCESSENTRY32W {
      dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
      ..Default::default()
    };
    if Process32FirstW(snapshot, &mut entry).is_ok() {
      loop {
        if entry.th32ProcessID == pid {
          let name = String::from_utf16_lossy(&entry.szExeFile)
            .trim_end_matches('\0')
            .to_string();
          let _ = CloseHandle(snapshot);
          return Ok(ProcessInfo {
            pid: entry.th32ProcessID,
            name,
            thread_count: entry.cntThreads,
            parent_pid: entry.th32ParentProcessID,
            memory_bytes: 0,
          });
        }
        if Process32NextW(snapshot, &mut entry).is_err() {
          break;
        }
      }
    }
    let _ = CloseHandle(snapshot);
  }
  Err(anyhow::anyhow!("process not found: {pid}"))
}
