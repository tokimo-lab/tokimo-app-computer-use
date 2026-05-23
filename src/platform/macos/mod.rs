use crate::error::Result;
use crate::platform::*;
use crate::types::*;

pub struct MacPlatform;

impl Default for MacPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl MacPlatform {
    pub fn new() -> Self {
        Self
    }
}

macro_rules! not_impl {
    () => {
        Err(anyhow::anyhow!("not yet implemented for macOS"))
    };
}

// === MouseControl ===
impl MouseControl for MacPlatform {
    fn move_cursor(&self, _x: i32, _y: i32) -> Result<()> {
        not_impl!()
    }
    fn get_cursor_position(&self) -> Result<(i32, i32)> {
        not_impl!()
    }
    fn click(&self, _handle: &WindowHandle, _x: f64, _y: f64, _button: MouseButton, _double_click: bool) -> Result<InputResult> {
        not_impl!()
    }
    fn click_by_xpath(&self, _handle: &WindowHandle, _xpath: &str, _button: MouseButton, _double_click: bool) -> Result<InputResult> {
        not_impl!()
    }
    fn drag(&self, _handle: &WindowHandle, _from_x: f64, _from_y: f64, _to_x: f64, _to_y: f64, _button: MouseButton) -> Result<InputResult> {
        not_impl!()
    }
    fn scroll(&self, _handle: &WindowHandle, _x: f64, _y: f64, _delta_x: i32, _delta_y: i32) -> Result<InputResult> {
        not_impl!()
    }
}

// === KeyboardControl ===
impl KeyboardControl for MacPlatform {
    fn type_text(&self, _handle: &WindowHandle, _text: &str, _position: Option<&InputPosition>) -> Result<InputResult> {
        not_impl!()
    }
    fn type_text_by_xpath(&self, _handle: &WindowHandle, _xpath: &str, _text: &str) -> Result<InputResult> {
        not_impl!()
    }
    fn type_text_raw(&self, _handle: &WindowHandle, _text: &str) -> Result<()> {
        not_impl!()
    }
    fn send_keys(&self, _keys: &[KeyCode], _modifiers: Option<&[KeyCode]>) -> Result<()> {
        not_impl!()
    }
    fn key_down(&self, _key: KeyCode) -> Result<()> {
        not_impl!()
    }
    fn key_release(&self, _key: KeyCode) -> Result<()> {
        not_impl!()
    }
}

// === WindowManager ===
impl WindowManager for MacPlatform {
    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        not_impl!()
    }
    fn list_visible_windows(&self) -> Result<Vec<WindowInfo>> {
        not_impl!()
    }
    fn find_windows_by_title(&self, _pattern: &str, _process_name: Option<&str>) -> Result<Vec<WindowInfo>> {
        not_impl!()
    }
    fn find_window_by_title(&self, _title: &str) -> Result<WindowHandle> {
        not_impl!()
    }
    fn get_windows_by_process_id(&self, _pid: u32) -> Result<Vec<WindowInfo>> {
        not_impl!()
    }
    fn get_windows_by_process_id_with_title(&self, _pid: u32, _pattern: &str, _fuzzy: bool) -> Result<Vec<WindowInfo>> {
        not_impl!()
    }
    fn get_child_windows(&self, _parent: &WindowHandle) -> Result<Vec<WindowInfo>> {
        not_impl!()
    }
    fn get_window_title(&self, _handle: &WindowHandle) -> Result<String> {
        not_impl!()
    }
    fn focus_window(&self, _handle: &WindowHandle) -> Result<()> {
        not_impl!()
    }
    fn move_window(&self, _handle: &WindowHandle, _x: i32, _y: i32) -> Result<()> {
        not_impl!()
    }
    fn resize_window(&self, _handle: &WindowHandle, _width: i32, _height: i32) -> Result<()> {
        not_impl!()
    }
    fn set_window_rect(&self, _handle: &WindowHandle, _x: i32, _y: i32, _width: i32, _height: i32) -> Result<()> {
        not_impl!()
    }
    fn minimize_window(&self, _handle: &WindowHandle) -> Result<()> {
        not_impl!()
    }
    fn maximize_window(&self, _handle: &WindowHandle) -> Result<()> {
        not_impl!()
    }
    fn restore_window(&self, _handle: &WindowHandle) -> Result<()> {
        not_impl!()
    }
    fn get_foreground_window(&self) -> Result<WindowHandle> {
        not_impl!()
    }
}

