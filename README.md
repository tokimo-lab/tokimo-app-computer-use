# tokimo-app-computer-use

Cross-platform desktop automation toolkit built in Rust. Provides both a CLI and a background Daemon for controlling mouse, keyboard, windows, UI elements, screenshots, and system information.

## Features

- **Mouse Control** — move, click, drag, scroll
- **Keyboard Control** — type text, send key combinations
- **Window Management** — enumerate, focus, move, resize
- **UI Elements** — find and interact with controls via XPath
- **Screenshot** — desktop and per-window capture (WebP/PNG/JPEG)
- **System Info** — CPU, GPU, memory, disk, network, battery
- **Process Management** — list, launch, terminate processes
- **Service Management** — query and control OS services
- **Registry** — read/write Windows Registry *(Windows only)*
- **Bluetooth** — classic and BLE device scanning
- **WiFi** — network scanning and connection info
- **USB** — device enumeration
- **Printer** — list printers and print documents
- **Audio** — volume control and device management
- **Startup** — manage startup items
- **Software** — list installed software
- **Terminal** — shell command execution

## Architecture

```
┌─────────────┐    IPC (named pipe / direct)  ┌──────────────────┐
│  CLI (clap) │ ─────────────────────────────>│  Daemon (bg proc) │
│ src/main.rs │                               │  daemon/main.rs   │
└─────────────┘                               └───────┬──────────┘
                                                      │
                                            ┌─────────▼──────────┐
                                            │  PlatformProvider   │
                                            │  (14 sub-traits)    │
                                            ├─────────────────────┤
                                            │ MacPlatform   │ WindowsPlatform │
                                            │ (macOS)       │ (Windows)       │
                                            └─────────────────────┘
```

- **CLI** — parses commands, then connects to the Daemon via named pipe (Windows) or calls the platform directly (macOS/Linux)
- **Daemon** — long-running process that maintains a platform instance and dispatches JSON-RPC requests
- **Library** — exports the `PlatformProvider` composite trait (14 sub-traits), data types, and IPC protocol definitions

## Building

```bash
cargo build --release
```

## Usage

```bash
# System info
tokimo-app system info

# Mouse click at coordinates
tokimo-app mouse click --x 100 --y 200

# Take a screenshot
tokimo-app screenshot --output screen.webp

# List windows
tokimo-app window list

# List processes
tokimo-app process list
```

## Platform Support

| Platform | Status | Notes |
|---|---|---|
| Windows | Full | All features including Registry, Services, Bluetooth |
| macOS | Full | Registry not available; some platform-specific differences |
| Linux | Planned | Not yet implemented |

## License

MIT
