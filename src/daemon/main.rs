fn main() {
  println!("tokimo-app-computer-daemon v{}", env!("CARGO_PKG_VERSION"));

  #[cfg(windows)]
  {
    // Initialize DPI awareness before anything else
    unsafe {
      let _ = windows::Win32::UI::WindowsAndMessaging::SetProcessDPIAware();
    }
  }

  if let Err(e) = tokimo_app_computer_use::daemon::run_daemon() {
    eprintln!("daemon error: {e}");
    std::process::exit(1);
  }
}
