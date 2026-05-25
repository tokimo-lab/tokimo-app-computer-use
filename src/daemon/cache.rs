//! In-memory snapshot ref store for the agent-friendly `find` → `[ref=eN]` → `click --ref eN` flow.
//!
//! Uses a two-tier resolution strategy (browser pattern):
//! 1. Fast path: match by `stable_id` (AX path on macOS, RuntimeId on Windows)
//! 2. Fallback: match by role + name + nth + distance

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::Context;

use crate::error::Result;
use crate::platform::{Element, PlatformProvider};
use crate::types::{ElementQuery, ElementScope};

#[derive(Debug, Clone)]
pub struct RefEntry {
  pub stable_id: String,
  pub role: String,
  pub name: String,
  pub nth: Option<usize>,
  pub x: i32,
  pub y: i32,
  pub width: i32,
  pub height: i32,
}

/// Tracks role:name counts for dedup (assigns nth only when duplicates exist).
struct RoleNameTracker {
  counts: HashMap<String, usize>,
}

impl RoleNameTracker {
  fn new() -> Self {
    Self { counts: HashMap::new() }
  }

  fn assign_nth(&mut self, role: &str, name: &str) -> Option<usize> {
    let key = format!("{}:{}", role, name);
    let count = self.counts.entry(key).or_insert(0);
    *count += 1;
    if *count > 1 {
      Some(*count - 1) // 0-based nth for duplicates
    } else {
      None // first occurrence has no nth
    }
  }
}

pub struct SnapshotCache {
  lock: Mutex<()>,
  scope: Mutex<Option<ElementScope>>,
  refs: Mutex<HashMap<String, RefEntry>>,
}

impl SnapshotCache {
  pub fn new() -> Self {
    Self {
      lock: Mutex::new(()),
      scope: Mutex::new(None),
      refs: Mutex::new(HashMap::new()),
    }
  }

  /// Replace the in-memory snapshot with descriptors for the given elements.
  /// Returns the generated ref ids in the same order as the input.
  pub fn replace(&self, scope: ElementScope, elements: &[Box<dyn Element>]) -> Vec<String> {
    let _g = self.lock.lock().unwrap();
    let mut ids = Vec::with_capacity(elements.len());
    let mut tracker = RoleNameTracker::new();
    let mut ref_map = HashMap::new();

    for (i, e) in elements.iter().enumerate() {
      let id = format!("e{}", i + 1);
      ids.push(id.clone());

      let role = e.control_type();
      let name = e.name();
      let stable_id = e.stable_id();
      let nth = tracker.assign_nth(&role, &name);

      ref_map.insert(
        id,
        RefEntry {
          stable_id,
          role,
          name,
          nth,
          x: e.x(),
          y: e.y(),
          width: e.width(),
          height: e.height(),
        },
      );
    }

    *self.refs.lock().unwrap() = ref_map;
    *self.scope.lock().unwrap() = Some(scope);
    ids
  }

  /// Look up `ref_id` and re-resolve the live element using two-tier resolution:
  /// 1. Fast path: match by stable_id
  /// 2. Fallback: match by role + name + nth + distance
  pub fn resolve<P: PlatformProvider + ?Sized>(
    &self,
    platform: &P,
    ref_id: &str,
  ) -> Result<(ElementScope, Box<dyn Element>)> {
    let scope = self
      .scope
      .lock()
      .unwrap()
      .clone()
      .ok_or_else(|| anyhow::anyhow!("no snapshot cached — run `element find` first"))?;

    let entry = self
      .refs
      .lock()
      .unwrap()
      .get(ref_id)
      .cloned()
      .ok_or_else(|| anyhow::anyhow!("ref '{ref_id}' not in snapshot; run `element find` first"))?;

    // Re-query the saved scope
    let q = ElementQuery {
      role: if entry.role.is_empty() || entry.role == "?" {
        None
      } else {
        Some(entry.role.clone())
      },
      text: if !entry.name.is_empty() {
        Some(entry.name.clone())
      } else {
        None
      },
      text_exact: false,
      index: None,
      max_depth: None,
      include_hidden: true,
      no_hit_test: false,
    };

    let mut candidates = platform.query_elements(scope.clone(), &q)?;

    // Fast path: match by stable_id
    if !entry.stable_id.is_empty() {
      if let Some(pos) = candidates.iter().position(|c| c.stable_id() == entry.stable_id) {
        return Ok((scope, candidates.swap_remove(pos)));
      }
    }

    // Fallback: match by role + name + nth + distance
    if candidates.is_empty() {
      anyhow::bail!("ref '{ref_id}' could not be re-resolved (UI changed?) — re-run `element find`");
    }

    // Filter by nth if applicable
    let filtered: Vec<_> = if entry.nth.is_some() {
      candidates
        .into_iter()
        .filter(|c| {
          let role = c.control_type();
          let name = c.name();
          role == entry.role && name == entry.name
        })
        .collect()
    } else {
      candidates
    };

    if filtered.is_empty() {
      anyhow::bail!("ref '{ref_id}' could not be re-resolved (UI changed?) — re-run `element find`");
    }

    // Pick by nth or closest center distance
    let target_cx = entry.x as f64 + entry.width as f64 / 2.0;
    let target_cy = entry.y as f64 + entry.height as f64 / 2.0;

    let best = if let Some(nth) = entry.nth {
      filtered.into_iter().nth(nth).unwrap()
    } else {
      let mut best_idx = 0usize;
      let mut best_dist = f64::MAX;
      for (i, c) in filtered.iter().enumerate() {
        let cx = c.x() as f64 + c.width() as f64 / 2.0;
        let cy = c.y() as f64 + c.height() as f64 / 2.0;
        let dist = (cx - target_cx).powi(2) + (cy - target_cy).powi(2);
        if dist < best_dist {
          best_dist = dist;
          best_idx = i;
        }
      }
      filtered.into_iter().nth(best_idx).unwrap()
    };

    Ok((scope, best))
  }
}

impl Default for SnapshotCache {
  fn default() -> Self {
    Self::new()
  }
}