// === ElementFinder ===
impl ElementFinder for MacPlatform {
    fn find_elements_by_xpath(&self, _handle: &WindowHandle, _xpath: &str) -> Result<Vec<Box<dyn Element>>> {
        not_impl!()
    }
    fn find_first_element_by_xpath(&self, _handle: &WindowHandle, _xpath: &str) -> Result<Box<dyn Element>> {
        not_impl!()
    }
}

// === UiTreeInspector ===
impl UiTreeInspector for MacPlatform {
    fn get_page_source(&self, _handle: &WindowHandle) -> Result<String> {
        not_impl!()
    }
}

// === ScreenCapture ===
impl ScreenCapture for MacPlatform {
    fn take_desktop_screenshot(&self, _config: Option<&ScreenshotConfig>) -> Result<Vec<u8>> {
        not_impl!()
    }
    fn take_window_screenshot(&self, _handle: &WindowHandle, _config: Option<&ScreenshotConfig>) -> Result<Vec<u8>> {
        not_impl!()
    }
}

// === ProcessManager ===
impl ProcessManager for MacPlatform {
    fn launch_app(&self, _path: &str, _wait_timeout_ms: u32) -> Result<u32> {
        not_impl!()
    }
    fn terminate_app(&self, _pid: u32) -> Result<bool> {
        not_impl!()
    }
    fn terminate_apps_by_name(&self, _name: &str) -> Result<(u32, u32)> {
        not_impl!()
    }
    fn get_process_ids_by_name(&self, _name: &str) -> Result<Vec<u32>> {
        not_impl!()
    }
    fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        not_impl!()
    }
    fn get_process_info(&self, _pid: u32) -> Result<ProcessInfo> {
        not_impl!()
    }
}

// === TerminalExecutor ===
impl TerminalExecutor for MacPlatform {
    fn execute_command(&self, _shell_type: &str, _command: &str) -> Result<TerminalResult> {
        not_impl!()
    }
}

// === SystemInfoProvider ===
impl SystemInfoProvider for MacPlatform {
    fn get_system_info(&self) -> Result<SystemInfo> {
        Ok(SystemInfo {
            computer_name: "macOS".to_string(),
            username: std::env::var("USER").unwrap_or_default(),
            os_version: std::env::consts::OS.to_string(),
            locale: String::new(),
            ui_language: String::new(),
            cpu: CpuInfo { name: String::new(), cores: 0, logical_processors: 0 },
            memory: MemoryInfo { total_bytes: 0, available_bytes: 0, used_bytes: 0, usage_percent: 0 },
            disks: Vec::new(),
            networks: Vec::new(),
            screen_width: 0,
            screen_height: 0,
            battery: None,
            gpus: Vec::new(),
            usbs: Vec::new(),
            bluetooth_devices: Vec::new(),
            wifi_networks: Vec::new(),
            audio_devices: Vec::new(),
            printers: Vec::new(),
            services: Vec::new(),
            startup_entries: Vec::new(),
        })
    }
    fn get_screen_size(&self) -> Result<(i32, i32)> {
        not_impl!()
    }
    fn list_printers(&self) -> Result<Vec<PrinterInfo>> {
        not_impl!()
    }
    fn print_document(&self, _file_path: &str, _printer_name: &str) -> Result<()> {
        not_impl!()
    }
}

// === ServiceManager ===
impl ServiceManager for MacPlatform {
    fn list_services(&self) -> Result<Vec<ServiceInfo>> {
        not_impl!()
    }
    fn get_service_detail(&self, _name: &str) -> Result<ServiceInfo> {
        not_impl!()
    }
    fn start_service(&self, _name: &str) -> Result<()> {
        not_impl!()
    }
    fn stop_service(&self, _name: &str) -> Result<()> {
        not_impl!()
    }
}

// === BluetoothManager ===
impl BluetoothManager for MacPlatform {
    fn scan_bluetooth_devices(&self) -> Result<Vec<BluetoothDeviceInfo>> {
        not_impl!()
    }
    fn scan_bluetooth_ble(&self, _duration_ms: u64) -> Result<Vec<BluetoothDeviceInfo>> {
        not_impl!()
    }
    fn list_pnp_bluetooth(&self) -> Result<Vec<BluetoothDeviceInfo>> {
        not_impl!()
    }
}
