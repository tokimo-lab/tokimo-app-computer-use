mod audio;
mod battery;
mod bluetooth;
mod element;
mod gpu;
mod keyboard;
mod mouse;
mod printer;
mod process;
mod registry;
mod scope;
mod screenshot;
mod service;
mod software;
mod startup;
mod system;
mod terminal;
mod usb;
mod wifi;
mod window;

use std::cmp::max;
use std::fmt::Write as _;

use std::io::{BufRead, BufReader, Write};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use serde_json::Value;
use tokimo_bus_cli::TokimoAuthArgs;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

// ── Command Executor abstraction ──

pub(crate) trait CommandExecutor {
  fn call(&mut self, method: &str, params: Value) -> Result<Value>;
}

// ── IpcClient: IPC via named pipe (Windows) or Unix socket (macOS/Linux) ──

struct IpcClient {
  reader: BufReader<Box<dyn std::io::Read + Send>>,
  writer: Box<dyn std::io::Write + Send>,
  next_id: u64,
}

impl IpcClient {
  #[cfg(windows)]
  fn connect(pipe_name: &str) -> Result<Self> {
    let file = std::fs::OpenOptions::new()
      .read(true)
      .write(true)
      .open(pipe_name)
      .with_context(|| format!("cannot connect to daemon at {pipe_name}"))?;

    let reader_file = file.try_clone()?;
    let reader = BufReader::new(Box::new(reader_file));

    Ok(Self {
      reader,
      writer: Box::new(file),
      next_id: 1,
    })
  }

  #[cfg(unix)]
  fn connect(socket_path: &str) -> Result<Self> {
    use std::os::unix::net::UnixStream;

    let stream = UnixStream::connect(socket_path)
      .with_context(|| format!("cannot connect to daemon at {socket_path}"))?;

    let reader_stream = stream.try_clone()?;
    let reader = BufReader::new(Box::new(reader_stream) as Box<dyn std::io::Read + Send>);

    Ok(Self {
      reader,
      writer: Box::new(stream) as Box<dyn std::io::Write + Send>,
      next_id: 1,
    })
  }
}

impl CommandExecutor for IpcClient {
  fn call(&mut self, method: &str, params: Value) -> Result<Value> {
    use crate::daemon::protocol::{Request, Response};

    let id = self.next_id;
    self.next_id += 1;

    let req = Request {
      id,
      method: method.to_string(),
      params,
    };
    let mut req_json = serde_json::to_string(&req)?;
    req_json.push('\n');
    self.writer.write_all(req_json.as_bytes())?;

    let mut line = String::new();
    self.reader.read_line(&mut line)?;
    if line.is_empty() {
      return Err(anyhow!("daemon disconnected"));
    }

    let resp: Response = serde_json::from_str(&line)?;
    if let Some(err) = resp.error {
      return Err(anyhow!("[{}] {}", err.code, err.message));
    }
    Ok(resp.result.unwrap_or(Value::Null))
  }
}

#[cfg(windows)]
fn try_connect_or_spawn_daemon() -> Result<IpcClient> {
  use crate::daemon::PIPE_NAME;

  if let Ok(client) = IpcClient::connect(PIPE_NAME) {
    return Ok(client);
  }

  let daemon_running = is_daemon_running();

  if !daemon_running {
    spawn_daemon_background()?;
  }

  for _ in 0..50 {
    std::thread::sleep(std::time::Duration::from_millis(100));
    if let Ok(client) = IpcClient::connect(PIPE_NAME) {
      return Ok(client);
    }
  }

  Err(anyhow!("daemon pipe not available after 5s — check daemon logs"))
}

#[cfg(windows)]
fn spawn_daemon_background() -> Result<()> {
  let exe = std::env::current_exe()?;
  let exe_dir = exe.parent().context("no parent dir for exe")?;
  let daemon_path = exe_dir.join("tokimo-app-computer-daemon.exe");

  if !daemon_path.exists() {
    return Err(anyhow!("daemon binary not found at {}", daemon_path.display()));
  }

  std::process::Command::new(&daemon_path)
    .creation_flags(0x08000000) // CREATE_NO_WINDOW
    .spawn()
    .context("failed to spawn daemon")?;

  Ok(())
}

