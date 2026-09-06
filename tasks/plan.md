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

## Task 2（待定）：`clamp_disabled` 超 100 回归锁定 — 待办

- 为已实现的 `min(volume, 100)` 关闭路径补边界测试（`120 -> 100`），纯测试增量。

## Task 3（待定）：滚轮 `total_step` 符号与大增量边界复核 — 待办

- 仅补单测，不改算法；如发现行为偏差另立任务。
