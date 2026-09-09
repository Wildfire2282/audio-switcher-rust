# Windows 11 Tray Application Development Standard (Generic)

> Status: generic engineering standard for any resident Windows 11 tray utility, not bound to a specific project. Each tool builds, versions, and releases independently; new tools start at §2, existing projects migrate per §6.
> Scope: single-exe, windowless Rust tray tools resident in the notification area. Rust language practice follows ownership/borrowing, error handling, memory optimization, `unsafe`, API design, concurrency, numeric safety, serde, naming, testing, documentation, observability, and performance best practice, prioritized CRITICAL > HIGH > MEDIUM > LOW.
> Reference baseline (2026-09-08 snapshot; locked, upgrades by separate decision): Rust 1.98.1 stable, `rust-version = "1.85"` as MSRV floor; `windows 0.62.2` / `tray-icon 0.24.2` / `muda 0.19.3`.
> Version: v8 (v6: §2.6 comment standard; v7: bilingual required; v8: fixed menu tail refresh→autostart→language→about→exit, language submenu unified on the audio-switcher reference).

---

## 1. Toolchain Baseline (locked; upgrades by separate decision)

### 1.1 Toolchain and profiles

`panic = "abort"` is banned. Rationale: `abort` skips `Drop` (orphaned tray icon) and any missed `panic` (indexing/`unwrap`/div-by-zero) becomes a silent crash, directly violating §5.4 "every failure visible". Not worth the size saving for a resident tray app.

```toml
[workspace]
resolver = "3"          # single-lockfile resolver (see §6)

[workspace.package]
edition = "2024"
rust-version = "1.85"   # floor; raise only for new language features, then sync CI matrix
license = "MIT"

[profile.release]       # resident tray idle, size-first, but keep unwind for visible failure
opt-level = "z"
lto = "fat"
codegen-units = 1
strip = true            # strips symbols; .pdata unwind tables remain, panic hook still fires
panic = "unwind"        # abort banned; main installs panic hook → show_msgbox + exit(1) (see §4 Binary)
incremental = false
overflow-checks = true  # deliberate: negligible cost at tray idle, arithmetic bugs surface early
# debug-assertions stay default (dev=true/release=false), do not override

[profile.dev]
overflow-checks = true  # unwind kept, backtrace + should_panic work

[profile.dev.package."*"]
opt-level = 2           # iteration tradeoff for giant windows/tray-icon deps; dev builds too slow without it

[profile.bench]
inherits = "release"
debug = true
strip = false
panic = "unwind"        # override inheritance: bench harness + should_panic need unwind
```

### 1.2 Dependencies (workspace inheritance, opt-in per crate)

```toml
[workspace.dependencies]
# —— required (tray minimum set) ——
windows = "0.62.2"
tray-icon = "0.24.2"      # menu module = muda re-export
muda = "0.19.3"           # version MUST == tray-icon's actual muda dep (check procedure below)
thiserror = "2.0"
single-instance = "0.3.3"
auto-launch = "0.6.0"     # sole autostart solution (clean up legacy keys on scheme change, see §6)
tracing = "0.1"           # facade: libraries emit, never install; subscriber installed once in main
# —— conditional (only when the module is selected) ——
serde = { version = "1.0", features = ["derive"] }  # config.rs only
serde_json = "1.0"                                  # config.rs only
# parking_lot not referenced by default: no async, use std::sync::Mutex/RwLock; poisoning acts as a corrupt-state fuse, safer than parking_lot's silent reuse. Exceptions require rationale plus measurement, recorded in the tool SPEC.
# —— build/dev ——
winres = "0.1.12"         # build only
embed-manifest = "1.4"    # build only
tempfile = "3.10"         # dev only (tests needing filesystem isolation)
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }  # file sink in bin main only
```

- Do not reference `windows-core` directly. Use `windows` re-exports; add only when `Error/Code` enters a public signature and re-exports fall short, then pin it in sync with `windows` (note the version coupling in a comment).
- `muda` coupling check: on every `tray-icon` upgrade run `cargo tree -i muda`; the direct reference must equal tray-icon's built-in version, synced to tray-icon on mismatch. Lock duplicate versions with `cargo deny bans` (see §1.3).
- `anyhow` scope: banned in libs (domain errors use `thiserror`); allowed only at the `main.rs` boundary for `context` chains, finally mapped to `show_msgbox + exit(1)`; `Result<(), String>` banned workspace-wide (stringly-typed, no `source` chain, see §4).
- Banned: direct `winreg` writes for new features (legacy-key migration cleanup only), `embed-resource` (unify on `embed_manifest + winres`), async runtimes (no async tasks; separate decision if needed), privately installed global subscribers in libraries (except the single file-sink install in `main`).
- Manually review the API diff before upgrading niche crates (autostart/single-instance class), based on lockfile version + measured API behavior.
- `windows` features, minimum-subset principle: take only what §4's capability mapping requires, never copy a full set. CI spot-checks `cargo tree -e features -p <tool>` for unused heavy features.