#[cfg(windows)]
fn is_daemon_running() -> bool {
  use windows::Win32::Foundation::CloseHandle;
  use windows::Win32::System::Diagnostics::ToolHelp::*;
  let target = "tokimo-app-computer-daemon.exe";
  unsafe {
    let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
      return false;
    };
    let mut entry = PROCESSENTRY32W {
      dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
      ..Default::default()
    };
    if Process32FirstW(snapshot, &mut entry).is_ok() {
      loop {
        let name = String::from_utf16_lossy(&entry.szExeFile)
          .trim_end_matches('\0')
          .to_string();
        if name.eq_ignore_ascii_case(target) {
          let _ = CloseHandle(snapshot);
          return true;
        }
        if Process32NextW(snapshot, &mut entry).is_err() {
          break;
        }
      }
    }
    let _ = CloseHandle(snapshot);
  }
  false
}

#[cfg(unix)]
fn try_connect_or_spawn_daemon() -> Result<IpcClient> {
  use crate::daemon::server::SOCKET_PATH;

  if let Ok(client) = IpcClient::connect(SOCKET_PATH) {
    return Ok(client);
  }

  spawn_daemon_background()?;

  for _ in 0..50 {
    std::thread::sleep(std::time::Duration::from_millis(100));
    if let Ok(client) = IpcClient::connect(SOCKET_PATH) {
      return Ok(client);
    }
  }

  Err(anyhow!("daemon socket not available after 5s — check daemon logs"))
}

#[cfg(unix)]
fn spawn_daemon_background() -> Result<()> {
  let exe = std::env::current_exe()?;
  let daemon_path = exe.with_file_name("tokimo-app-computer-daemon");

  if !daemon_path.exists() {
    return Err(anyhow!("daemon binary not found at {}", daemon_path.display()));
  }

  std::process::Command::new(&daemon_path)
    .spawn()
    .context("failed to spawn daemon")?;

  Ok(())
}

// ── Daemon management ──

#[derive(Subcommand, Debug)]
pub enum DaemonAction {
  /// Start the daemon process
  Start,
  /// Stop the daemon process
  Stop,
  /// Show daemon status
  Status,
}

#[cfg(unix)]
fn daemon_pid() -> Option<u32> {
  use std::process::Command;
  // Use -x for exact match on process name (not arguments)
  let output = Command::new("pgrep")
    .arg("-x")
    .arg("tokimo-app-computer-daemon")
    .output()
    .ok()?;
  let stdout = String::from_utf8_lossy(&output.stdout);
  // Get the first PID, but skip our own process if somehow matched
  let self_pid = std::process::id();
  for line in stdout.trim().lines() {
    if let Ok(pid) = line.parse::<u32>() {
      if pid != self_pid {
        return Some(pid);
      }
    }
  }
  None
}

#[cfg(unix)]
fn is_daemon_running() -> bool {
  daemon_pid().is_some()
}

fn handle_daemon_action(action: DaemonAction) -> Result<()> {
  match action {
    DaemonAction::Start => {
      if is_daemon_running() {
        println!("Daemon is already running (PID: {})", daemon_pid().unwrap());
        return Ok(());
      }
      spawn_daemon_background()?;
      // Wait for socket to appear
      for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if is_daemon_running() {
          println!("Daemon started (PID: {})", daemon_pid().unwrap());
          return Ok(());
        }
      }
      Err(anyhow!("daemon failed to start within 5s"))
    }
    DaemonAction::Stop => {
      if !is_daemon_running() {
        println!("Daemon is not running");
        return Ok(());
      }
      let pid = daemon_pid().unwrap();
      #[cfg(unix)]
      {
        std::process::Command::new("kill")
          .arg(pid.to_string())
          .status()
          .context("failed to kill daemon")?;
      }
      #[cfg(windows)]
      {
        std::process::Command::new("taskkill")
          .args(["/PID", &pid.to_string(), "/F"])
          .status()
          .context("failed to kill daemon")?;
      }
      println!("Daemon stopped (PID: {pid})");
      Ok(())
    }
    DaemonAction::Status => {
      if let Some(pid) = daemon_pid() {
        println!("Daemon is running (PID: {pid})");
      } else {
        println!("Daemon is not running");
      }
      Ok(())
    }
  }
}

