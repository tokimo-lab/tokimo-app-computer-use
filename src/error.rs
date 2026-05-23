pub use anyhow::{Error, Result};

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
  #[error("element not found: {0}")]
  ElementNotFound(String),
  #[error("matched {0} elements, please add --index or refine --text/--role")]
  AmbiguousMatch(usize),
  #[error("element is disabled: {0}")]
  Disabled(String),
  #[error(
    "Accessibility permission not granted. Open System Settings → Privacy & Security → Accessibility and enable this app"
  )]
  AxPermissionDenied,
  #[error("Screen Recording permission not granted (required for window list/screenshot)")]
  ScreenRecordingPermissionDenied,
}
