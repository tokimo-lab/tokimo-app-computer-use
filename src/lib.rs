#![warn(clippy::all)]

pub mod cli;
pub mod error;
pub mod platform;
pub mod types;

#[cfg(windows)]
pub mod daemon;

#[cfg(windows)]
pub use daemon::protocol;

pub use error::{Error, Result};
pub use platform::*;
pub use types::*;

/// Create a platform-specific provider for the current OS.
///
/// On Windows, returns a `WindowsPlatform` instance with full UI Automation support.
/// Other platforms can implement the same traits to provide cross-platform support.
#[cfg(windows)]
pub fn create_platform() -> impl PlatformProvider + Send + Sync {
  platform::windows::WindowsPlatform::new()
}