// ── CLI definition ──

#[derive(Parser, Debug)]
#[command(
  name = "tokimo-app-computer-use",
  about = "Desktop automation CLI — mouse, keyboard, windows, elements, screenshots, system",
  long_about = "Tokimo Computer Use CLI — Desktop automation.\n\nControl mouse, keyboard, windows, UI elements, and more via platform-specific backends.",
  term_width = 100
)]
pub struct Cli {
  #[command(flatten)]
  pub auth: TokimoAuthArgs,
  #[command(subcommand)]
  pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
  /// Daemon process management
  Daemon {
    #[command(subcommand)]
    action: DaemonAction,
  },
  /// Window management
  Window {
    #[command(subcommand)]
    action: window::WindowAction,
  },
  /// Mouse control
  Mouse {
    #[command(subcommand)]
    action: mouse::MouseAction,
  },
  /// Keyboard control
  Keyboard {
    #[command(subcommand)]
    action: keyboard::KeyboardAction,
  },
  /// UI element inspection
  Element {
    #[command(subcommand)]
    action: element::ElementAction,
  },
  /// Screenshot capture
  Screenshot {
    #[command(subcommand)]
    action: screenshot::ScreenshotAction,
  },
  /// Process management
  Process {
    #[command(subcommand)]
    action: process::ProcessAction,
  },
  /// System info
  System {
    #[command(subcommand)]
    action: system::SystemAction,
  },
  /// Execute terminal commands
  Term {
    #[command(subcommand)]
    action: terminal::TermAction,
  },
  /// USB device management
  Usb {
    #[command(subcommand)]
    action: usb::UsbAction,
  },
  /// Printer management
  Printer {
    #[command(subcommand)]
    action: printer::PrinterAction,
  },
  /// Bluetooth device management
  Bluetooth {
    #[command(subcommand)]
    action: bluetooth::BluetoothAction,
  },
  /// WiFi network management
  Wifi {
    #[command(subcommand)]
    action: wifi::WifiAction,
  },
  /// Audio device management
  Audio {
    #[command(subcommand)]
    action: audio::AudioAction,
  },
  /// Windows service management
  Service {
    #[command(subcommand)]
    action: service::ServiceAction,
  },
  /// Startup entry management
  Startup {
    #[command(subcommand)]
    action: startup::StartupAction,
  },
  /// GPU information
  Gpu {
    #[command(subcommand)]
    action: gpu::GpuAction,
  },
  /// Battery status
  Battery {
    #[command(subcommand)]
    action: battery::BatteryAction,
  },
  /// Windows registry operations
  Registry {
    #[command(subcommand)]
    action: registry::RegistryAction,
  },
  /// Installed software
  Software {
    #[command(subcommand)]
    action: software::SoftwareAction,
  },
  /// Browser automation via vendored agent-browser (CDP)
  ///
  /// All args after `browser` are passed through verbatim to the bundled
  /// `agent-browser` binary. Run `tokimo-app-computer-use browser --help`
  /// to see its full command set.
  #[command(trailing_var_arg = true, allow_hyphen_values = true, disable_help_flag = true)]
  Browser {
    /// Args forwarded to agent-browser (e.g. `open https://example.com`).
    #[arg(value_name = "ARGS")]
    args: Vec<String>,
  },
}

// ── CLI entry point ──

