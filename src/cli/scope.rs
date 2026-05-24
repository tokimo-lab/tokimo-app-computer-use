use clap::Args;
use serde_json::{Value, json};

#[derive(Args, Debug, Default, Clone)]
pub(crate) struct WindowSel {
  /// Window handle (numeric HWND/NSWindow)
  #[arg(long = "window", short = 'w', value_name = "HANDLE")]
  pub handle: Option<i64>,
  /// App name or bundle ID (uses first matching window)
  #[arg(long = "app", short = 'a', value_name = "NAME")]
  pub app: Option<String>,
}

impl WindowSel {
  pub fn to_json_scope(&self) -> Value {
    if let Some(h) = self.handle {
      json!({"handle": h})
    } else if let Some(ref name) = self.app {
      json!({"app": name})
    } else {
      json!({})
    }
  }
}
