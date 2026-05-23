# tokimo-app-computer-use

Rust 实现的 Windows 桌面自动化工具包，提供 CLI 和 Daemon 两种模式。

## 功能

- **鼠标控制** — 移动、点击、拖拽
- **键盘控制** — 按键输入、组合键
- **窗口管理** — 枚举、聚焦、移动、调整大小
- **UI 元素** — 通过 UI Automation 查找和操作控件
- **截图** — GDI 截屏，支持 WebP/PNG/JPEG 编码
- **系统信息** — CPU、GPU、内存、磁盘、网络、电池
- **进程管理** — 列出、启动、终止进程
- **服务管理** — Windows 服务查询与控制
- **注册表** — 读写 Windows 注册表
- **蓝牙** — 设备枚举与信息查询
- **WiFi** — 网络扫描与连接信息
- **USB** — 设备枚举
- **打印机** — 列出打印机与打印任务
- **音频** — 音频设备与音量控制
- **启动项** — 开机自启管理
- **软件** — 已安装软件列表
- **终端** — Terminal/ConPTY 会话管理

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
                                     └───────────────────-┘
```

- **CLI** — 解析命令后通过 named pipe 连接 Daemon（未运行时自动启动）
- **Daemon** — 常驻进程，接受 JSON 请求分派到 platform 实现
- **Library** — 导出 `PlatformProvider` 复合 trait（13+ 子 trait）

## 构建

```bash
cargo build --release
```

## 使用

```bash
# 系统信息
tokimo-app system info

# 鼠标点击
tokimo-app mouse click --x 100 --y 200

# 截图
tokimo-app screenshot --output screen.webp

# 窗口列表
tokimo-app window list
```

## 平台支持

| 平台 | 状态 |
|---|---|
| Windows | 完整实现 |
| macOS | Stub（待实现） |

## 许可证

MIT
