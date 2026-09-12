# AudioSwitcher SPEC — behavior, acceptance, 1.0 contract, exemptions

> Tool SPEC per `docs/unified-scheme.md` §2: user-visible behavior and
> acceptance live here; engineering standards live in the scheme doc and win
> on conflict. Version: crate `0.5.0`, config schema v3.

## 1. Behavior

- Tray icon shows the default render endpoint; slashed glyph when muted.
- Right-click menu (fixed shape): grayed `AudioSwitcher v{ver}` title →
  output devices → input devices → mute toggle → volume-limit
  submenu (Enabled + 25/50/75%) → hotkeys submenu (five opt-in toggles) →
  system tools (volume mixer, sound settings) → Refresh Devices → autostart toggle → language submenu (Follow System / 中文 /
  English) → About → Exit last (scheme-v8 fixed tail, no separators inside).
  When nothing is enumerated, a grayed `No audio devices` line takes the
  device group's place (an empty enumeration must not look like a menu that
  lost its devices).
- Middle-click toggles mute. Hover the tray icon and roll the wheel to
  adjust volume (EarTrumpet's hover-scroll gesture; this tool accelerates
  1% → 2% → 5% while rolling where EarTrumpet steps a flat 2%); the cursor
  must still be over the icon at event time, and leaving the icon resets
  acceleration.
- Volume limiting clamps the master volume to the configured preset whenever
  the default device or volume changes.
- Global hotkeys (EarTrumpet-style shortcuts, opt-in one action at a time):
  toggle mute, volume up/down (2% per press), next/previous output device
  (wrapping through the enumeration order) — defaults `Ctrl+Alt+M` /
  `Ctrl+Alt+Up` / `Ctrl+Alt+Down` / `Ctrl+Alt+Right` / `Ctrl+Alt+Left`, shown
  in the label of each submenu toggle. The hotkeys submenu binds the default
  combination when an action is switched on (checked = bound); a combination
  already owned by another program is reported in one dialog, disabled in the
  config, and the remaining hotkeys stay active; all are released on exit.
  Custom combinations are set by editing the `hotkeys` object in `config.json`
  (canonical `Ctrl+Alt+M` form; an unsupported value disables that action with
  a log warning).
- Autostart: follows `HKCU\...\Run\AudioSwitcher`. Fresh installs with
  `autostart: true` self-heal an explicitly-`Disabled` state in the
  background; an unreadable state grays the menu item (never writes).
- Language defaults to Follow System (startup resolution: Simplified-Chinese
  locales → 中文, Traditional-Chinese locales → English fallback, everything
  else → English). Explicit 中文/English overrides persist.
