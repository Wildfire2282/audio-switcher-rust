# AudioSwitcher

Windows tray tool to switch the default audio device, toggle mute, and cap the master volume.

Windows 托盘工具：切换默认音频设备、静音开关、主音量上限。

[English](#english) · [中文](#chinese)

---

## English

### Usage

#### Devices

- Pick an **output** or **input** device entry; the checked entry is the current default.
- **Refresh Devices** re-detects devices — use after sleep/resume or replug if the list looks stale.
- A grayed **No audio devices** line appears when nothing is detected at all.

#### Mute & volume

- **Mute** toggles global mute; middle-click the tray icon does the same.
- Hover the tray icon and roll the wheel to adjust volume — accelerates `1%` → `2%` → `5%` while rolling.

#### Volume limit

- The volume-limit submenu caps the master volume at `25` / `50` / `75%`.
- Disable it with **Enabled** for unlimited volume.

#### Hotkeys

All hotkeys are unbound by default. Click **Open Hotkey Settings** (below **Open Sound Settings**) to open the config folder, then edit `hotkeys` in `config.json` — the file header carries bilingual comments with format, actions, and an example. Save and restart to apply.

| Action | Config key | Step |
| --- | --- | --- |
| Toggle mute | `mute` | — |
| Volume up | `volume_up` | `2%` per press |
| Volume down | `volume_down` | `2%` per press |
| Next output device | `next_device` | — |
| Previous output device | `prev_device` | — |

Each key takes a combination string (`null` disables):

```json
{
  "hotkeys": {
    "mute": "Ctrl+Alt+M",
    "volume_up": null
  }
}
```

A combination another program already owns is reported in a dialog and disabled, so the remaining hotkeys keep working.

#### System & settings

- **Volume mixer** / **Sound settings** open the system tools. **Open Hotkey Settings** opens the config folder for manual hotkey editing.
- **Run at startup** toggles login autostart (grayed out while the state cannot be read).
- The language submenu offers **Follow System** / **中文** / **English**.
- **About** opens the release homepage. **Exit** quits (it also releases the hotkeys).

> The tooltip shows device and state on one line (`Speaker - 62%`, or `Speaker - Muted` while muted); the tray icon is slashed when muted. All failures pop an error dialog; success is silent.

### Build

```powershell
cargo build --release
# -> target/release/audio-switcher.exe
```

- Single file, nothing beside it: icons, `VERSIONINFO`, and the DPI manifest are embedded at build time; every dependency is a Rust static library, and the MSVC CRT is linked statically (`.cargo/config.toml`), so no VC++ Redistributable is needed on the target machine.
- `scripts/package.ps1` runs that build and emits the release artifact `dist/audio-switcher-v<version>-x64.exe` with its SHA256, while checking that the image imports only OS DLLs and stays inside the size budget.
- `scripts/smoke.ps1` is the pre-release gate (build, tests, clippy).

| What | Location |
| --- | --- |
| Config | `%APPDATA%\audio-switcher\` |
| Logs | `%LOCALAPPDATA%\audio-switcher\logs\` |

Runtime state lives outside the exe directory.

---

## 中文

### 使用

#### 设备

- 选择**输出**或**输入**设备条目；打勾的即当前默认设备。
- **刷新设备**重新检测设备——睡眠恢复或重插后列表过时可用。
- 未检测到任何设备时，显示灰色**无音频设备**行。

#### 静音与音量

- **静音**切换全局静音；中键点击托盘图标效果相同。
- 悬停托盘图标后滚滚轮调音量——滚动中按 `1%` → `2%` → `5%` 加速。

#### 音量上限

- 音量上限子菜单把主音量封顶在 `25` / `50` / `75%`。
- 用**启用**关闭上限，不再封顶。

#### 全局快捷键

默认无绑定。点击**打开快捷键设置**（在**打开声音设置**之下）打开配置文件夹，再改 `config.json` 里的 `hotkeys`——文件头有中英文注释，写明格式、动作与示例。保存后重启生效。

| 动作 | 配置键 | 步长 |
| --- | --- | --- |
| 静音切换 | `mute` | — |
| 音量加 | `volume_up` | 每次 `2%` |
| 音量减 | `volume_down` | 每次 `2%` |
| 下一个输出设备 | `next_device` | — |
| 上一个输出设备 | `prev_device` | — |

每个键填组合字符串（`null` 即关闭）：

```json
{
  "hotkeys": {
    "mute": "Ctrl+Alt+M",
    "volume_up": null
  }
}
```

被其他程序占用的组合会弹窗报告并保持关闭，其余快捷键照常工作。

#### 系统与设置

- **音量合成器**／**声音设置**打开系统工具。**打开快捷键设置**打开配置文件夹，用于手动改快捷键。
- **开机自启**开关登录自启（读取不到状态时置灰）。
- 语言子菜单提供**跟随系统** / **中文** / **English**。
- **关于**打开 release 主页。**退出**退出（同时释放快捷键）。

> 提示条把设备和状态显示在一行（`Speaker - 62%`，静音时 `Speaker - Muted`）；静音时托盘图标带斜杠。所有失败都弹窗报错，成功则静默。

### 构建

```powershell
cargo build --release
# -> target/release/audio-switcher.exe
```

- 单文件，旁边无任何附带文件：图标、`VERSIONINFO`、DPI manifest 均在构建时嵌入；所有依赖都是 Rust 静态库，MSVC CRT 静态链接（`.cargo/config.toml`），目标机器无需 VC++ 运行库。
- `scripts/package.ps1` 执行该构建并产出 release 工件 `dist/audio-switcher-v<version>-x64.exe`（附 SHA256），同时检查镜像只导入 OS DLL 且体积在预算内。
- `scripts/smoke.ps1` 是发版门禁（构建、测试、clippy）。

| 内容 | 位置 |
| --- | --- |
| 配置 | `%APPDATA%\audio-switcher\` |
| 日志 | `%LOCALAPPDATA%\audio-switcher\logs\` |

运行时状态放在 exe 目录之外。
