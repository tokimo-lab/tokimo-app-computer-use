use crate::error::Result;
use crate::types::ServiceInfo;
use windows::Win32::System::Services::*;

pub fn list_services() -> Result<Vec<ServiceInfo>> {
  let mut services = Vec::new();

  unsafe {
    let sc_manager = OpenSCManagerW(None, None, SC_MANAGER_ENUMERATE_SERVICE)?;
    let _guard = ServiceHandleGuard(sc_manager);

    let mut needed = 0u32;
    let mut count = 0u32;
    let mut resume_handle = 0u32;

    let _ = EnumServicesStatusExW(
      sc_manager,
      SC_ENUM_PROCESS_INFO,
      SERVICE_WIN32,
      SERVICE_STATE_ALL,
      None,
      &mut needed,
      &mut count,
      Some(&mut resume_handle),
      None,
    );

    if needed == 0 {
      return Ok(services);
    }

    let mut buffer = vec![0u8; needed as usize];
    resume_handle = 0;
    EnumServicesStatusExW(
      sc_manager,
      SC_ENUM_PROCESS_INFO,
      SERVICE_WIN32,
      SERVICE_STATE_ALL,
      Some(&mut buffer),
      &mut needed,
      &mut count,
      Some(&mut resume_handle),
      None,
    )?;

    let info_ptr = buffer.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW;
    for i in 0..count as usize {
      let info = &*info_ptr.add(i);
      let name = info.lpServiceName.to_string().unwrap_or_default();
      let display = info.lpDisplayName.to_string().unwrap_or_default();
      let status = service_state_str(info.ServiceStatusProcess.dwCurrentState.0);
      let stype = service_type_str(info.ServiceStatusProcess.dwServiceType.0);

      if !name.is_empty() {
        services.push(ServiceInfo {
          name,
          display_name: display,
          status: status.to_string(),
          service_type: stype.to_string(),
        });
      }
    }
  }

  Ok(services)
}

pub fn get_service_detail(name: &str) -> Result<ServiceInfo> {
  unsafe {
    let sc_manager = OpenSCManagerW(None, None, SC_MANAGER_ENUMERATE_SERVICE)?;
    let _guard = ServiceHandleGuard(sc_manager);

    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let service = OpenServiceW(sc_manager, windows::core::PCWSTR(name_w.as_ptr()), SERVICE_QUERY_STATUS)?;
    let _svc_guard = ServiceHandleGuard(service);

    let mut needed = 0u32;
    let _ = QueryServiceStatusEx(service, SC_STATUS_PROCESS_INFO, None, &mut needed);

    if needed == 0 {
      anyhow::bail!("service not found: {name}");
    }

    let mut buffer = vec![0u8; needed as usize];
    QueryServiceStatusEx(service, SC_STATUS_PROCESS_INFO, Some(&mut buffer), &mut needed)?;

    let status_ptr = buffer.as_ptr() as *const SERVICE_STATUS_PROCESS;
    let status_info = &*status_ptr;

    // Get display name via EnumServicesStatusExW lookup
    let display = get_display_name(sc_manager, name).unwrap_or_default();
    let status = service_state_str(status_info.dwCurrentState.0);
    let stype = service_type_str(status_info.dwServiceType.0);

    Ok(ServiceInfo {
      name: name.to_string(),
      display_name: display,
      status: status.to_string(),
      service_type: stype.to_string(),
    })
  }
}

pub fn start_service(name: &str) -> Result<()> {
  unsafe {
    let sc_manager = OpenSCManagerW(None, None, SC_MANAGER_CONNECT)?;
    let _guard = ServiceHandleGuard(sc_manager);

    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let service = OpenServiceW(sc_manager, windows::core::PCWSTR(name_w.as_ptr()), SERVICE_START)?;
    let _svc_guard = ServiceHandleGuard(service);

    StartServiceW(service, None)?;
  }
  Ok(())
}

pub fn stop_service(name: &str) -> Result<()> {
  unsafe {
    let sc_manager = OpenSCManagerW(None, None, SC_MANAGER_CONNECT)?;
    let _guard = ServiceHandleGuard(sc_manager);

    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let service = OpenServiceW(sc_manager, windows::core::PCWSTR(name_w.as_ptr()), SERVICE_STOP)?;
    let _svc_guard = ServiceHandleGuard(service);

    let mut status = SERVICE_STATUS::default();
    ControlService(service, SERVICE_CONTROL_STOP, &mut status)?;
  }
  Ok(())
}

fn get_display_name(sc_manager: SC_HANDLE, service_name: &str) -> Option<String> {
  let name_w: Vec<u16> = service_name.encode_utf16().chain(std::iter::once(0)).collect();
  unsafe {
    let service = OpenServiceW(sc_manager, windows::core::PCWSTR(name_w.as_ptr()), SERVICE_QUERY_CONFIG).ok()?;
    let _guard = ServiceHandleGuard(service);

    let mut needed = 0u32;
    let _ = QueryServiceConfigW(service, None, 0, &mut needed);
    if needed == 0 {
      return None;
    }
    let mut buffer = vec![0u8; needed as usize];
    QueryServiceConfigW(service, Some(buffer.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW), needed, &mut needed).ok()?;
    let config = &*(buffer.as_ptr() as *const QUERY_SERVICE_CONFIGW);
    Some(config.lpDisplayName.to_string().unwrap_or_default())
  }
}

fn service_state_str(state: u32) -> &'static str {
  match state {
    1 => "Stopped",
    2 => "Starting",
    3 => "Stopping",
    4 => "Running",
    5 => "Continuing",
    6 => "Pausing",
    7 => "Paused",
    _ => "Unknown",
  }
}

fn service_type_str(stype: u32) -> &'static str {
  if stype & SERVICE_WIN32_OWN_PROCESS.0 != 0 {
    "OwnProcess"
  } else if stype & SERVICE_WIN32_SHARE_PROCESS.0 != 0 {
    "ShareProcess"
  } else if stype & SERVICE_KERNEL_DRIVER.0 != 0 {
    "KernelDriver"
  } else if stype & SERVICE_FILE_SYSTEM_DRIVER.0 != 0 {
    "FileSystemDriver"
  } else {
    "Other"
  }
}

struct ServiceHandleGuard(SC_HANDLE);

impl Drop for ServiceHandleGuard {
  fn drop(&mut self) {
    unsafe {
      let _ = windows::Win32::System::Services::CloseServiceHandle(self.0);
    }
  }
}