- About opens the release homepage (`ABOUT_URL`, single const in `lib.rs`).
- Second launch exits silently (0). COM/init failures dialog and exit (1).
  Panics dialog and exit (1); logs land in
  `%LOCALAPPDATA%\audio-switcher\logs\`.
- Config lives at `%APPDATA%\audio-switcher\config.json` (kebab-case dir;
  v1 `AudioSwitcher` dir is imported once, then abandoned). Unknown fields
  reset loudly (timestamped `.bak` + defaults). Dialogs are English.
- Tray tooltip is one line naming the default device: `Device - 62%`, and
  `Device - Muted` (localized) while muted — the device name survives mute
  (EarTrumpet's tray tooltip keeps the device next to the state). With no
  device at all the tooltip degrades to the state alone.

## 2. Acceptance

- `cargo check` green; three greens + deny in CI.
- Packaging gate: `scripts/package.ps1` emits exactly one exe whose imports are
  OS DLLs only (no `vcruntime140`/`ucrtbase`/VC++ Redistributable) and whose
  size stays inside the scheme §6 800 KB single-exe budget; the artifact runs
  from an otherwise empty directory and writes nothing beside itself.
- Grep clean: `parking_lot`, `windows-core`/`windows_core`, `println!`,
  `eprintln!`, `dbg!`, `Result<(), String>`, `WH_CBT`/`SetWinEventHook`
  centering hooks, `open_file(`.
- Unit tests (all in-module, run in the workspace suite): TOOL_ID mapping
  (mutex/autostart/config-dir derivation), autostart key names + Unknown≠off,
  `Url` scheme/host gate, tooltip 64-char budget + device-name-survives-mute
  format, title display-name+version, vol-preset round-trip, hotkey
  round-trip (`hotkey_*` id ⇄ action), hotkey combination parse (canonical
  form, each error class, named/letter/digit keys), hotkey action ids stable
  and reversible, occupied-combo summary lists every conflict, pending-queue
  drain order, hotkey label/check state + binding-change rebuild, hotkey
  i18n coverage for both languages, cycle-index wraparound, volume-step
  clamp (incl. `i32::MIN`/`i32::MAX`), unknown-id warn path, config migration
  (v1 Zh→System, v2 explicit zh survives the v3 bump, unknown-field/
  unknown-lang loud reset, hotkey default-off on v2 files, combo
  canonicalization, invalid combo disables only that action, unknown field
  inside `hotkeys` resets loudly, legacy import, path chain),
  empty-enumeration placeholder, locale mapping branches, menu sync-state
  rebuild matrix, hover wheel gate (cursor over icon at event time, leave
  resets acceleration).
- Real-machine matrix (§6): single screen 100% + mixed-DPI dual screen +
  dark/light × 中文/English smoke + one sleep-resume (manual Refresh covers
  callback loss) + one hotkey pass (each enabled action fires once; an
  occupied combination dialogs once and is disabled). Evidence attached to
  the Release.

## 3. 1.0-contract registry (frozen at 1.0; adding menu items is minor)

| Item | Contract |
|---|---|
| Config format | v3 fields (`version`, `lang`, `volume_limit_enabled`, `volume_limit`/`volumeLimit` alias, `autostart`, `hotkeys`); renames migrate, never silently drop. `hotkeys` holds one optional combo string per action (`null`/absent = off); a v2 file migrates with every hotkey off |
| Menu ids | `title`, `refresh`, `mute`, `vol_enabled`, `vol_N`, `hotkeys`, `hotkey_mute`, `hotkey_volume_up`, `hotkey_volume_down`, `hotkey_next_device`, `hotkey_prev_device`, `no_devices`, `open_mixer`, `open_sound`, `autostart`, `language`, `lang_system`, `lang_zh`, `lang_en`, `about`, `exit`, `device_*`, `input_*` — never rename, never reuse deleted ids |
| Hotkey ids | `RegisterHotKey`/`WM_HOTKEY` ids 1–5 = Mute, VolumeUp, VolumeDown, NextDevice, PrevDevice — stable, never reused; registration happens on the message-pumping thread and is released on exit |
| Default hotkey combos | `Ctrl+Alt+M`, `Ctrl+Alt+Up`, `Ctrl+Alt+Down`, `Ctrl+Alt+Right`, `Ctrl+Alt+Left` (bound by the menu toggles; changing a default combo is a major change) |
| Autostart location | `HKCU\...\Run` value `AudioSwitcher`, CurrentUser mode |
| ABOUT_URL origin | `https://github.com/Wildfire2282/audio-switcher` (== Cargo `repository` origin) |
| Mutex id | `audio-switcher-single-instance-v1` |

## 4. Scheme deltas (0.x, recorded here)

- Config dir `AudioSwitcher` → `audio-switcher` (kebab per cross-tool
  contract); one-time import of the legacy file.
- Config schema v1 → v2: `lang` default `Zh` → `System`; v1 `zh` values
  migrate to `System` (v1 could not separate explicit choice from default).
  The rule is scoped to `version < 2`, so the v3 bump does not re-migrate a
  v2 file's explicit `zh`.
- Config schema v2 → v3: new opt-in `hotkeys` object (five action keys,
  canonical combo strings). A v2 file migrates with every hotkey off. On
  load, combos are canonicalized (`"ctrl+alt+m"` → `"Ctrl+Alt+M"`); an
  unsupported combo disables only that action with a log warning, while an
  unknown field inside `hotkeys` stays a config error (backup + defaults).
- Global hotkeys use `RegisterHotKey(None, …)` on the pumping thread;
  `platform::pump` routes `WM_HOTKEY` (new `pump → hotkey` edge, contract
  comment on the callee side). Occupied combos are collected and reported in
  one dialog (scheme §4) and disabled in the config, so "checked" in the menu
  always means "actually registered".
- Volume hotkeys step 2% (EarTrumpet parity: its absolute-volume shortcuts
  step 2). The wheel keeps its own 1% → 2% → 5% hover acceleration.
- Tray tooltip keeps the device name while muted (`Device - Muted`), matching
  EarTrumpet's `state - device` tooltip; previously the device was dropped.
- Single-file delivery: the MSVC CRT is linked statically
  (`.cargo/config.toml`, `-C target-feature=+crt-static`) so the shipped exe
  runs on a clean Windows install without the VC++ Redistributable and with no
  DLL beside it. Measured cost: 619 KB → 700 KB (still inside the scheme §6
  800 KB single-exe budget). Release artifact: `dist/audio-switcher-v<ver>-x64.exe`
  via `scripts/package.ps1`, which also asserts OS-only imports.
- `tracing-subscriber` is a runtime `[dependencies]` entry, not dev-deps:
  the release binary file sink links it (dev-deps do not link into bins).
- COM callbacks are manual vtables (`ComHeader` + static vtbls in
  `audio/real.rs`): the `implement` macro's `::windows_core` codegen would
  force a direct `windows-core` dep, which the cross-tool contract drops.
  Behavior (push notifications, STA/MTA split) is unchanged.

## 5. §8 exemption registry

| Item | Rationale | Review-by | Owner | On user request |
|---|---|---|---|---|
| Theme bundle (`theme.rs`, `ThemeMode`, dark menus, `ImmersiveColorSet` listener) | Icons are a single monochrome set that does not follow the system; no `theme` field, no listener | 1.0 | audio | implement on request |
| DPI bundle (`dpi.rs`, `pick_size`, `WM_DPICHANGED` coalescing) | Single 32px RGBA icon, system-rendered menus; no blur path to fix | 1.0 | audio | implement on request |
| Locale `intl` listener (live `WM_SETTINGCHANGE` re-resolve) | `Lang::System` resolves once at startup; runtime locale switches apply on restart | 1.0 | audio | implement on request |
| Icon RGBA regeneration (root renderer + light/dark/size matrix) | Current `tray_muted/unmuted.rgba` pair ships as-is; full Lucide pipeline deferred | 1.0 | audio | regenerate on request |
| Volume-level tray glyph (EarTrumpet `Bar0/1/2/3` + mute) | Two-state glyph plus the volume in the tooltip carries the same information; the offline renderer pipeline (§5.3) is not built yet | 1.0 | audio | implement on request |
| `config.rs` split (>500-line file) | Reviewed at 0.5 when hotkeys landed: single cohesive persistence module (Lang, Hotkeys, AppConfig, path chain, migration), hotkeys stay with the fields they serialize — split only when a second consumer needs the pieces | 0.6 | audio | n/a |
| `audio/real.rs` split (>500-line file) | WASAPI backend + manual COM objects stay together; split on next feature | 0.4 | audio | n/a |
| OutputDebugString layer | File sink is the record; debugger channel adds no user value yet | 1.0 | audio | add on request |
| Traditional-Chinese translation | Falls back to English (recorded); add `Lang::ZhHant` on request | 1.0 | audio | translate on request |
