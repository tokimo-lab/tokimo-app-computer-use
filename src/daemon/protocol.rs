use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
  pub id: u64,
  pub method: String,
  #[serde(default)]
  pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
  pub id: u64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub result: Option<serde_json::Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<ErrorInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorInfo {
  pub code: i32,
  pub message: String,
}

impl Response {
  pub fn success(id: u64, result: serde_json::Value) -> Self {
    Self {
      id,
      result: Some(result),
      error: None,
    }
  }

  pub fn error(id: u32, code: i32, message: String) -> Self {
    Self {
      id: id as u64,
      result: None,
      error: Some(ErrorInfo { code, message }),
    }
  }
}
