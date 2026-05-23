use std::ffi::CStr;

use crate::error::Result;
use crate::types::*;

// --- libproc FFI ---

unsafe extern "C" {
  fn proc_listpids(type_: u32, typeinfo: u32, buffer: *mut u32, buffersize: u32) -> i32;
  fn proc_pidpath(pid: i32, buffer: *mut std::ffi::c_char, buffersize: u32) -> i32;
  fn proc_name(pid: i32, buffer: *mut std::ffi::c_char, buffersize: u32) -> i32;
  fn proc_pidinfo(pid: i32, flavor: u32, arg: u64, buffer: *mut std::ffi::c_void, buffersize: i32) -> i32;
}

const PROC_ALL_PIDS: u32 = 1;
const PROC_PIDTASKINFO: u32 = 4;

#[repr(C)]
struct ProcTaskInfo {
  pti_virtual_size: u64,
  pti_resident_size: u64,
  pti_total_user: u64,
  pti_total_system: u64,
  pti_threads_user: u64,
  pti_threads_system: u64,
  pti_policy: i32,
  pti_faults: i32,
  pti_pageins: i32,
  pti_cow_faults: i32,
  pti_messages_sent: i32,
  pti_messages_received: i32,
  pti_syscalls_mach: i32,
  pti_syscalls_unix: i32,
  pti_csw: i32,
  pti_threadnum: i32,
  pti_numrunning: i32,
  pti_priority: i32,
}

/// Get thread count and memory usage for a process.
fn get_task_info(pid: u32) -> (u32, u64) {
  unsafe {
    let mut info: ProcTaskInfo = std::mem::zeroed();
    let ret = proc_pidinfo(
      pid as i32,
      PROC_PIDTASKINFO,
      0,
      &mut info as *mut _ as *mut _,
      std::mem::size_of::<ProcTaskInfo>() as i32,
    );
    if ret > 0 {
      (info.pti_threadnum as u32, info.pti_resident_size)
    } else {
      (0, 0)
    }
  }
}

/// Get all PIDs on the system.
fn get_all_pids() -> Vec<u32> {
  unsafe {
    let buf_size = proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0);
    if buf_size <= 0 {
      return Vec::new();
    }
    let count = buf_size as usize / std::mem::size_of::<u32>();
    let mut pids = vec![0u32; count];
    let actual = proc_listpids(PROC_ALL_PIDS, 0, pids.as_mut_ptr(), buf_size as u32);
    if actual <= 0 {
      return Vec::new();
    }
    pids.truncate(actual as usize / std::mem::size_of::<u32>());
    pids
  }
}

/// Get the full path of a process by PID.
fn get_proc_path(pid: u32) -> String {
  unsafe {
    let mut buf = [0i8; 1024];
    let len = proc_pidpath(pid as i32, buf.as_mut_ptr(), buf.len() as u32);
    if len > 0 {
      CStr::from_ptr(buf.as_ptr())
        .to_string_lossy()
        .into_owned()
    } else {
      String::new()
    }
  }
}

/// Get the process name by PID.
fn get_proc_name(pid: u32) -> String {
  unsafe {
    let mut buf = [0i8; 1024];
    let len = proc_name(pid as i32, buf.as_mut_ptr(), buf.len() as u32);
    if len > 0 {
      CStr::from_ptr(buf.as_ptr())
        .to_string_lossy()
        .into_owned()
    } else {
      String::new()
    }
  }
}

fn extract_name(path: &str) -> String {
  std::path::Path::new(path)
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or(path)
    .to_string()
}

pub fn list_processes() -> Result<Vec<ProcessInfo>> {
  let pids = get_all_pids();
  let mut processes = Vec::with_capacity(pids.len());
  for pid in pids {
    if pid == 0 {
      continue;
    }
    let path = get_proc_path(pid);
    let name = if path.is_empty() {
      get_proc_name(pid)
    } else {
      extract_name(&path)
    };
    if name.is_empty() {
      continue;
    }
    let (thread_count, memory_bytes) = get_task_info(pid);
    processes.push(ProcessInfo {
      pid,
      name,
      thread_count,
      parent_pid: 0,
      memory_bytes,
    });
  }
  Ok(processes)
}