### 1.3 Lints and gates

```toml
[workspace.lints.rust]
unexpected_cfgs = "warn"  # add check-cfg only for custom cfgs; cfg(windows) is builtin, do not declare
missing_docs = "warn"
unsafe_op_in_unsafe_fn = "warn"
[workspace.lints.clippy]
correctness = { level = "deny", priority = 1 }
suspicious = { level = "warn", priority = 1 }
style = { level = "warn", priority = 1 }
complexity = { level = "warn", priority = 1 }
perf = { level = "warn", priority = 1 }
pedantic = { level = "allow", priority = -1 }  # pick item by item, never the whole group
nursery = { level = "allow", priority = -1 }
cargo = { level = "warn", priority = -1 }      # false positives on publish=false bins take per-crate allow + comment
```

Gates: `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test` all green (the "three greens") + `cargo deny check` (advisories/bans/licenses/sources; judge 0.x duplicates by minor version) + `grep -Rn "println!\|dbg!" -- src tests` zero hits (diagnostics go through `tracing`, §5.4) + miri for `unsafe`-containing pure-logic modules (scoped below).

- `cargo tree -d` is manual review only, not a hard gate (transitive duplicates locked via `cargo deny bans`).
- Scope miri to pure logic: `cargo miri test -p <tool> --lib -- <pure-logic module paths>`; all Win32 calls isolated behind `#[cfg(windows)]`, miri never covers Win32.
- Registry/system-state test policy: default `cargo test` writes no real registry keys and mutates no system state. Read logic under test goes through trait + mock; integration tests that must touch real keys live in `tests/` + `#[ignore]` + explicit opt-in env var (e.g. `RUN_REAL_REGISTRY_TESTS=1`), scoped to `HKCU\Software\<Tool>\__tests__` + RAII cleanup.

---

## 2. Engineering Scaffold

```text
<tool>/
  Cargo.toml  build.rs  icons/  <tool>.manifest  README.md  SPEC.md  # fixed
  src/
    lib.rs main.rs app.rs|app/  domain.rs                          # required minimum
    config.rs hotkey.rs                                            # conditional·needs persistence/needs global hotkeys
    platform/  instance.rs dialog.rs shell.rs locale.rs            # required (bilingual v7: every tool follows the system language)
               autostart.rs theme.rs dpi.rs                        # conditional (autostart selected by default, OPT-OUT in tool SPEC; theme/dpi only when icons follow the system)
               com.rs hook.rs                                      # conditional·needs COM/needs hooks
    ui/        tray.rs menu.rs i18n.rs                             # required (bilingual v7: every user-visible key in 中文 + English)
               icon.rs tooltip.rs text.rs                          # conditional·needs custom bitmaps/needs rich tooltip/needs copy reuse

`README.md` documents user-visible behavior only; `SPEC.md` documents this tool's behavior and acceptance (input/output/acceptance + exemption/OPT-OUT/1.0-contract registry). All engineering standards come from this document and win on conflict.

**Selection overview**: required set implemented unconditionally; a selected conditional module takes on its full bundle, never half.

| Conditional module | Trigger | Full bundle |
|---|---|---|
| `config.rs` + serde | needs persistence | `version` + `migrate` + path chain (config rule below) + follow fields only when following (§5.2) |
| `platform/autostart.rs` | selected by default; no-autostart needs OPT-OUT rationale in tool SPEC | three typed functions + legacy-key cleanup (§4/§6), no silent `bool` default |
| `platform/theme.rs` + `ui/icon.rs` | icons/menus follow light/dark | `Theme/ThemeMode/resolve/apply_app_mode` + three-arg `make_icon` + listener debounce (§4/§5.2) |
| `platform/dpi.rs` | bitmaps blur under mixed DPI; skip for vector/system-rendered | `tray_icon_size` + `pick_size` + change coalescing (§4) |
| `ui/i18n.rs` + `platform/locale.rs` | Required (v7, bilingual) | two-tier: persistent tools take `Lang::System` default + three-way menu group + legacy-default migrate; stateless tools take system-follow + `intl` live rebuild (§5.2); no single-language exemption |
| `hotkey.rs` | needs global hotkeys | `register/unregister + describe` + unit tests + events merged into `app::Action` (§4) |
| `platform/com.rs` | needs COM | `ComGuard` + held by `App` + declared STA model + `main` fails with `exit(1)` (§4) |
| `platform/hook.rs` | needs thread hooks | RAII guard + Acquire/Release pairing + `try_lock` inside callbacks; never install a hook just to center a dialog (§4 Dialogs) |
| domain trait + mock | hardware backend needs unit tests | trait abstraction + mock + injected App; plain code when no hardware/test need |

- lib + minimal bin: declare `[lib]` for multi-module crates or integration tests; tiny single-file tools may stay pure bin, no forced split. `main.rs` ≤30 lines — guards → `App::run`, exit on failure (second instance `exit(0)`, init failure `exit(1)`, see §4), no business logic, no UI copy.
- `prelude.rs` only when ≥3 modules repeat the same import set. Crate docs inline as `//!` (published crates may unify README via `include_str!`; internal crates not required).
- Visibility defaults to `pub(crate)`; `pub` needs review justification. Libraries never install a log subscriber (single file-sink install in `main` excepted, see §5.4).
- `build.rs` minimal, deterministic, idempotent: `#![allow(missing_docs)]` + complete `rerun-if-changed` (manifest + `icons/**` + `build.rs`) + non-Windows no-op guard (return directly outside `#[cfg(windows)]`, keeps Linux `cargo check` green) + `embed_manifest` + `winres`.
- Single-source versions: VERSIONINFO and manifest version always generated from `CARGO_PKG_VERSION` (§7), never hand-synced.
- `<tool>.manifest`: four `supportedOS` GUIDs; only `dpiAwareness PerMonitorV2, PerMonitor`. The manifest is a static declaration; runtime still needs Get-DPI queries. The ban covers `SetProcessDpiAwareness*`-class Set APIs, not Get queries.
- `icons/` glyph pipeline, see §5.3; the renderer is stable logic, shared from the start: one parameterized renderer tool at the workspace root `tools/`, each tool keeps only SVG sources + RGBA outputs. Never copy the renderer per tool.
- Cross-platform compile gate: Windows-only CI by default; `#[cfg(not(windows))]` thin stubs (`unimplemented!` + comment pointing at real-machine acceptance) only when Linux `cargo check` is needed, never stubs written for all modules up front.
- config.rs (conditional·needs persistence): all fields `#[serde(default)]` + `version` + `migrate`; unknown fields rejected (`deny_unknown_fields` + renames handled in `migrate`; local config typos must be loud).
- config.rs path: `%APPDATA%\<Tool>\config.json`, `LazyLock` cache + fallback chain `%APPDATA% → %LOCALAPPDATA%\<Tool>\config.json → temp read-only degradation (warn log)`. `./config.json` banned (`Program Files` is not writable).

