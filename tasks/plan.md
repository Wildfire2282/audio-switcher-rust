# 任务计划（增量实施 · 最小垂直切片）

> 来源：SPEC.md。每次仅做一个任务，做完即停。

## Task 1（本次）：`clamp_volume` 全路径 100 上限 — 已完成

- 范围：仅 `src/config.rs` 内 `clamp_volume` 与对应单测。
- 验收：
  - [ ] RED：新增测试 `clamp_enabled_out_of_range_limit_still_caps_at_100`，构造 `volume_limit: 200` + `volume: 150`，期望输出 `100`，首次运行失败。
  - [ ] GREEN：最小修复使该测试通过，且不改变 `1..=100` 正常路径行为。
  - [ ] `cargo test` 全量通过。
  - [ ] `cargo build` 成功。
- 不做：不碰持久化、音频后端、UI。

## Task 2：`clamp_disabled` 超 100 回归锁定 — 已完成

- 为已实现的 `min(volume, 100)` 关闭路径补边界测试（`120 -> 100`），纯测试增量。

## 输入设备切换（新增）：输出区下方镜像输入区 — 已完成

- 范围：`AudioBackend` 采集三方法、Real/Mock/桩实现、菜单输入区（禁用标题 + `input_` 选项）、`InputDevice` 分发、托盘与 App 接线、中英 i18n。
- 验收：新增 `set_default_input_device_mock`、`parse_input_device`、`sync_state_tracks_input_devices`；`cargo test` 全量通过；`cargo build`、`cargo clippy`、`cargo fmt --check` 通过。
- 不做：输入端音量/静音控制仍走输出默认端，不动音量上限、滚轮、通知链路。

## 评审整改（新增）：五维评审发现全修复 — 已完成

- 补 `integration_capture_enumerate_switch_restores`（`#[ignore]` 真机测试，RAII 恢复默认采集端）。
- `menu.rs` 提取 `check_entries`/`sync_entries`，消解输出与输入两处重复。
- `Mock` 采集无缓存改Shutdown为注释说明设计意图；`def_in_id` 改名 `def_input_id`；`eMultimedia (0)` 勘误为 `(1)`。
- `cargo test` / `cargo clippy` / `cargo fmt --check` 全通过。

## Task 3：滚轮 `total_step` 符号与大增量边界复核 — 已完成

- 仅补单测，不改算法；如发现行为偏差另立任务。
