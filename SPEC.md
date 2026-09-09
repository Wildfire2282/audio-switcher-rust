# AudioSwitcher SPEC — behavior, acceptance, 1.0 contract, exemptions

> Tool SPEC per `docs/unified-scheme.md` §2: user-visible behavior and
> acceptance live here; engineering standards live in the scheme doc and win
> on conflict. Version: crate `0.3.0`, config schema v2.

## 1. Behavior

- Tray icon shows the default render endpoint; slashed glyph when muted.
- Right-click menu (fixed shape): grayed `AudioSwitcher v{ver}` title →
  output devices → input devices → mute toggle → volume-limit
  submenu (Enabled + 25/50/75%) → system tools (volume mixer, sound
  settings) → Refresh Devices → autostart toggle → language submenu (Follow System / 中文 /
  English) → About → Exit last (scheme-v8 fixed tail, no separators inside).
- Middle-click toggles mute. Left-click the tray icon, then roll the wheel
  within 3 seconds to adjust volume (acceleration 1% → 2% → 5% while
  rolling); rolling keeps the window open, idling past it needs a fresh
  click. Hover alone never changes the volume, and the cursor must still be
  over the icon at event time.
- Volume limiting clamps the master volume to the configured preset whenever
  the default device or volume changes.
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

## 2. Acceptance

- `cargo check` green; three greens + deny in CI.
- Grep clean: `parking_lot`, `windows-core`/`windows_core`, `println!`,
  `eprintln!`, `dbg!`, `Result<(), String>`, `WH_CBT`/`SetWinEventHook`
  centering hooks, `open_file(`.
- Unit tests (all in-module, run in the workspace suite): TOOL_ID mapping
  (mutex/autostart/config-dir derivation), autostart key names + Unknown≠off,
  `Url` scheme/host gate, tooltip 64-char budget, title display-name+version,
  vol-preset round-trip, unknown-id warn path, config migration (v1 Zh→System,
  unknown-field/unknown-lang loud reset, legacy import, path chain),
  locale mapping branches, menu sync-state rebuild matrix, left-click
  wheel-arm window (hover never arms, stale clicks expire).
- Real-machine matrix (§6): single screen 100% + mixed-DPI dual screen +
  dark/light × 中文/English smoke + one sleep-resume (manual Refresh covers
  callback loss). Evidence attached to the Release.

## 3. 1.0-contract registry (frozen at 1.0; adding menu items is minor)

| Item | Contract |
|---|---|
| Config format | v2 fields (`version`, `lang`, `volume_limit_enabled`, `volume_limit`/`volumeLimit` alias, `autostart`); renames migrate, never silently drop |
| Menu ids | `title`, `refresh`, `mute`, `vol_enabled`, `vol_N`, `open_mixer`, `open_sound`, `autostart`, `language`, `lang_system`, `lang_zh`, `lang_en`, `about`, `exit`, `device_*`, `input_*` — never rename, never reuse deleted ids |
| Autostart location | `HKCU\...\Run` value `AudioSwitcher`, CurrentUser mode |
| ABOUT_URL origin | `https://github.com/Wildfire2282/audio-switcher` (== Cargo `repository` origin) |
| Mutex id | `audio-switcher-single-instance-v1` |

## 4. Scheme deltas (0.x, recorded here)

- Config dir `AudioSwitcher` → `audio-switcher` (kebab per cross-tool
  contract); one-time import of the legacy file.
- Config schema v1 → v2: `lang` default `Zh` → `System`; v1 `zh` values
  migrate to `System` (v1 could not separate explicit choice from default).
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
| `config.rs` split (>500-line file) | Single cohesive persistence module; split on next feature | 0.4 | audio | n/a |
| `audio/real.rs` split (>500-line file) | WASAPI backend + manual COM objects stay together; split on next feature | 0.4 | audio | n/a |
| OutputDebugString layer | File sink is the record; debugger channel adds no user value yet | 1.0 | audio | add on request |
| Traditional-Chinese translation | Falls back to English (recorded); add `Lang::ZhHant` on request | 1.0 | audio | translate on request |
