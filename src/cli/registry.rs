use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::CommandExecutor;

#[derive(Subcommand, Debug)]
pub enum RegistryAction {
  /// Read a registry value (omit --name for Default value)
  Get {
    /// Key path, e.g. "HKLM\SOFTWARE\Microsoft\Windows"
    key: String,
    /// Value name (omit for Default)
    #[arg(long)]
    name: Option<String>,
  },
  /// List subkeys under a key
  Subkeys {
    /// Key path
    key: String,
  },
  /// List values under a key
  Values {
    /// Key path
    key: String,
  },
  /// Set a registry value
  Set {
    /// Key path
    key: String,
    /// Value name
    #[arg(long)]
    name: String,
    /// Value type: REG_SZ, REG_DWORD, REG_QWORD, REG_BINARY, REG_EXPAND_SZ, REG_MULTI_SZ
    #[arg(long)]
    r#type: String,
    /// Value data
    #[arg(long)]
    data: String,
  },
  /// Create a registry key
  CreateKey {
    /// Key path
    key: String,
  },
  /// Delete a registry value
  DeleteValue {
    /// Key path
    key: String,
    /// Value name to delete
    #[arg(long)]
    name: String,
  },
  /// Delete a registry key (must be empty)
  DeleteKey {
    /// Key path
    key: String,
  },
}

pub fn cmd(executor: &mut dyn CommandExecutor, action: RegistryAction) -> Result<()> {
  match action {
    RegistryAction::Get { key, name } => {
      let mut params = json!({"key_path": key});
      if let Some(n) = &name {
        params["value_name"] = json!(n);
      }
      let r = executor.call("registry.read", params)?;
      super::kv_print(&[
        ("Type:", r["type"].as_str().unwrap_or("?")),
        ("Value:", r["value"].as_str().unwrap_or("")),
      ]);
    }
    RegistryAction::Subkeys { key } => {
      let r = executor.call("registry.list_subkeys", json!({"key_path": key}))?;
      if let Some(arr) = r.as_array() {
        for s in arr {
          if let Some(name) = s.as_str() {
            println!("{name}");
          }
        }
      }
    }
    RegistryAction::Values { key } => {
      let r = executor.call("registry.list_values", json!({"key_path": key}))?;
      if let Some(arr) = r.as_array() {
        for s in arr {
          if let Some(name) = s.as_str() {
            println!("{name}");
          }
        }
      }
    }
    RegistryAction::Set {
      key,
      name,
      r#type,
      data,
    } => {
      executor.call(
        "registry.set_value",
        json!({"key_path": key, "value_name": name, "value_type": r#type, "data": data}),
      )?;
      println!("ok");
    }
    RegistryAction::CreateKey { key } => {
      executor.call("registry.create_key", json!({"key_path": key}))?;
      println!("ok");
    }
    RegistryAction::DeleteValue { key, name } => {
      executor.call("registry.delete_value", json!({"key_path": key, "value_name": name}))?;
      println!("ok");
    }
    RegistryAction::DeleteKey { key } => {
      executor.call("registry.delete_key", json!({"key_path": key}))?;
      println!("ok");
    }
  }
  Ok(())
}
