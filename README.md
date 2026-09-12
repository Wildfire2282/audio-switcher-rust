# AudioSwitcher

Windows tray tool to switch the default audio device, toggle mute, and cap
the master volume.

## Use

- Pick an output or input device entry; the checked entry is the current
  default. **Refresh Devices** re-detects devices (use after sleep/resume or replug
  if the list looks stale). A grayed **No audio devices** line appears when
  nothing is detected at all.
- **Mute** toggles global mute; middle-click the tray icon does the same.
  Hover the tray icon and roll the wheel to adjust volume (accelerates
  1% → 2% → 5% while rolling).
- The volume-limit submenu caps the master volume at 25/50/75%; disable it
  with **Enabled** for unlimited volume.
- The **Hotkeys** submenu binds one global shortcut per action (all off by
  default): toggle mute, volume up/down (2% per press), next/previous output
  device. Checked = bound to the combination shown in the label
  (`Ctrl+Alt+M`, `Ctrl+Alt+Up`, `Ctrl+Alt+Down`, `Ctrl+Alt+Right`,
  `Ctrl+Alt+Left`). A combination another program already owns is reported in
  a dialog and left disabled, so the remaining hotkeys keep working. Put your
  own combinations in `hotkeys` in `config.json` (for example
  `"mute": "Ctrl+Shift+F9"`, `null` to disable).
- **Volume mixer** / **Sound settings** open the system tools. **Run at
  startup** toggles login autostart (grayed out while the state cannot be
  read). The language submenu offers Follow System / 中文 / English.
- **About** opens the release homepage. **Exit** quits (it also releases the
  hotkeys).

The tooltip shows the device and state on one line (`Speaker - 62%`, or
`Speaker - Muted` while muted); the tray icon is slashed when muted. All
failures pop an error dialog; success is silent. Logs live under
`%LOCALAPPDATA%\audio-switcher\logs\`.

## Build

- `cargo build --release` → `target/release/audio-switcher.exe`. One file with
  nothing beside it: icons, VERSIONINFO and the DPI manifest are embedded at
  build time, every dependency is a Rust static library, and the MSVC CRT is
  linked statically (`.cargo/config.toml`) so no VC++ Redistributable is
  needed on the target machine.
- `scripts/package.ps1` runs that build and emits the release artifact
  `dist/audio-switcher-v<version>-x64.exe` with its SHA256, while checking
  that the image imports only OS DLLs and stays inside the size budget.
- `scripts/smoke.ps1` is the pre-release gate (build, tests, clippy).
- Runtime state lives outside the exe directory: config in
  `%APPDATA%\audio-switcher\`, logs in `%LOCALAPPDATA%\audio-switcher\logs\`.
