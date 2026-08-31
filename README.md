# Audio Switcher Rust

Windows 托盘音频切换器 — 纯 Rust，右键切换设备、中键静音、悬停滚轮调音量。

## 特性
- 托盘图标与菜单（`tray-icon` + `muda`）
- WASAPI 设备枚举/切换/音量/静音
- 滚轮加速、音量上限、开机自启
- 原子化配置持久化（`%APPDATA%\AudioSwitcher\config.json`）

## 结构
```
src/
  lib.rs        # 库入口（proj-lib-main-split）
  main.rs       # 最小二进制入口
  config.rs     # 配置与 Lang 强类型
  app/          # 运行时状态与消息循环
  audio/        # AudioBackend 抽象（Real/Mock）
  platform/     # COM / Hook / Shell / Autostart
  ui/           # Tray / Menu / Tooltip / Wheel / Icon / i18n
  prelude.rs    # 常用重导出
icons/          # app.ico + 2× rgba 托盘图标
```

## 构建
```sh
cargo build --release   # 产物 target/release/audio-switcher-rust.exe
cargo test
cargo clippy
```

## 配置
- 路径：`%APPDATA%\AudioSwitcher\config.json`
- 字段：`lang` (`zh`/`en`)、`volume_limit_enabled`、`volume_limit` (1..100)、`wheel_acceleration`、`autostart`

## 许可
MIT
