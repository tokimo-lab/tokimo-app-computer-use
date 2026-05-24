pub mod cache;
pub mod handler;
pub mod protocol;
pub mod server;

#[cfg(windows)]
pub use server::PIPE_NAME;

use std::sync::Arc;

use crate::create_platform;

/// Entry point for the daemon process.
/// Creates a platform provider and starts the IPC server.
pub fn run_daemon() -> std::io::Result<()> {
  let platform = create_platform();
  let platform = Arc::new(platform);
  let cache = Arc::new(cache::SnapshotCache::new());
  server::run_server(platform, cache)
}
