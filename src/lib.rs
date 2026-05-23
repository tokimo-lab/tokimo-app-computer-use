#![warn(clippy::all)]

pub mod cli;
pub mod daemon;
pub mod error;
pub mod platform;
pub mod types;

pub use daemon::protocol;
pub use error::{Error, Result};
pub use platform::*;
pub use types::*;

/// Create a platform-specific provider for the current OS.
#[cfg(windows)]
pub fn create_platform() -> impl PlatformProvider + Send + Sync {
  platform::windows::WindowsPlatform::new()
}

/// Create a platform-specific provider for the current OS.
#[cfg(target_os = "macos")]
pub fn create_platform() -> impl PlatformProvider + Send + Sync {
  platform::macos::MacPlatform::new()
}
