use serde::{Deserialize, Serialize};

/// Opaque window handle (platform-specific meaning inside)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowHandle(pub i64);

/// Window information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
  pub hwnd: i64,
  #[serde(rename = "parentHwnd", default)]
  pub parent_hwnd: i64,
  pub title: String,
  #[serde(rename = "className")]
  pub class_name: String,
  #[serde(rename = "processId")]
  pub process_id: u32,
  #[serde(rename = "processName")]
  pub process_name: String,
  #[serde(rename = "isVisible")]
  pub is_visible: bool,
  #[serde(rename = "isMinimized")]
  pub is_minimized: bool,
  #[serde(rename = "isMaximized")]
  pub is_maximized: bool,
  #[serde(rename = "isTopmost")]
  pub is_topmost: bool,
  #[serde(rename = "isToolWindow")]
  pub is_tool_window: bool,
  #[serde(rename = "isLayered")]
  pub is_layered: bool,
  #[serde(rename = "isNoActivate")]
  pub is_no_activate: bool,
  pub width: i32,
  pub height: i32,
  pub x: i32,
  pub y: i32,
}

/// Internal window rectangle
#[derive(Debug, Clone)]
pub struct WindowRECT {
  pub x: i32,
  pub y: i32,
  pub w: i32,
  pub h: i32,
}

/// Element position information
#[derive(Debug, Clone)]
pub struct ElementPosition {
  pub left: i32,
  pub top: i32,
  pub right: i32,
  pub bottom: i32,
  pub center_x: i32,
  pub center_y: i32,
  pub relative_center_x: i32,
  pub relative_center_y: i32,
  pub window_width: i32,
  pub window_height: i32,
}

/// Unified input operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputResult {
  pub success: bool,
  pub error: Option<String>,
  pub screen_x: i32,
  pub screen_y: i32,
  pub relative_x: i32,
  pub relative_y: i32,
}

impl InputResult {
  pub fn success(screen_x: i32, screen_y: i32, relative_x: i32, relative_y: i32) -> Self {
    Self {
      success: true,
      error: None,
      screen_x,
      screen_y,
      relative_x,
      relative_y,
    }
  }

  pub fn fail(error: String) -> Self {
    Self {
      success: false,
      error: Some(error),
      screen_x: 0,
      screen_y: 0,
      relative_x: 0,
      relative_y: 0,
    }
  }

  pub fn fail_with_coords(error: String, screen_x: i32, screen_y: i32, relative_x: i32, relative_y: i32) -> Self {
    Self {
      success: false,
      error: Some(error),
      screen_x,
      screen_y,
      relative_x,
      relative_y,
    }
  }
}

/// Normalized input position (0.0-1.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputPosition {
  pub x: f64,
  pub y: f64,
}

/// Screenshot options trait
pub trait ScreenshotOptions {
  fn get_quality(&self) -> u8;
  fn get_format(&self) -> &str;
}

/// Screenshot configuration
#[derive(Debug, Clone)]
pub struct ScreenshotConfig {
  pub left: Option<i32>,
  pub top: Option<i32>,
  pub right: Option<i32>,
  pub bottom: Option<i32>,
  pub quality: Option<u8>,
  pub format: Option<String>,
}

impl ScreenshotConfig {
  pub fn get_rect(&self) -> Option<(i32, i32, i32, i32)> {
    if let (Some(l), Some(t), Some(r), Some(b)) = (self.left, self.top, self.right, self.bottom) {
      Some((l, t, r, b))
    } else {
      None
    }
  }

  pub fn new() -> Self {
    Self::default()
  }
}

impl ScreenshotOptions for ScreenshotConfig {
  fn get_quality(&self) -> u8 {
    self.quality.unwrap_or(100)
  }
  fn get_format(&self) -> &str {
    self.format.as_deref().unwrap_or("png")
  }
}

impl Default for ScreenshotConfig {
  fn default() -> Self {
    Self {
      left: None,
      top: None,
      right: None,
      bottom: None,
      quality: Some(80),
      format: Some("webp".to_string()),
    }
  }
}

/// Window screenshot configuration
#[derive(Debug, Clone)]
pub struct WindowScreenshotConfig {
  pub quality: Option<u8>,
  pub format: Option<String>,
}

impl WindowScreenshotConfig {
  pub fn new() -> Self {
    Self::default()
  }
}

impl ScreenshotOptions for WindowScreenshotConfig {
  fn get_quality(&self) -> u8 {
    self.quality.unwrap_or(80)
  }
  fn get_format(&self) -> &str {
    self.format.as_deref().unwrap_or("webp")
  }
}

impl Default for WindowScreenshotConfig {
  fn default() -> Self {
    Self {
      quality: Some(80),
      format: Some("webp".to_string()),
    }
  }
}

/// Mouse button type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
  Left,
  Right,
  Middle,
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
  pub locale_name: String,
  pub ui_language: String,
  pub major_version: u32,
  pub minor_version: u32,
  pub build_number: u32,
  pub platform_id: u32,
  pub version_string: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub gpus: Vec<GpuInfo>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub audio_devices: Vec<AudioDeviceInfo>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub wifi_networks: Vec<WifiNetworkInfo>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub startup_entries: Vec<StartupEntry>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub printers: Vec<PrinterInfo>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub usbs: Vec<UsbDeviceInfo>,
}

/// GPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub driver_version: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub provider_name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub driver_date: Option<String>,
  #[serde(default)]
  pub vendor_id: u32,
  #[serde(default)]
  pub device_id: u32,
  #[serde(default)]
  pub dedicated_video_memory: u64,
  #[serde(default)]
  pub shared_system_memory: u64,
  #[serde(default)]
  pub vram_bytes: u64,
  #[serde(default)]
  pub is_software: bool,
  #[serde(default)]
  pub is_remote: bool,
}

/// Audio device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
  pub name: String,
  pub device_id: String,
  pub device_type: String,
  #[serde(default)]
  pub is_default: bool,
  #[serde(default)]
  pub volume: u32,
  #[serde(default)]
  pub muted: bool,
}

/// WiFi network information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiNetworkInfo {
  pub ssid: String,
  #[serde(default)]
  pub signal_quality: u32,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub bssid: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub auth_type: Option<String>,
  #[serde(default)]
  pub is_connected: bool,
}

/// Startup entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupEntry {
  pub name: String,
  pub command: String,
  pub location: String,
}

/// Installed software information (from registry Uninstall keys)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareInfo {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub version: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub publisher: Option<String>,
  #[serde(rename = "installLocation", skip_serializing_if = "Option::is_none")]
  pub install_location: Option<String>,
  #[serde(rename = "uninstallString", skip_serializing_if = "Option::is_none")]
  pub uninstall_string: Option<String>,
  #[serde(rename = "installDate", skip_serializing_if = "Option::is_none")]
  pub install_date: Option<String>,
  #[serde(rename = "estimatedSizeKB", skip_serializing_if = "Option::is_none")]
  pub estimated_size_kb: Option<u32>,
}

/// Printer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterInfo {
  pub name: String,
  pub driver: String,
  pub port: String,
  pub is_default: bool,
  pub is_shared: bool,
}

/// USB device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDeviceInfo {
  pub name: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub description: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub manufacturer: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub vid: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub pid: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub serial_number: String,
}

/// Bluetooth device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothDeviceInfo {
  pub name: String,
  pub address: String,
  #[serde(default)]
  pub is_connected: bool,
  #[serde(default)]
  pub is_paired: bool,
  pub source: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub rssi: Option<i16>,
}

/// Windows service information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
  pub name: String,
  pub display_name: String,
  pub status: String,
  pub service_type: String,
}

/// Terminal command result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalResult {
  pub exit_code: i32,
  pub stdout: String,
  pub stderr: String,
}

/// Key code enum (platform-agnostic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyCode {
  A,
  B,
  C,
  D,
  E,
  F,
  G,
  H,
  I,
  J,
  K,
  L,
  M,
  N,
  O,
  P,
  Q,
  R,
  S,
  T,
  U,
  V,
  W,
  X,
  Y,
  Z,
  Digit0,
  Digit1,
  Digit2,
  Digit3,
  Digit4,
  Digit5,
  Digit6,
  Digit7,
  Digit8,
  Digit9,
  F1,
  F2,
  F3,
  F4,
  F5,
  F6,
  F7,
  F8,
  F9,
  F10,
  F11,
  F12,
  F13,
  F14,
  F15,
  F16,
  F17,
  F18,
  F19,
  F20,
  F21,
  F22,
  F23,
  F24,
  Numpad0,
  Numpad1,
  Numpad2,
  Numpad3,
  Numpad4,
  Numpad5,
  Numpad6,
  Numpad7,
  Numpad8,
  Numpad9,
  NumpadMultiply,
  NumpadAdd,
  NumpadSeparator,
  NumpadSubtract,
  NumpadDecimal,
  NumpadDivide,
  Ctrl,
  LCtrl,
  RCtrl,
  Shift,
  LShift,
  RShift,
  Alt,
  LAlt,
  RAlt,
  Win,
  LWin,
  RWin,
  Left,
  Up,
  Right,
  Down,
  ArrowLeft,
  ArrowUp,
  ArrowRight,
  ArrowDown,
  Home,
  End,
  PageUp,
  PageDown,
  Insert,
  Enter,
  Return,
  Tab,
  Escape,
  Esc,
  Space,
  Spacebar,
  Backspace,
  Back,
  Delete,
  Del,
  CapsLock,
  Caps,
  NumLock,
  ScrollLock,
  PrintScreen,
  PrtScr,
  Pause,
  Break,
  Apps,
  Menu,
  Semicolon,
  Equals,
  Comma,
  Minus,
  Period,
  Slash,
  Backtick,
  LeftBracket,
  Backslash,
  RightBracket,
  Quote,
  VolumeUp,
  VolumeDown,
  VolumeMute,
  MediaNextTrack,
  MediaPrevTrack,
  MediaStop,
  MediaPlayPause,
  BrowserBack,
  BrowserForward,
  BrowserRefresh,
  BrowserStop,
  BrowserSearch,
  BrowserFavorites,
  BrowserHome,
  LaunchMail,
  LaunchMediaSelect,
  LaunchApp1,
  LaunchApp2,
  Clear,
  Select,
  Print,
  Execute,
  Help,
  Sleep,
  LeftMouse,
  RightMouse,
  MiddleMouse,
  XButton1,
  XButton2,
}
