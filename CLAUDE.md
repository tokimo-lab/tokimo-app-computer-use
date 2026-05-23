# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

`tokimo-app-computer-use` — Rust 实现的 Windows 桌面自动化工具包，提供 CLI 和 Daemon 两种模式，控制鼠标、键盘、窗口、UI 元素、截屏及系统信息查询。

## 构建与测试

```bash
cargo build                    # debug 构建
cargo build --release          # release 构建（LTO + strip symbols）
cargo test                     # 运行全部测试
cargo test --test cli_e2e      # 运行单个测试文件
cargo test -- --test-threads=1 # 串行运行（UI 自动化测试需要）
cargo fmt                      # 格式化代码
```

> 测试需要真实的 Windows 环境和 GUI 可用（Calculator 等应用会被启动）。CI 中需确保桌面会话存在。

## 架构

```
┌─────────────┐    named pipe IPC     ┌──────────────────┐
│  CLI (clap) │ ──────────────────────>│  Daemon (后台进程) │
│ src/main.rs │  \\.\pipe\tokimo-app   │  daemon/main.rs  │
└─────────────┘    JSON-RPC 协议       └───────┬──────────┘
                                               │
                                     ┌─────────▼─────────┐
                                     │ WindowsPlatform    │
                                     │ (PlatformProvider) │
                                     ├───────────────────-┤
                                     │ MouseControl       │
                                     │ KeyboardControl    │
                                     │ WindowManager      │
                                     │ Element / Finder   │
                                     │ ScreenCapture      │
                                     │ ProcessManager     │
                                     │ SystemInfoProvider │
                                     │ BluetoothProvider  │
                                     │ ServiceProvider    │
                                     │ RegistryProvider   │
                                     │ ...                │
                                     └───────────────────-┘
```

- **CLI** — 解析命令后通过 named pipe 连接 Daemon（如 Daemon 未运行则自动启动），非 Windows 时回退到 `DirectExecutor` 直接调用
- **Daemon** — 常驻进程，维护 `WindowsPlatform` 实例，接受 JSON 请求分派到 platform trait 方法
- **Library** (`lib.rs`) — 导出 `platform::PlatformProvider` 复合 trait（组合 13+ 子 trait）、数据类型、错误类型、IPC 协议定义

### IPC 协议

JSON 换行分隔，请求格式：`{ "id": "xxx", "method": "mouse.click", "params": {...} }`
响应格式：`{ "id": "xxx", "result": {...} }` 或 `{ "id": "xxx", "error": "..." }`

## 代码规范

- Rust Edition 2024，`rustfmt.toml` 配置：2 空格缩进、120 字符宽度、item 级别 import 整理
- 错误处理统一用 `anyhow`（`src/error.rs` re-export）
- 数据类型定义在 `src/types.rs`，CLI 和 platform 共享
- 新增 platform 功能：在 `src/platform/mod.rs` 添加 trait → `src/platform/windows/` 实现 → `src/cli/` 添加子命令 → `src/daemon/handler.rs` 注册 method 路由

## API 文档参考

### Windows API

| 资源 | 链接 |
|---|---|
| Win32 API 总览 | https://learn.microsoft.com/en-us/windows/win32/api/ |
| UI Automation | https://learn.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32 |
| SendInput (键盘鼠标) | https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput |
| GDI 截屏 | https://learn.microsoft.com/en-us/windows/win32/gdi/capturing-an-image |
| Windows Registry | https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry |
| Windows Services | https://learn.microsoft.com/en-us/windows/win32/services/services |
| Bluetooth API | https://learn.microsoft.com/en-us/windows/win32/bluetooth/bluetooth-start-page |
| DXGI (GPU 枚举) | https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/d3d10-graphics-programming-guide-dxgi |
| `windows` crate 文档 | https://microsoft.github.io/windows-docs-rs/ |
| `windows` crate 源码 | https://github.com/microsoft/windows-rs |

### macOS API (目前为 stub 实现)

| 资源 | 链接 |
|---|---|
| AppKit (窗口管理) | https://developer.apple.com/documentation/appkit |
| Accessibility API | https://developer.apple.com/documentation/applicationaccessibility |
| Core Graphics (截屏) | https://developer.apple.com/documentation/coregraphics |
| IOKit (硬件信息) | https://developer.apple.com/documentation/iokit |
| CGEvent (输入模拟) | https://developer.apple.com/documentation/coregraphics/cgevent |
| Apple 官方文档总览 | https://developer.apple.com/documentation/technologies |

## 关键依赖

| crate | 用途 |
|---|---|
| `windows` 0.62 | Windows API 绑定（28 个 feature flag） |
| `clap` 4 | CLI 参数解析 |
| `serde` / `serde_json` | IPC 协议序列化 |
| `image` / `webp` | 截图编码（默认 WebP） |
| `anyhow` | 错误处理 |
| `tokimo-bus-client` | Tokimo 认证总线 |