pub fn get_process_info(pid: u32) -> Result<ProcessInfo> {
  let path = get_proc_path(pid);
  let name = if path.is_empty() {
    get_proc_name(pid)
  } else {
    extract_name(&path)
  };
  if name.is_empty() {
    return Err(anyhow::anyhow!("process not found: {pid}"));
  }
  let (thread_count, memory_bytes) = get_task_info(pid);
  Ok(ProcessInfo {
    pid,
    name,
    thread_count,
    parent_pid: 0,
    memory_bytes,
  })
}

pub fn get_process_ids_by_name(name: &str) -> Result<Vec<u32>> {
  let name_lower = name.to_lowercase();
  let pids = get_all_pids();
  let mut result = Vec::new();
  for pid in pids {
    if pid == 0 {
      continue;
    }
    let path = get_proc_path(pid);
    let proc_name = if path.is_empty() {
      get_proc_name(pid)
    } else {
      extract_name(&path)
    };
    if proc_name.to_lowercase().contains(&name_lower)
      || path.to_lowercase().contains(&name_lower)
    {
      result.push(pid);
    }
  }
  Ok(result)
}

pub fn launch_app(path: &str, wait_timeout_ms: u32) -> Result<u32> {
  // Collect PIDs before launch
  let target_name = extract_name(path);
  let before_pids: std::collections::HashSet<u32> =
    get_process_ids_by_name(&target_name)?.into_iter().collect();

  // Use NSWorkspace to open the application
  let launched: anyhow::Result<()> = launch_via_ns_workspace(path).or_else(|_| {
    // Fallback: direct exec
    std::process::Command::new(path).spawn().map(|_| ()).map_err(|e| e.into())
  });

  if let Err(e) = launched {
    return Err(anyhow::anyhow!("failed to launch {path}: {e}"));
  }

  // Wait for the new process to appear
  let start = std::time::Instant::now();
  let timeout = std::time::Duration::from_millis(wait_timeout_ms as u64);
  loop {
    let current = get_process_ids_by_name(&target_name)?;
    for &pid in &current {
      if !before_pids.contains(&pid) {
        return Ok(pid);
      }
    }
    if start.elapsed() > timeout {
      break;
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
  }
  Err(anyhow::anyhow!("no new process found after launching: {path}"))
}

fn launch_via_ns_workspace(path: &str) -> Result<()> {
  unsafe {
    let cls = objc2::class!(NSWorkspace);
    let ws: *mut objc2::runtime::AnyObject = objc2::msg_send![cls, sharedWorkspace];
    if ws.is_null() {
      return Err(anyhow::anyhow!("sharedWorkspace is null"));
    }

    // Create NSString from path
    let path_str: &CStr = &std::ffi::CString::new(path).unwrap();
    let ns_string_cls = objc2::class!(NSString);
    let ns_path: *mut objc2::runtime::AnyObject = objc2::msg_send![
      ns_string_cls,
      stringWithUTF8String: path_str.as_ptr()
    ];
    if ns_path.is_null() {
      return Err(anyhow::anyhow!("stringWithUTF8String returned null"));
    }

    // Create NSURL from path string
    let ns_url_cls = objc2::class!(NSURL);
    let url: *mut objc2::runtime::AnyObject =
      objc2::msg_send![ns_url_cls, fileURLWithPath: ns_path];
    if url.is_null() {
      return Err(anyhow::anyhow!("fileURLWithPath returned null"));
    }

    // Open the URL
    let opened: bool = objc2::msg_send![ws, openURL: url];
    if opened {
      Ok(())
    } else {
      Err(anyhow::anyhow!("NSWorkspace::openURL returned false"))
    }
  }
}

pub fn terminate_app(pid: u32) -> Result<bool> {
  unsafe {
    let cls =
      objc2::class!(NSRunningApplication);
    let app: *mut objc2::runtime::AnyObject =
      objc2::msg_send![cls, runningApplicationWithProcessIdentifier: pid as i32];
    if app.is_null() {
      // Fallback to kill
      let status = std::process::Command::new("kill").arg(pid.to_string()).status()?;
      return Ok(status.success());
    }
    let result: bool = objc2::msg_send![app, terminate];
    Ok(result)
  }
}

pub fn terminate_apps_by_name(name: &str) -> Result<(u32, u32)> {
  let pids = get_process_ids_by_name(name)?;
  let total = pids.len() as u32;
  let mut success = 0u32;
  for pid in pids {
    if terminate_app(pid).unwrap_or(false) {
      success += 1;
    }
  }
  Ok((total, success))
}
