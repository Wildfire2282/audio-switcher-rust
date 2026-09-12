# AudioSwitcher

Windows tray tool to switch the default audio device, toggle mute, and cap the master volume.
Windows 托盘工具：切换默认音频设备、静音开关、主音量上限。

## Usage / 使用

- Pick an output or input device entry; the checked entry is the current default. **Refresh Devices** re-detects devices (use after sleep/resume or replug if the list looks stale). A grayed **No audio devices** line appears when nothing is detected at all.
  选择输出或输入设备条目；打勾的即当前默认设备。**Refresh Devices / 刷新设备** 重新检测设备（睡眠恢复或重插后列表过时可用）。未检测到任何设备时显示灰色 **No audio devices / 无音频设备** 行。
- **Mute** toggles global mute; middle-click the tray icon does the same. Hover the tray icon and roll the wheel to adjust volume (accelerates 1% → 2% → 5% while rolling).
  **Mute / 静音** 切换全局静音；中键点击托盘图标效果相同。悬停托盘图标后滚滚轮调音量（滚动中按 1% → 2% → 5% 加速）。
- The volume-limit submenu caps the master volume at 25/50/75%; disable it with **Enabled** for unlimited volume.
  音量上限子菜单把主音量封顶在 25/50/75%；用 **Enabled / 启用** 关闭上限，不再封顶。
- The **Hotkeys** submenu binds one global shortcut per action (all off by default): toggle mute, volume up/down (2% per press), next/previous output device. Checked = bound to the combination shown in the label (`Ctrl+Alt+M`, `Ctrl+Alt+Up`, `Ctrl+Alt+Down`, `Ctrl+Alt+Right`, `Ctrl+Alt+Left`). A combination another program already owns is reported in a dialog and left disabled, so the remaining hotkeys keep working. Put your own combinations in `hotkeys` in `config.json` (for example `"mute": "Ctrl+Shift+F9"`, `null` to disable).
  **Hotkeys / 全局快捷键** 子菜单为每个动作绑定一个全局快捷键（默认全关）：静音切换、音量加/减（每次 2%）、上一个/下一个输出设备。打勾 = 已绑定标签里显示的组合（`Ctrl+Alt+M`、`Ctrl+Alt+Up`、`Ctrl+Alt+Down`、`Ctrl+Alt+Right`、`Ctrl+Alt+Left`）。被其他程序占用的组合会弹窗报告并保持关闭，其余快捷键照常工作。在 `config.json` 的 `hotkeys` 里填自己的组合（例如 `"mute": "Ctrl+Shift+F9"`，`null` 即关闭）。
- **Volume mixer** / **Sound settings** open the system tools. **Run at startup** toggles login autostart (grayed out while the state cannot be read). The language submenu offers Follow System / 中文 / English.
  **Volume mixer / 音量合成器**、**Sound settings / 声音设置** 打开系统工具。**Run at startup / 开机自启** 开关登录自启（读取不到状态时置灰）。语言子菜单提供 跟随系统 / 中文 / English。
- **About** opens the release homepage. **Exit** quits (it also releases the hotkeys).
  **About / 关于** 打开 release 主页。**Exit / 退出** 退出（同时释放快捷键）。

The tooltip shows the device and state on one line (`Speaker - 62%`, or `Speaker - Muted` while muted); the tray icon is slashed when muted. All failures pop an error dialog; success is silent. Logs live under `%LOCALAPPDATA%\audio-switcher\logs\`.
提示条把设备和状态显示在一行（`Speaker - 62%`，静音时 `Speaker - Muted`）；静音时托盘图标带斜杠。所有失败都弹窗报错，成功则静默。日志在 `%LOCALAPPDATA%\audio-switcher\logs\`。

## Build / 构建

- `cargo build --release` → `target/release/audio-switcher.exe`. One file with nothing beside it: icons, VERSIONINFO and the DPI manifest are embedded at build time, every dependency is a Rust static library, and the MSVC CRT is linked statically (`.cargo/config.toml`) so no VC++ Redistributable is needed on the target machine.
  单文件，旁边无任何附带文件：图标、VERSIONINFO、DPI manifest 均在构建时嵌入，所有依赖都是 Rust 静态库，MSVC CRT 静态链接（`.cargo/config.toml`），目标机器无需 VC++ 运行库。
- `scripts/package.ps1` runs that build and emits the release artifact `dist/audio-switcher-v<version>-x64.exe` with its SHA256, while checking that the image imports only OS DLLs and stays inside the size budget.
  `scripts/package.ps1` 执行该构建并产出 release 工件 `dist/audio-switcher-v<version>-x64.exe`（附 SHA256），同时检查镜像只导入 OS DLL 且体积在预算内。
- `scripts/smoke.ps1` is the pre-release gate (build, tests, clippy).
  `scripts/smoke.ps1` 是发版门禁（构建、测试、clippy）。
- Runtime state lives outside the exe directory: config in `%APPDATA%\audio-switcher\`, logs in `%LOCALAPPDATA%\audio-switcher\logs\`.
  运行时状态放在 exe 目录之外：配置在 `%APPDATA%\audio-switcher\`，日志在 `%LOCALAPPDATA%\audio-switcher\logs\`。