### 2.1 File and directory naming

- Root: `Cargo.toml` (workspace, `members` listing each tool, `resolver = "3"`, single `Cargo.lock`) + `docs/` (this standard, committed with the repo, never ignored) + `<tool>/`; never commit `target/`, `*.exe`, `*.pdb`, `.env*` (remove tracked binaries with `git rm --cached`).
- Per-tool layout: `Cargo.toml build.rs icons/ <tool>.manifest src/ tests/ README.md SPEC.md`; root `tools/` holds the shared renderer.
- All lowercase snake_case names; multi-file modules as `foo/mod.rs`; integration tests in `tests/`, unit tests live in nearby `#[cfg(test)]` modules.
- Recommended `Cargo.toml` field order: package (name/version/edition.workspace/rust-version.workspace/description/license/repository/keywords/categories) → lib/bin → dependencies (workspace-inherited) → dev/build → `lints.workspace = true`; style guidance only, not CI-enforced. `publish = false` semantics: add for internal libs; pure bins need nothing (bins are never published).

### 2.2 Module contracts

- One-way dependencies: `main → app → {ui, domain, config} → platform`; `platform/*` mutually unreferenced by default, foundational deps like `dialog/shell` allowed with a contract comment on the callee side + review sign-off; `ui` never calls Win32 directly (via platform); `domain` never depends on ui/app (pure business logic, unit-testable). Reverse dependencies bounced at review.
- Business actions belong to `app` (e.g. `app::Action`); `ui::menu` owns only `menu::id` constants + `from_id` mapping + handle sync, no business semantics. `hotkey` events map to the same `app::Action` (§4).
- Module docs follow §2.6; smallest cross-module interfaces, default `pub(crate)`, `pub` needs review justification.
- Blob budget: soft 500-line budget per file, split when exceeded.

### 2.3 Five steps to add a feature (follow in order)

1. Add a behavior entry to the tool SPEC (input/output/acceptance).
2. `menu::id` constant + `app::Action` variant (unknown ids follow the §4 message-pump path: `debug_assert + log + ignore in release`, locked by test; never silently swallow).
3. Handler dispatch + domain implementation (pure functions preferred).
4. Cover the new state in `sync_state`/rebuild (with debounce, see §5.1).
5. Unit tests + real-machine acceptance.