pub fn run_cli(cli: Cli) -> Result<()> {
  if cli.auth.token.is_some() {
    let _credentials = tokimo_bus_cli::Credentials::resolve(&cli.auth)?;
  }

  let command = match cli.command {
    Some(cmd) => cmd,
    None => {
      use clap::CommandFactory;
      let mut cmd = Cli::command();
      cmd.print_help()?;
      return Ok(());
    }
  };

  // Daemon management — does not need our daemon
  if let Command::Daemon { action } = command {
    return handle_daemon_action(action);
  }

  // Browser passthrough — does not need our daemon
  if let Command::Browser { args } = command {
    return run_browser_passthrough(args);
  }

  let mut executor: Box<dyn CommandExecutor> = Box::new(try_connect_or_spawn_daemon()?);

  match command {
    Command::Window { action } => window::cmd(&mut *executor, action),
    Command::Mouse { action } => mouse::cmd(&mut *executor, action),
    Command::Keyboard { action } => keyboard::cmd(&mut *executor, action),
    Command::Element { action } => element::cmd(&mut *executor, action),
    Command::Screenshot { action } => screenshot::cmd(&mut *executor, action),
    Command::Process { action } => process::cmd(&mut *executor, action),
    Command::System { action } => system::cmd(&mut *executor, action),
    Command::Term { action } => terminal::cmd(&mut *executor, action),
    Command::Usb { action } => usb::cmd(&mut *executor, action),
    Command::Printer { action } => printer::cmd(&mut *executor, action),
    Command::Bluetooth { action } => bluetooth::cmd(&mut *executor, action),
    Command::Wifi { action } => wifi::cmd(&mut *executor, action),
    Command::Audio { action } => audio::cmd(&mut *executor, action),
    Command::Service { action } => service::cmd(&mut *executor, action),
    Command::Startup { action } => startup::cmd(&mut *executor, action),
    Command::Gpu { action } => gpu::cmd(&mut *executor, action),
    Command::Battery { action } => battery::cmd(&mut *executor, action),
    Command::Registry { action } => registry::cmd(&mut *executor, action),
    Command::Software { action } => software::cmd(&mut *executor, action),
    Command::Daemon { .. } => unreachable!("Daemon handled above"),
    Command::Browser { .. } => unreachable!("Browser handled above"),
  }
}

// ── Browser passthrough ──
//
// Invokes the vendored agent-browser library directly (no subprocess), and
// injects sensible defaults so the user gets:
//   - a visible browser window (--headed) instead of headless
//   - a persistent profile directory (--profile <fixed-path>) so logins / cookies survive across runs
// We also block the `install` and `upgrade` subcommands — agent-browser ships a
// Chrome-for-Testing downloader and a self-updater, neither of which makes
// sense when shipped inside tokimo-app-computer-use.

fn run_browser_passthrough(args: Vec<String>) -> Result<()> {
  // Block dangerous / nonsensical subcommands.
  if let Some(sub) = args.iter().find(|a| !a.starts_with('-')) {
    match sub.as_str() {
      "install" => {
        return Err(anyhow!(
          "`browser install` is disabled in tokimo-app-computer-use (we do not auto-download Chrome). \
           Install Chrome / Edge / Brave via your OS package manager or vendor website."
        ));
      }
      "upgrade" => {
        return Err(anyhow!(
          "`browser upgrade` is disabled in tokimo-app-computer-use. \
           Upgrades happen via tokimo-app-computer-use releases."
        ));
      }
      _ => {}
    }
  }

  // Inject defaults if user did not provide them.
  let has_flag = |name: &str| args.iter().any(|a| a == name || a.starts_with(&format!("{name}=")));
  let mut injected: Vec<String> = Vec::new();

  if !has_flag("--headed") && std::env::var("AGENT_BROWSER_HEADED").is_err() {
    injected.push("--headed".to_string());
  }

  if !has_flag("--profile") && std::env::var("AGENT_BROWSER_PROFILE").is_err() {
    let profile_dir = default_browser_profile_dir()?;
    if !profile_dir.exists() {
      std::fs::create_dir_all(&profile_dir)
        .with_context(|| format!("failed to create default browser profile dir {}", profile_dir.display()))?;
    }
    injected.push("--profile".to_string());
    injected.push(profile_dir.to_string_lossy().into_owned());
  }

  // agent_browser::run expects argv-style: argv[0] is the program name and is skipped internally.
  let mut full = Vec::with_capacity(1 + injected.len() + args.len());
  full.push("agent-browser".to_string());
  full.extend(injected);
  full.extend(args);

  agent_browser::run(full);
  Ok(())
}

