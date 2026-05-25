// Thin shim — all logic lives in lib.rs as `pub fn run(args)`.
// Allows the same code to be invoked either as a standalone binary or
// embedded into another crate (e.g. tokimo-app-computer-use).
fn main() {
  agent_browser::run(std::env::args().collect());
}