### 2.4 Three steps to fix a bug

1. Write the failing reproduction first (pure logic into `#[test]`; hardware steps into the tool SPEC as real-machine steps; flaky timing/hook races may stop the bleeding first, then add a stable repro and note the gap).
2. Minimal blast radius: touch only in-contract modules; cross-boundary changes update the callee's type-level contract (signatures/enums/errors) first, comments are footnotes.
3. Three greens + permanent regression test.

### 2.5 New-tool scaffold checklist

Template (`cargo generate` or root `xtask scaffold --name <tool>`, adopt one, drop the other) → tick conditional modules (no COM, no com.rs; no persistence, no config.rs; no hotkeys, no hotkey.rs; `locale.rs` + `i18n.rs` always scaffolded, bilingual required since v7) → generate icons/manifest/Cargo/README/SPEC (single-source versioning, see §7) → tool SPEC defines behavior → join workspace members. Each step independently delegable to an agent, accepted against the tree plus the tool SPEC item by item. Never "copy an existing tool directory and delete files" as policy (copies drag bugs, stale manifest GUIDs, and stale icons).

### 2.6 Comment standard

- Write *why*, not *what*: comments explain intent/constraints/background (Win32 routing rationale, magic-number provenance, tradeoffs and costs); restating code is banned (no `// increment counter` on `i += 1`).
- English only: all comments (`//!`/`///`/`//`, TODOs, SAFETY notes) in English; identifiers quoted verbatim. Existing non-English comments are not bulk-rewritten; Anglicize a file when touching it.
- Layers: `//!`/`///` address users (what/how), `//` addresses maintainers (why/watch out). No file-header comments (versioning belongs to git).
- Module `//!` opens with a one-sentence responsibility; append input/output/non-goals only when the boundary is easy to misread, then stop, no template padding.
- Public-item `///` opens with one sentence; fallible items add `# Errors`, panicking items `# Panics`, `unsafe fn` adds `# Safety` (enforced via §1.3 `missing_docs = "warn"`); worthwhile examples as runnable doctests.
- `unsafe`: every `unsafe` block carries `// SAFETY:` (why sound here), every `unsafe fn` carries `# Safety` (caller obligations). Uncommented `unsafe` bounced at review.
- Debt markers in one format, `// TODO(<owner>, <date>): <item>`, same shape for `FIXME`/`HACK`; ownerless or dateless TODOs never merge; debt tracked in the tool SPEC registry.
- Commented-out code banned: git recovers deletions, leftovers rot. No vacuous comments, no comments contradicting code; update comments with the code change, review comments as code.
- Line width follows `cargo fmt` (default 100 columns), wrap long comments by hand; no secrets/PII in comments, same redaction standard as logs (§5.4).

---

## 3. Rust Code Standard

- Follow Rust best practice throughout (ownership/borrowing, error handling, memory optimization, `unsafe`, API design, concurrency, numeric safety, serde, naming, testing, documentation, observability, performance), CRITICAL > HIGH > MEDIUM > LOW.
- Exception list (five items only, best practice governs everything else):
  1. `anyhow`: banned in libs (domain errors use `thiserror`); the `main.rs` boundary may chain `anyhow::Context` before mapping to `show_msgbox + exit(1)`.
  2. `Result<(), String>` banned workspace-wide: typed `thiserror` domain errors + `#[source]` chains, `Display` for users, `Debug` for logs.
  3. serde rejects unknown fields: `#[serde(deny_unknown_fields)]` + renames migrated in `migrate`; tolerating unknowns swallows config typos.
  4. Startup-path panic handling: `panic = "unwind"` + `main` panic hook (`show_msgbox + exit(1)`); "banning panic" is unenforceable, and `abort` crashes silently.
  5. Ordinal-level hacks (e.g. uxtheme ordinal imports): paired review + real-machine multi-build matrix (at least three Win11 build tiers, `x64` mandatory) + feature gate + runtime-probe failure fallback (§4 themes).

---

## 4. Windows 11 Platform API Registry

