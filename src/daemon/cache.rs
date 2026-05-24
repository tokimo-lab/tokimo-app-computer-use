//! Snapshot ref store for the agent-friendly `find` → `[ref=eN]` → `click --ref eN` flow.
//!
//! Live element handles (AXUIElementRef on macOS, IUIAutomationElement on
//! Windows) cannot be persisted across processes, and the macOS CLI is a fresh
//! process for every invocation. So instead of caching live handles we persist
//! a small descriptor for each element to a JSON file under the user's cache
//! dir, then re-resolve on demand by re-querying the scope and matching the
//! descriptor (role + name + closest center). This gives the same UX as the
//! browser channel (snapshot → ref → action) while staying simple and
//! cross-platform.

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::platform::{Element, PlatformProvider};
use crate::types::{ElementQuery, ElementScope};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementDescriptor {
  pub id: String,
  pub role: String,
  pub name: String,
  pub text: String,
  pub automation_id: String,
  pub class_name: String,
  pub x: i32,
  pub y: i32,
  pub width: i32,
  pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
  pub scope: ElementScope,
  pub refs: Vec<ElementDescriptor>,
}

pub struct SnapshotCache {
  path: PathBuf,
  lock: Mutex<()>,
}

impl SnapshotCache {
  pub fn new() -> Self {
    Self {
      path: default_snapshot_path(),
      lock: Mutex::new(()),
    }
  }

  /// Replace the on-disk snapshot with descriptors for the given elements.
  /// Returns the generated ref ids in the same order as the input.
  pub fn replace(&self, scope: ElementScope, elements: &[Box<dyn Element>]) -> Vec<String> {
    let _g = self.lock.lock().unwrap();
    let mut ids = Vec::with_capacity(elements.len());
    let descriptors: Vec<ElementDescriptor> = elements
      .iter()
      .enumerate()
      .map(|(i, e)| {
        let id = format!("e{}", i + 1);
        ids.push(id.clone());
        ElementDescriptor {
          id,
          role: e.control_type(),
          name: e.name(),
          text: e.text(),
          automation_id: e.automation_id(),
          class_name: e.class_name(),
          x: e.x(),
          y: e.y(),
          width: e.width(),
          height: e.height(),
        }
      })
      .collect();
    let snap = Snapshot {
      scope,
      refs: descriptors,
    };
    if let Some(parent) = self.path.parent() {
      let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(&snap) {
      let _ = std::fs::write(&self.path, json);
    }
    ids
  }

  fn load(&self) -> Result<Snapshot> {
    let _g = self.lock.lock().unwrap();
    let data = std::fs::read_to_string(&self.path).with_context(|| {
      format!(
        "no snapshot found at {} — run `element find` first",
        self.path.display()
      )
    })?;
    let snap: Snapshot = serde_json::from_str(&data).context("snapshot file is corrupted; re-run `element find`")?;
    Ok(snap)
  }

  /// Look up `ref_id` and re-resolve the live element by re-querying the saved
  /// scope and matching the saved descriptor (best by closest center). Returns
  /// the resolved live element and the scope.
  pub fn resolve<P: PlatformProvider + ?Sized>(
    &self,
    platform: &P,
    ref_id: &str,
  ) -> Result<(ElementScope, Box<dyn Element>)> {
    let snap = self.load()?;
    let desc = snap
      .refs
      .iter()
      .find(|r| r.id == ref_id)
      .ok_or_else(|| anyhow::anyhow!("ref '{ref_id}' not in snapshot; run `element find` first"))?
      .clone();

    // Re-query the saved scope with the descriptor as filter. Use automation_id
    // as text filter when available (more unique than name); fall back to name.
    let text_filter = if !desc.automation_id.is_empty() {
      Some(desc.automation_id.clone())
    } else if !desc.name.is_empty() {
      Some(desc.name.clone())
    } else if !desc.text.is_empty() {
      Some(desc.text.clone())
    } else {
      None
    };

    let q = ElementQuery {
      role: if desc.role.is_empty() || desc.role == "?" {
        None
      } else {
        Some(desc.role.clone())
      },
      text: text_filter,
      text_exact: false,
      index: None,
      max_depth: None,
      include_hidden: true,
      no_hit_test: false,
    };

    let mut candidates = platform.query_elements(snap.scope.clone(), &q)?;
    if candidates.is_empty() {
      // Try again without text filter (descriptor may have been auto-id only;
      // some apps emit no name/text on re-query).
      let q2 = ElementQuery {
        role: q.role.clone(),
        text: None,
        text_exact: false,
        index: None,
        max_depth: None,
        include_hidden: true,
        no_hit_test: false,
      };
      candidates = platform.query_elements(snap.scope.clone(), &q2)?;
    }
    if candidates.is_empty() {
      anyhow::bail!("ref '{ref_id}' could not be re-resolved (UI changed?) — re-run `element find`");
    }

    let target_cx = desc.x as f64 + desc.width as f64 / 2.0;
    let target_cy = desc.y as f64 + desc.height as f64 / 2.0;
    let mut best_idx = 0usize;
    let mut best_dist = f64::MAX;
    for (i, c) in candidates.iter().enumerate() {
      let cx = c.x() as f64 + c.width() as f64 / 2.0;
      let cy = c.y() as f64 + c.height() as f64 / 2.0;
      let dist = (cx - target_cx).powi(2) + (cy - target_cy).powi(2);
      if dist < best_dist {
        best_dist = dist;
        best_idx = i;
      }
    }
    Ok((snap.scope, candidates.swap_remove(best_idx)))
  }
}

impl Default for SnapshotCache {
  fn default() -> Self {
    Self::new()
  }
}

fn default_snapshot_path() -> PathBuf {
  let base = dirs::cache_dir()
    .or_else(dirs::data_local_dir)
    .unwrap_or_else(std::env::temp_dir);
  base.join("tokimo-app-computer-use").join("snapshot.json")
}
