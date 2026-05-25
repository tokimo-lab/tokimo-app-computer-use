use std::sync::Arc;

use crate::platform::PlatformProvider;

use super::cache::SnapshotCache;

/// IPC pipe name on Windows
#[cfg(windows)]
pub const PIPE_NAME: &str = r"\\.\pipe\tokimo-app-computer-daemon";

/// Unix socket path on macOS/Linux
#[cfg(unix)]
pub const SOCKET_PATH: &str = "/tmp/tokimo-app-computer-daemon.sock";

/// Start the IPC server, blocking the current thread.
/// Each incoming connection is handled in a new thread.
pub fn run_server<P: PlatformProvider + Send + Sync + 'static>(
  platform: Arc<P>,
  cache: Arc<SnapshotCache>,
) -> std::io::Result<()> {
  #[cfg(windows)]
  {
    run_named_pipe_server(platform, cache)
  }
  #[cfg(unix)]
  {
    run_unix_socket_server(platform, cache)
  }
}

#[cfg(windows)]
fn run_named_pipe_server<P: PlatformProvider + Send + Sync + 'static>(
  platform: Arc<P>,
  cache: Arc<SnapshotCache>,
) -> std::io::Result<()> {
  use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
  use windows::Win32::Storage::FileSystem::*;
  use windows::Win32::System::Pipes::*;
  use windows::core::PCWSTR;

  println!("Starting tokimo-app-computer-daemon on {PIPE_NAME}");

  loop {
    let pipe_name_wide: Vec<u16> = PIPE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    let pipe_handle = unsafe {
      CreateNamedPipeW(
        PCWSTR(pipe_name_wide.as_ptr()),
        PIPE_ACCESS_DUPLEX,
        PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
        10,   // max instances
        4096, // out buffer
        4096, // in buffer
        0,    // default timeout
        None, // security attributes
      )
    };

    if pipe_handle == INVALID_HANDLE_VALUE {
      return Err(std::io::Error::last_os_error());
    }

    // Wait for a client to connect
    let connected = unsafe { ConnectNamedPipe(pipe_handle, None) };

    if connected.is_err() {
      let err = std::io::Error::last_os_error();
      // ERROR_PIPE_CONNECTED (535) means client connected between Create and Connect
      if err.raw_os_error() != Some(535) {
        unsafe {
          let _ = CloseHandle(pipe_handle);
        }
        continue;
      }
    }

    let platform = platform.clone();
    let cache = cache.clone();
    let raw_handle = pipe_handle.0 as usize;
    std::thread::spawn(move || {
      use super::handler::handle_request;
      use super::protocol::Request;
      use std::io::{BufRead, BufReader, Write};
      use std::os::windows::io::FromRawHandle;
      use windows::Win32::Foundation::HANDLE;
      let pipe_handle = HANDLE(raw_handle as *mut core::ffi::c_void);
      let mut file = unsafe { std::fs::File::from_raw_handle(pipe_handle.0 as _) };
      let reader_file = file.try_clone().expect("clone pipe handle");
      let reader = BufReader::new(reader_file);

      for line in reader.lines() {
        let line = match line {
          Ok(l) if !l.is_empty() => l,
          _ => break,
        };

        let request: Request = match serde_json::from_str(&line) {
          Ok(r) => r,
          Err(e) => {
            let resp = super::protocol::Response::error(0, -32700, format!("parse error: {e}"));
            let mut resp_json = serde_json::to_string(&resp).unwrap();
            resp_json.push('\n');
            let _ = file.write_all(resp_json.as_bytes());
            continue;
          }
        };

        let resp = handle_request(platform.as_ref(), cache.as_ref(), request);
        let mut resp_json = serde_json::to_string(&resp).unwrap();
        resp_json.push('\n');
        let _ = file.write_all(resp_json.as_bytes());
      }

      // Disconnect before dropping file (which closes the handle)
      unsafe {
        let _ = DisconnectNamedPipe(pipe_handle);
      }
      drop(file);
    });
  }
}

#[cfg(unix)]
fn run_unix_socket_server<P: PlatformProvider + Send + Sync + 'static>(
  platform: Arc<P>,
  cache: Arc<SnapshotCache>,
) -> std::io::Result<()> {
  use std::os::unix::net::UnixListener;

  // Remove stale socket if exists
  let _ = std::fs::remove_file(SOCKET_PATH);

  let listener = UnixListener::bind(SOCKET_PATH)?;
  println!("Starting tokimo-app-computer-daemon on {SOCKET_PATH}");

  for stream in listener.incoming() {
    let stream = match stream {
      Ok(s) => s,
      Err(e) => {
        eprintln!("Connection failed: {e}");
        continue;
      }
    };

    let platform = platform.clone();
    let cache = cache.clone();
    std::thread::spawn(move || {
      use super::handler::handle_request;
      use super::protocol::Request;
      use std::io::{BufRead, BufReader, Write};

      let reader_stream = stream.try_clone().expect("clone unix stream");
      let reader = BufReader::new(reader_stream);
      let mut writer = stream;

      for line in reader.lines() {
        let line = match line {
          Ok(l) if !l.is_empty() => l,
          _ => break,
        };

        let request: Request = match serde_json::from_str(&line) {
          Ok(r) => r,
          Err(e) => {
            let resp = super::protocol::Response::error(0, -32700, format!("parse error: {e}"));
            let mut resp_json = serde_json::to_string(&resp).unwrap();
            resp_json.push('\n');
            let _ = writer.write_all(resp_json.as_bytes());
            continue;
          }
        };

        let resp = handle_request(platform.as_ref(), cache.as_ref(), request);
        let mut resp_json = serde_json::to_string(&resp).unwrap();
        resp_json.push('\n');
        let _ = writer.write_all(resp_json.as_bytes());
      }
    });
  }

  Ok(())
}
