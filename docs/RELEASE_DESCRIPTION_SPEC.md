# Release Description 规范

> 仅写用户可观测的改动（User-Observable Changes）。遵循 [Keep a Changelog 1.1.0](https://keepachangelog.com/zh-CN/1.1.0/) 与 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/) 成熟模板，面向最终使用者而非开发者。

## 1. 原则

1. **用户视角唯一**：只写安装后能感知到的行为差异。不写 `refactor`/`chore`/`ci`/`style`/`test`/`docs`/`build`。
2. **可观测判定**：若改动不导致托盘菜单文案、交互、音量/静音行为、开机自启、性能/兼容性、文件路径的差异，则不写入。
3. **语言面向用户**：用产品语言而非实现语言。写“修复自定义音量上限确定按钮无响应”，不写“修复 `HMENU(dangling_mut)` 误用”。
4. **每条可验证**：一条对应一个用户可复现的 `Given → When → Then`。
5. **空描述是合法的**：若本版无用户可观测改动，Release 正文留空，不凑写内部重构。

## 2. 模板（Keep a Changelog 精简）

```markdown
## [x.y.z] - YYYY-MM-DD

### Added
- 新增：...

### Changed
- 变更：...

### Fixed
- 修复：...

### Security
- 安全（如有）：...

### Deprecated / Removed / Breaking（按需）
- ...
```

- **标题**：`## [x.y.z] - YYYY-MM-DD`，`x.y.z` 与 `Cargo.toml`/`git tag` 完全一致。
- **分类仅用以上 6 类**：`Added`/`Changed`/`Fixed` 为常用；无内容则整节省略，不写空节。
- **每条以动词开头**，中文项目用中文、英文项目用英文，保持一版内语言一致（本仓用中文）。
- **不写**：`Internal`/`Refactor`/`Chore`/`Performance`（除非用户可感知如“冷启动 <400ms”）、`Docs`、`CI`。

## 3. 包含 / 排除对照

| 包含（写） | 排除（不写） |
|---|---|
| 新增菜单项、新增语言、新增设置项 | `refactor: 提取 fetch_snapshot_inner` |
| 修复：静音/音量/设备切换不生效、对话框无响应 | `chore: bump windows 0.62.2 → 0.63` |
| 变更：托盘提示截断规则、滚轮加速阈值 | `ci: clippy pedantic=warn`、`.gitignore` |
| 兼容：MSRV 1.80→1.85、Edition 2021→2024（影响安装） | `test: 新增 wheel_i32_min` |
| 性能：用户可感知的启动耗时、内存占用 | `perf: icon LazyLock 改 Mutex`（内部） |
| 安全：NUL 截断、权限 | 内部 `unsafe` 注释 |

## 4. 写法

- **格式**：`- 一句话（≤30字） + 括号补充影响范围`。可加 `(#123)` 关联 issue/PR。
- **好**：`- 修复：自定义音量上限对话框“确定”按钮点击无响应（音量上限→自定义）`
- **好**：`- 变更：无音频设备时托盘不再显示旧列表，显示空状态`
- **坏**：`- fix: dialog.rs dangling HMENU`（实现语言，用户无感）
- **坏**：`- 更新依赖`（无行为差异）

## 5. 生成流程

1. 收集 `git log <prevTag>..HEAD --oneline`，按 Conventional Commits 过滤：仅保留 `feat`/`fix`/`perf(用户可感)`/`BREAKING CHANGE`/`security`。
2. 将每条 `feat/fix` 翻译为用户语言，合并同类，去重。
3. 按 `Added/Changed/Fixed/Security` 归类，排序：`Fixed` 优先于 `Added`（用户先关心修障）。
4. 逐条自检：删除后用户是否会遗漏重要行为？否→删。
5. 若结果为空，Release 正文留空（符合本仓“所有 release 都不要描述”基线，需写时再按此规范写）。

## 6. 示例

### 好 — v0.1.1 用户视角

```markdown
## [0.1.1] - 2026-09-01

### Fixed
- 修复：自定义音量上限对话框“确定”按钮无响应
- 修复：无音频设备时仍显示旧设备列表

### Changed
- 变更：托盘提示过长时截断并过滤换行，避免显示异常
- 变更：快速滚动时音量步进更稳定，修复极端滚轮值异常
```

### 坏 — 开发者视角（不应写入 Release）

```markdown
### Fixed
- fix Hook race with AtomicUsize CAS
- fix vtable transmute safety
### Chore
- chore: clippy 83→55
```

## 7. 校验清单（发布前）

- [ ] 正文无 `refactor/chore/ci/test/docs/build` 条目
- [ ] 每条可在 Windows 10/11 托盘上手动复现
- [ ] 技术术语已译为用户语言（`HMENU`/`vtable`/`OnceLock` 不出现）
- [ ] 无实现细节、commit hash、内部编号
- [ ] 空 Release 允许直接留空

## 8. 参考

- Keep a Changelog 1.1.0 — https://keepachangelog.com/zh-CN/1.1.0/
- Conventional Commits 1.0.0 — https://www.conventionalcommits.org/zh-hans/
- Semantic Versioning 2.0.0 — https://semver.org/lang/zh-CN/
- GitHub Releases 最佳实践：Release 标题 = tag，资产 = `audio-switcher-rust.exe`，正文仅用户可观测

## 9. 在本仓的落地

- 本仓当前策略：**所有 release 默认留空**。需写时，按本规范手写上述 3~6 行，禁止自动把 `git log` 全量粘贴。
- 模板文件：`docs/RELEASE_DESCRIPTION_SPEC.md`（本文件）为准。
```

