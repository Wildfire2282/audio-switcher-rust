# Audio Switcher · 音频托盘切换器

> 一款 Windows 托盘小工具：右键切换播放设备，中键静音，悬停滚轮调音量。
>
> A tiny Windows tray tool: right-click to switch playback devices, middle-click to mute, hover + scroll to adjust volume.

[中文](#中文) · [English](#english)

---

## 中文

### 这是什么

Audio Switcher 常驻系统托盘，帮你快速在多个音频设备（音箱、耳机、显示器音频等）之间切换，并提供音量上限保护。单文件、无界面打扰，开机可自启。

### 下载安装

1. 前往 [Releases](https://github.com/Wildfire2282/audio-switcher-rust/releases) 下载最新版 `audio-switcher-rust.exe`。
2. 双击运行，托盘区会出现图标。需要 Windows 10 1809 或更高版本。
3. 如需开机自启：右键托盘图标 → 勾选「开机自启」。

### 用法

| 操作 | 效果 |
|---|---|
| 右键托盘图标 | 打开菜单：切换设备、静音、音量上限、打开系统声音面板等 |
| 中键点击托盘图标 | 一键静音 / 取消静音 |
| 鼠标悬停在图标上 + 滚轮 | 调节音量；快速滚动会自动加大步进 |

只有鼠标正停在托盘图标上时滚轮才有效，离开即失效，避免误触改音量。

### 菜单说明

- **设备列表**：当前默认设备打 ✓，点击即切换。
- **全局静音**：一键静音 / 恢复。
- **音量上限**：限制最大音量，防止误触把声音开太大。有关闭、25%、50%、75% 四档。
- **打开音量合成器 / 打开声音设置**：跳转到系统自带面板。
- **开机自启**：登录后自动运行。
- **Language**：中文 / English。
- **关于 / 退出**。

### 设置保存在哪里

设置保存在 `%APPDATA%\AudioSwitcher\config.json`，一般不需要手动修改。删除该文件后重启，程序会自动恢复默认设置。

### 常见问题

- **图标不见了？** Windows 可能把托盘图标收进了溢出区域，点击托盘旁的「∧」把它拖出来即可。
- **切设备没声音？** 部分应用需要重新选择输出设备，或在「打开声音设置」里确认默认设备。
- **只能运行一个**：重复启动不会打开第二个窗口，程序是单实例运行的。

---

## English

### What is it

Audio Switcher lives in your system tray and lets you quickly switch between playback devices (speakers, headphones, monitor audio, …), with a volume cap to protect your ears. Single file, no nagging windows, optional auto-start.

### Download & install

1. Go to [Releases](https://github.com/Wildfire2282/audio-switcher-rust/releases) and download the latest `audio-switcher-rust.exe`.
2. Double-click to run — an icon appears in the tray. Requires Windows 10 1809 or later.
3. To run at login: right-click the tray icon → check “Auto Launch”.

### Usage

| Action | Effect |
|---|---|
| Right-click the tray icon | Open the menu: switch devices, mute, volume limit, system sound panels, … |
| Middle-click the tray icon | Toggle mute |
| Hover over the icon + scroll | Adjust volume; fast scrolling steps up automatically |

Scrolling only works while the cursor is over the tray icon; it stops immediately after leaving, to avoid accidental changes.

### Menu guide

- **Device list**: the current default is checked (✓); click another to switch.
- **Mute**: toggle global mute.
- **Volume Limit**: caps the maximum volume so a stray scroll can’t blast your ears. Off / 25% / 50% / 75%.
- **Open Volume Mixer / Open Sound Settings**: jump to the built-in Windows panels.
- **Auto Launch**: start automatically at login.
- **Language**: 中文 / English.
- **About / Exit**.

### Where are settings stored

Settings live in `%APPDATA%\AudioSwitcher\config.json` — you normally never need to touch it. Delete it and restart to restore defaults.

### FAQ

- **Icon missing?** Windows may have tucked it into the tray overflow; click “∧” next to the tray and drag it out.
- **No sound after switching?** Some apps need their output device re-selected, or confirm the default device via “Open Sound Settings”.
- **Single instance**: launching it twice won’t open a second copy.

---

## 从源码构建 · Build from source

```sh
cargo build --release
```

产物 · Output: `target/release/audio-switcher-rust.exe`

## 许可 · License

MIT
