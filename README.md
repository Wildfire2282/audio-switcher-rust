# 音频托盘管理工具

> Audio Switcher Rust — Windows 托盘音频切换器，纯 Rust 实现，右键切换设备、中键静音、悬停滚轮调音量。单实例、PerMonitorV2 DPI、原子化配置持久化。

## 系统要求
- Windows 10 1809+ / Windows 11
- MSRV 1.85（`rust-version` 声明于 `Cargo.toml`，Edition 2024）

## 安装
```sh
cargo build --release
# 产物：target/release/audio-switcher-rust.exe（已 strip + LTO，约 346KB）
```
或直接运行 `cargo run --release`（`#![windows_subsystem = "windows"]` 无控制台）。

## 使用
- **右键托盘**：设备列表（✓ 为当前默认）、全局静音、音量上限（启用/25%/50%/自定义）、打开音量合成器/声音设置、开机自启、Language、关于、退出
- **中键托盘**：切换静音
- **悬停 + 滚轮**：调音量（快速滚动步进 5%）；离开托盘 2.5s 内仍响应

## 配置
`%APPDATA%\AudioSwitcher\config.json`（`LazyLock` 缓存路径，原子写入 `.tmp.<nanos>-<cnt>` 后 `rename`）

| 字段 | 类型 | 默认 | 说明 |
|---|---|---|---|
| `version` | `u32` | `1` | 迁移版本 |
| `lang` | `"zh"`\|`"en"` | `"zh"` | 强类型 `Lang` 枚举，大小写不敏感 |
| `volume_limit_enabled` | `bool` | `true` | 是否限幅 |
| `volume_limit` | `1..=100` | `25` | 限幅阈值 |
| `autostart` | `bool` | `true` | 开机自启（`auto-launch`，`CurrentUser`） |

损坏 JSON → 回落默认值并重写；`volume_limit` 越界 → 重置 25。

## 开发
```sh
cargo test            # 23 passed + 1 ignored + 3 doctests
cargo clippy          # correctness=deny，pedantic=warn（FFI 噪声已 allow）
cargo fmt --check
cargo build --release
```
`scripts/smoke.ps1` 覆盖 build/test/manifest 检查。

## 结构
```text
.
├── Cargo.toml
├── build.rs              # embed_manifest + winres app.ico
├── audio-switcher-rust.manifest  # PerMonitorV2
├── icons/                # app.ico/app.svg + 2×.rgba（按需加载，LazyLock 缓存）
├── src/
│   ├── lib.rs            # 库入口（含 prelude 重导出，crate 文档 = README）
│   ├── main.rs           # 仅 SingleInstanceGuard + ComGuard + App::new
│   ├── config.rs         # Lang/AppConfig/clamp_volume
│   ├── prelude.rs        # use audio_switcher_rust::prelude::*
│   ├── app/              # App + AppBuilder + handler::MenuAction
│   ├── audio/            # AudioBackend trait + RealBackend/MockBackend + AudioSnapshot
│   ├── platform/         # com/hook/shell/autostart/instance/dialog
│   └── ui/               # tray/menu/tooltip/wheel/icon/i18n
└── target/
```

库/二进制分离（`proj-lib-main-split`）、`pub(crate)` 收敛内部 API、`thiserror` 统一错误、`OnceLock`/`LazyLock` 按需选择。

## 许可
MIT — 见 `Cargo.toml` `license`
