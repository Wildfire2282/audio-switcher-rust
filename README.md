# AudioSwitcher

Windows tray tool to switch the default audio device, toggle mute, and cap
the master volume.

## Use

- Pick an output or input device entry; the checked entry is the current
  default. **Refresh Devices** re-detects devices (use after sleep/resume or replug
  if the list looks stale).
- **Mute** toggles global mute; middle-click the tray icon does the same.
  Left-click the tray icon, then roll the wheel within 3 seconds to adjust
  volume (accelerates 1% → 2% → 5% while rolling; hovering without clicking
  never changes the volume).
- The volume-limit submenu caps the master volume at 25/50/75%; disable it
  with **Enabled** for unlimited volume.
- **Volume mixer** / **Sound settings** open the system tools. **Run at
  startup** toggles login autostart (grayed out while the state cannot be
  read). The language submenu offers Follow System / 中文 / English.
- **About** opens the release homepage. **Exit** quits.

The tooltip shows device, volume, and mute state on one line; the tray icon
is slashed when muted. All failures pop an error dialog; success is silent.
Logs live under `%LOCALAPPDATA%\audio-switcher\logs\`.