fn default_browser_profile_dir() -> Result<std::path::PathBuf> {
  let base = dirs::home_dir().context("cannot resolve home directory")?;
  Ok(base.join(".tokimo-browser").join("profile"))
}

// ── Common helpers ──

pub(crate) fn format_bytes(bytes: u64) -> String {
  if bytes >= 1_073_741_824 {
    format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
  } else if bytes >= 1_048_576 {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
  } else if bytes >= 1024 {
    format!("{:.1} KB", bytes as f64 / 1024.0)
  } else {
    format!("{bytes} B")
  }
}

pub(crate) fn print_input_result(value: &Value) {
  let success = value["success"].as_bool().unwrap_or(true);
  if !success {
    println!("fail");
    return;
  }
  if let Some(pos) = value.get("position") {
    let x = pos["x"].as_f64().unwrap_or(0.0);
    let y = pos["y"].as_f64().unwrap_or(0.0);
    if x.fract() == 0.0 && y.fract() == 0.0 {
      println!("ok {},{}", x as i64, y as i64);
    } else {
      println!("ok {x},{y}");
    }
  } else {
    println!("ok");
  }
}

// ── Unified table printing ──

const TABLE_PAD: &str = "  ";
const TABLE_SEP: char = '-';

pub(crate) struct Column {
  header: &'static str,
  width: usize,
  align_right: bool,
}

pub(crate) struct Table {
  columns: Vec<Column>,
  rows: Vec<Vec<String>>,
}

impl Table {
  pub fn new(columns: Vec<(&'static str, usize)>) -> Self {
    Self {
      columns: columns
        .into_iter()
        .map(|(h, w)| Column {
          header: h,
          width: w,
          align_right: false,
        })
        .collect(),
      rows: Vec::new(),
    }
  }

  pub fn align_right(mut self, col: usize) -> Self {
    if col < self.columns.len() {
      self.columns[col].align_right = true;
    }
    self
  }

  pub fn row(&mut self, values: Vec<String>) {
    self.rows.push(values);
  }

  pub fn print(&self) {
    if self.columns.is_empty() {
      return;
    }

    // Header
    let mut hdr_line = String::new();
    let mut sep_line = String::new();
    for (ci, col) in self.columns.iter().enumerate() {
      if ci > 0 {
        hdr_line.push_str(TABLE_PAD);
        sep_line.push_str(TABLE_PAD);
      }
      let _ = write!(hdr_line, "{:<width$}", col.header, width = col.width);
      let sep_w = max(col.header.len(), col.width);
      for _ in 0..sep_w {
        sep_line.push(TABLE_SEP);
      }
    }
    println!("{hdr_line}");
    println!("{sep_line}");

    // Rows
    for row in &self.rows {
      let mut line = String::new();
      for (ci, col) in self.columns.iter().enumerate() {
        if ci > 0 {
          line.push_str(TABLE_PAD);
        }
        let val = row.get(ci).map(|s| s.as_str()).unwrap_or("");
        if col.align_right {
          let _ = write!(line, "{:>width$}", val, width = col.width);
        } else {
          let _ = write!(line, "{:<width$}", val, width = col.width);
        }
      }
      println!("{line}");
    }
  }
}

pub(crate) fn kv_print(pairs: &[(&str, &str)]) {
  let max_label = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
  for (k, v) in pairs {
    println!("{:<width$} {}", k, v, width = max_label);
  }
}