| Capability | Stance | Standard API |
|---|---|---|
| Single instance | Required | `SingleInstanceGuard::acquire(canonical_id) -> Result<Option<Self>, InstanceError>`: `Ok(None)` = already running → `exit(0)`; `Err` = creation failure (ACL/namespace) → dialog + `exit(1)`. Single canonical id: `"{kebab}-single-instance-v1"`; autostart/mutex/config-dir names all derive from one `TOOL_ID` constant (`kebab` for mutex/dir, `PascalCase` for autostart display name only, mapping locked by unit test) |
| COM | Conditional·needs COM | `ComGuard::init() -> Result<Self, ComError>` (`#[must_use]`, side-effect split: init shows no dialog; the caller (`main`) dialogs and exits (`exit(1)`)). Held by `App` for lifetime; STA model declared explicitly (`COINIT_APARTMENTTHREADED`, `!Send`/`!Sync` markers); MTA needs a separate decision. `main` fails with `exit(1)` |
| Autostart | Default (OPT-OUT in tool SPEC) | `get_exe_path() -> Option<PathBuf>` / `set_autostart(bool) -> Result<(), AutostartError>` (`thiserror`, `#[source]` keeps the underlying error, no `String`) / `autostart_state() -> AutostartState::{Enabled, Disabled, Unknown(String)}` (read failure is `Unknown`, UI grayed + tooltip note, never "read failure means off"). `auto-launch` mapping: `AutoLaunch::new(PascalName, exe, CurrentUser, &[])` + `enable/disable/is_enabled`; "configured on but actually off at startup" self-heals in the background only when the state reads explicit `Disabled`; never writes on `Unknown`, only reports. Failures pop `show_autostart_error(source)` (with actionable guidance) |
| Dialogs | Required | `show_msgbox(&str)` + `show_autostart_error(&AutostartError)`; centering via owner-centering/`SetWindowPos` (never a resident thread hook just for centering). `MB_TOPMOST` for critical errors only, `SETFOREGROUND` focus-stealing banned; titled with the tool name; silent on success. No `show_info` by default; onboarding/conflict guidance needs a tool-SPEC entry |
| External links | Required | `platform::shell::open_url(&Url)` (`Url` newtype, `https` + publishing-domain allowlist only, `FromStr` validated; no raw `&str`). Failures surface a dialog (no `let _ =` swallowing); implemented via `ShellExecuteW` (verb=open, explicit working dir), illegal schemes locked out by test |
| Message pump | Required | `MsgWaitForMultipleObjectsEx(timeout) + PeekMessageW` (with periodic work, `timeout` default 200ms, tunable in the tool SPEC with its idle-CPU budget noted); pure-event tools may use `GetMessageW`. Menu-event channel polled non-blockingly and dispatched via `from_id`; unknown ids: `debug_assert! + tracing::warn + ignore in release` (locked by test), never silently swallowed. `WM_SETTINGCHANGE`/`WM_DPICHANGED` bursts coalesced at 100ms debounce (see §5.2/§5.3), exactly one `update + rebuild` per burst |
| Hotkeys | Conditional·needs global hotkeys | `register_all() -> Result<(), HotkeyError>` (collect every occupied combo and report them together, with `describe_hotkey` guidance) + `action_for_hotkey`/`describe_hotkey` unit tests + `unregister_all` on exit; events map to `app::Action`; key changes need tool-SPEC entry + review (known conflicts with outside software recorded in tool SPEC) |
| Themes | Conditional·icons follow the system | `platform::theme::{Theme, system_theme, resolve, apply_app_mode}`; detection reads `HKCU\...\Personalize\AppsUseLightTheme` (0=dark, read failure is `Unknown` → keep last theme + log, never "failure means light"). Dark menus are an ordinal hack: `SetPreferredAppMode(1)`[135] once at startup before menu creation (order `apply_app_mode → build menus`, wrong order leaves a light residue; reviewers must verify the order), then `RefreshImmersiveColorPolicyState`[104] + `FlushMenuThemes`[136] on switch; runtime `GetProcAddress` ordinal probing + `stdcall` signature with `# Safety` comment, missing/stale ordinals fall back to light + `tracing::warn` (logging is the visibility floor for cosmetic degradation; aligns with the §5.4 exception). Same review bar as ordinal hacks; no theme module means no following |
| Language | Required (v7, bilingual) | `platform::locale::system_lang()` (`GetUserDefaultLocaleName` user locale; Simplified-Chinese locales → Simplified Chinese, Traditional-Chinese locales → Traditional (fall back to English where untranslated, recorded in the tool SPEC), everything else → English; read failure is `Unknown` → keep last language + log, never "failure means English"; stateless startup fallback English + log). Persistent tools (has `config.rs`): `Lang::System` default (symmetric with `ThemeMode`) + three-way menu group, checks reflect the mode; legacy non-`System` defaults migrate to `System`. Stateless tools (no `config.rs`): follow the system + `intl` live rebuild, recorded in the tool SPEC |
| Resolution | Conditional·bitmaps blur | `platform::dpi::tray_icon_size(hwnd_or_monitor) -> u32`: explicit DPI source (per taskbar-monitor `GetDpiForMonitor` for tray icons, `GetDpiForSystem` without a window handle with the approximation noted in a comment; never pass 0). `GetSystemMetricsForDpi(SM_CXSMICON, dpi)`, whole-size fallback on failure, never panics. `pick_size` documents its scaling assumption in a comment (`SM_CXSMICON` already scaled, only round up to 16/20/24/32, never multiply by dpi/96 twice) |
| Binary | Required | `#![windows_subsystem = "windows"]`; single exe; `panic hook → show_msgbox + exit(1)`; `tray` drops with `App`, `Drop` order commented (menus before icon), no notification-area residue |

