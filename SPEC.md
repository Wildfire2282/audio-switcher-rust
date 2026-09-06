# SPEC — Audio Switcher Rust（最小基线）

> 范围：本次单步增量仅覆盖音量上限钳制不变量。其余行为沿用 README 与现有实现，不在本 SPEC 中重定义。

## 1. 不变量

- `clamp_volume(volume, cfg)` 输出恒满足 `0 <= output <= 100`。
- 启用上限时：`output = min(volume, volume_limit, 100)`。
- 关闭上限时：`output = min(volume, 100)`。
- `volume_limit` 持久化有效范围为 `1..=100`，越界在 `migrate` / `recover_tolerant` 中回落至默认值 25。

## 2. 非目标

- 不修改托盘、音频后端、滚轮加速、菜单、i18n、开机自启行为。
- 不新增配置字段，不修改持久化格式。

## 3. 验收

- 新增回归测试覆盖启用上限但 `volume_limit` 越界时的 100 上限。
- `cargo test` 全量通过，`cargo build` 成功。