---

## 5. Tray UI/UX Standard

### 5.1 Menu shape and layout

Fixed top-down: ①`title` grayed, non-clickable `{DisplayName} v{ver}` → ②separator → ③feature group (tool-specific: device lists, exclusive groups, toggles, tool submenus, system-tool shortcuts; per tool SPEC; long device lists capped + "More…" overflow, N in tool SPEC) → ④separator → ⑤fixed tail group, identical in every tool: `refresh` → `autostart` → language submenu → `about` → `exit` always last (no separators inside the tail, none between about and exit).

- ids centralized in `menu::id` + group prefixes; exactly one checked item per exclusive group; rebuild only on add/remove/reorder/rename/language change, otherwise in-place `sync_state()` (`#[must_use] -> bool`, `true` = changed; language/theme/DPI bursts coalesced, one rebuild).
- Fixed tail (order + ids frozen; no tool-specific items inside, tail items never move into the feature group): `id::REFRESH` (`refresh`) → `id::AUTOSTART` (`autostart`) → language submenu (id `language`) → `id::ABOUT` (`about`) → `id::EXIT` (`exit`). `autostart` renders `autostart_state()` (`Unknown` grayed + tooltip note, never default-off). Extending the tail is a scheme change (version bump + tool-SPEC note), never a per-tool decision.
- Refresh item (required for device-listing tools): `id::REFRESH` + `app::Action::Refresh`, first in the tail; clicks run a full `refresh_ui` (uncached backends re-enumerate by default; caching backends implement a mandatory `clear_cache`, no `noop` default). Automatic listeners stay (device-change notification, mechanism chosen in tool SPEC); manual refresh is the sleep-resume/callback-loss fallback; tests cover `from_id` parsing + re-enumeration after cache clear.
- Refresh label (uniform, every tool): `tr("refresh")` renders `刷新设备列表` / `Refresh Devices`, locked by test; the id stays `id::REFRESH`.
- Language submenu (reference implementation: audio-switcher `ui/menu.rs`): `Submenu` id `language`, label `tr("language")`; three `CheckMenuItem` children with frozen ids `lang_system` (`tr("system")`) / `lang_zh` (`tr("chinese")` = 中文) / `lang_en` (`tr("english")` = English); exactly one checked — checks follow the language mode, never the effective language; labels render in the effective language. Persistent tools bind checks to `cfg.lang`; stateless tools bind to the session mode (default follow-system, explicit choice lasts until exit, recorded in the tool SPEC).
- `TrayWrapper { tray, handles }` + `new / rebuild_menu / sync_menu / update_tooltip / update_icon` (a single naming set); tooltip is a one-line status summary (truncate to a 64-char budget, abbreviate over, locked by test); `with_menu_on_left_click` decided in tool SPEC (an inert left click must be documented there, no "clicked nothing happens" dead zones).
- About: `app::Action::About` → `open_url(ABOUT_URL)`; `ABOUT_URL` defined once per crate (points at the tool's release homepage; prefix locked by test, scheme validated via `Url`).
- Version: title takes `env!("CARGO_PKG_VERSION")`, Cargo.toml the single source (generation in §7); tests assert the title carries the version; `repository` shares the ABOUT_URL origin.

### 5.2 Language: bilingual required (v7) — follow by default, explicit lock when persistent

- Themes: `Theme::Light/Dark` + `ThemeMode::System/Light/Dark` (fields only when following, `#[serde(default)]`; no module when not following). Detection per §4 (`Unknown` on failure).
- Language, every tool: `platform/locale.rs` + `ui/i18n.rs` ship unconditionally; mapping per §4. `ui/i18n.rs` covers every user-visible menu/tooltip/dialog key in both languages (unknown keys pass through verbatim, locked by test); `resolve` / `lang_for_locale` pure functions branch-tested. Single-language builds are banned and take no §8 exemption.
- Language, persistent tools (has `config.rs`): `Lang::System` default (symmetric with `ThemeMode`); `lang` field with `#[serde(default)]`; three-way submenu in the fixed tail (§5.1), checks reflect the mode; legacy non-`System` defaults migrate to `System`. In System mode live `intl` rebuild is recommended; startup-resolve with restart-apply is allowed only as dated SPEC debt with a review-by (audio-switcher SPEC §8).
- Language, stateless tools (no `config.rs`): follow the system at startup + `intl` live rebuild, and carry the same tail language submenu bound to a session mode (default follow-system; explicit choice lasts until exit), recorded in the tool SPEC.
- Same-pump handling with debounce: `WM_SETTINGCHANGE` filters `lParam` (wide-char compare on `ImmersiveColorSet`/`intl`, case-sensitive, locked by test) + 100ms debounce; `ImmersiveColorSet` (themes, re-read, on change `apply_app_mode + update_icon + rebuild_menu`) / `intl` (language, re-resolve in following modes, rebuild on change).

### 5.3 Sharpness: resolution and icons

- Bitmaps at needed sizes: `make_icon(state, theme, size)` (explicit three-arg signature), size from `tray_icon_size()`, never hardcoded; `pick_size(dpi)` matrix tested (96→16/120→20/144→24/192→32, round non-whole sizes up, no double scaling); cache key `(state, theme, size)`, `update_icon` owns invalidation (clear on theme/DPI change, locked by test).
- Change coalescing: `WM_DPICHANGED` / `WM_DISPLAYCHANGE` debounced into one `update_icon + rebuild_menu`; system-rendered text/menus/dialogs untouched.
- Glyphs sourced solely from Lucide (24×24 / fill none / stroke 2 / round); upstream originals frozen in `icons/lucide/upstream/`, working copies in `icons/lucide/<name>.svg` with edits noted in a file-head XML comment; allowed edits: optical emboldening across sizes (recorded with a real-machine screenshot in tool SPEC), spacing, path composition; viewBox/centering/corner-radius/monochrome semantics frozen, no fill/color/new visual language (ISC permits modification, attribution file kept in-repo).
- Offline rendering (single parameterized tool at workspace root `tools/`): one fixed SVG parser (declared feature subset, superset elements are compile errors); size table + black/white → RGBA (unpremultiplied, `width/height/stride=width*4`, header carries dimensions, loader verifies consistency) + ICO; supersampling ≥4; stroke-width follows each SVG's own declaration; transparent background, alpha from coverage × `opacity` only; no viewBox re-cropping.
- Outputs: `icons/<name>_<theme>_<size>.rgba` (16/20/24/32, light `#000000` / dark `#FFFFFF`), runtime `include_bytes!` + build-time check (`build.rs` asserts every `(state, theme, size)` combo exists, missing entries fail compilation, no runtime panic fallback); `icons/app.ico` slim set 16/24/32/48/256 (set changes need tool-SPEC evidence); icon-to-state mapping in tool SPEC. Menus text-only by default; icons need a tool-SPEC entry.

### 5.4 Iron rules

Silence on success; every failure visible (sole exception: cosmetic degradation like stale uxtheme ordinals falling back to light may skip the dialog but must `tracing::warn` to disk); new trigger mechanisms must reuse the `app::Action` entry; menus/hotkeys bind stable hardware ids (tool SPEC defines the stable id + reboot/plug testing); diagnostics: user-actionable → dialog; non-actionable → `tracing` (`OutputDebugString` + file sink under `%LOCALAPPDATA%\<Tool>\logs\`, rotated and redacted, no secrets/PII) + zero `println!`/`eprintln!`/`dbg!` workspace-wide (CI grep enforced).

---

## 6. Existing-Project Migration Guide (new tools skip, start at §2.5)

**Real-machine matrix** (environment coverage; only behavior already covered by this standard): Win11 physical machines (record actual build numbers, never "latest"); VMs run the three greens only, no display acceptance. Minimum ship-blocking subset: single screen 100% + mixed-DPI dual screen (high-scale primary + 100% secondary) + dark/light × bilingual four-combination smoke test (icons, menu, title) + one sleep-resume. Full matrix (100/125/150/200% sweep) for major versions only. Evidence: checklist + key screenshots attached to the Release.

- [ ] P0 workspace: root `Cargo.toml` (`[workspace] members + resolver="3" + full §1`, single `Cargo.lock`); each tool inherits; add `rust-version`, fill package metadata, `lints.workspace = true`; align profiles. Default size budget: single exe ≤800KB (tool SPEC may tighten, never loosen silently). Acceptance: `cargo build --workspace --release` green + `cargo deny check` green.
- [ ] P1 lib split and typed errors: `[lib]` per §2 decision tree (tiny tools may stay pure bin); `main.rs` ≤30 lines; `anyhow` converged to the `main` boundary, `Result<(), String>` converted to typed errors; deps per §1.2 tiers. Autostart-scheme changes include migration code that cleans legacy keys (key names locked by test). Acceptance: three greens.
- [ ] P2 platform and asset convergence: typed three-function autostart (with `Unknown`); single-instance distinguishing "already running" from "creation failed"; dialog (`show_msgbox` + owner centering); build.rs (single-source version generation + non-Windows no-op + complete rerun) + `icons/`/manifest layout; manifest keeps only `dpiAwareness`; icon pipeline (root shared renderer + vendor + full-set regeneration + LICENSE + attribution). Acceptance: autostart survives reboot; second launch exits silently; ICO sharp at all sizes.
- [ ] P3 UI convergence: `menu::id + from_id` (fixed tail per §5.1: refresh→autostart→language→about→exit); unified `TrayWrapper` naming; one tooltip formatter name (64-char budget); themes/locales/DPI + listener debounce; explicit three-arg `make_icon`; ABOUT_URL/repository same origin. Acceptance: menu-hotkey parity; dark/light following; fresh installs follow the system by default; sharp at all DPIs; correct title version; working About; exit last.
- [ ] P4 gates and audit fixes: real-registry tests to `#[ignore]` + RAII cleanup; zero `println!`/`eprintln!`/`dbg!` in tests (CI grep); stray `unsafe` gets `// SAFETY`, `unsafe fn` gets `# Safety`; startup-path panics to hook + typed errors; tracked binaries removed via `git rm --cached`; gates into CI (Windows runner for the full §1.3 set + MSRV `cargo check/test`).
- [ ] P5 follow-ups (non-blocking): split >500-line files; re-review copy exemptions; manual refresh items for device-class tools (§5.1).

---

## 7. Versioning and Release

- Independent SemVer per tool; 0.x: patch = bug fixes, minor = features (compatible field additions handled in `migrate`, breaking config changes bump minor + tool-SPEC note). major reserved past 1.0; breaking commits marked (`BREAKING:` prefix + tool-SPEC link).
- 1.0 graduation bar: tool-SPEC success criteria all green + frozen config format + menu-id stability contract (recorded as the tool SPEC "1.0 contract": never rename, never reuse deleted ids — not "frozen set"; adding menu items is minor). Afterwards major = incompatible config / removed-or-renamed menu ids / default-hotkey changes / autostart-location changes.
- Single source `Cargo.toml [package] version`, everything else generated: `build.rs` generates manifest `assemblyIdentity` (first three segments + `.0`, fourth always 0; `-rc.N` stripped) + winres VERSIONINFO from `CARGO_PKG_VERSION`; humans edit Cargo.toml only. `-rc.N` prereleases allowed in Cargo (real-machine validation); tag format `<tool>-vX.Y.Z-rc.N` (rc) / `<tool>-vX.Y.Z` (release); single `Cargo.lock` committed (workspace root).
- Annotated tag `<tool>-vX.Y.Z` on the main branch (bare `v` banned, signing recommended); Release pins the same tag + single exe + delta notes + real-machine matrix checklist; one tool's release never moves another's (prefix names on a shared Releases page).
- Release checklist: tool-SPEC acceptance green → bump version (one place in Cargo) → verify generated artifacts (manifest/VERSIONINFO automatic, spot-check exe properties) → three greens + `cargo deny` → size budget (default 800KB) → commit + tag + push (including tags) → upload exe to Release. Unsigned state disclosed in the Release with SmartScreen expectations (§8 not touching signing is status, not user impact; the copy must say so).
- Release-notes template (user perspective): title = tag; single-asset (one exe); body documents only post-install behavior deltas, never `refactor/chore/ci/style/test/docs/build` or dependency bumps, no download section. Chinese section first, English second (`Added/Changed/Fixed` mapping the Chinese `新增/变更/修复`, fixed order, empty sections dropped symmetrically). One-liners (Chinese ≤30 chars), 1:1 correspondence. No technical terms/function names/commit hashes. Version matches `Cargo.toml`/tag with release date. Self-check: no technical entries, every entry reproducible in the tray, section correspondence holds, no download section.

---

## 8. Non-Goals and Exemptions

No installers/signing/admin rights/registries outside HKCU; no "roughly right" shortcuts (e.g. coarse external display-switch commands for precise switching); no main window/resident notifications; no committed secrets; no telemetry/no outbound (user-initiated About navigation + local crash log files excepted; crash reports require the user to explicitly send the log file, never auto-uploaded).
Sharing trigger: identical logic (renderer/dialog/theme probing/autostart wrapper) extracted into a shared crate on its third occurrence, copying allowed for the first two.
Capability selection follows the §2 decision table (tool SPEC decides, no surplus modules).
Exemption registry (tool SPEC registry table, unregistered means unexempted): `| Item | Rationale | Review-by | Owner | On user request |`; bilingual (`locale.rs` + `i18n.rs`) takes no exemption — partial deferrals (live `intl` listener, Traditional-Chinese translation) are dated debt with a review-by, never permanent exemptions.
