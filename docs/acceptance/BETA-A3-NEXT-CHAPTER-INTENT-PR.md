# Beta-A3 后续：作者章节目标契约（独立 Draft PR 范围）

## 背景

Beta-A3 已完成审校可信度、正式桌面 Provider 稳定性与 Run 输入冻结三项 P1。作者章节目标
目前仍主要依赖小说简介与结构化大纲，没有独立的持久化契约、Run 冻结版本和最小编辑入口。

本文件记录后续 Draft PR 的最小范围与验收标准，避免在大迁移中回退已完成的主线修复。

## 目标

让作者可以为某一章写下明确的创作目标，并保证：

1. 目标可以保存、读取和编辑；
2. Planner 能读取目标并建立 `keyEvents`；
3. 新 Run 在开始时冻结该目标；
4. 旧 Run 不被后来修改的目标污染；
5. 有冲突时明确提示作者；
6. 模型不能覆盖作者明确确认的硬性目标；
7. 历史章节没有目标时继续兼容；
8. GUI 提供最小可用的编辑入口。

## 建议数据契约

```text
ChapterIntent
  id
  novelId
  chapterId
  requiredEvents[]        作者明确要求发生的事件
  forbiddenEvents[]       不允许发生的事件
  requiredCharacters[]    必须出现的角色
  orderingConstraints[]   必须保留的因果与时间顺序
  notes                   作者备注
  version                 乐观并发版本
  updatedAt
  confirmed               作者是否已确认
```

持久化建议复用 `workflow_artifact` 语义而不是新增一张大表：

- 类型：`chapter_intent`
- scope：`chapter:<chapterId>`
- 状态：`draft` → `confirmed`
- `protected_user_content = true`（作者内容）
- 用现有版本号与 `content_sha256` 做并发保护

这样可以避免新迁移，同时保留历史与审计。若确实需要独立表，必须先提供可回滚迁移与备份。

## Planner 与 Run 冻结

- `open_run` 把当前 `confirmed` 的 `chapter_intent` 版本写入
  `RunContract.source_versions["chapter_intent"]`，并复用 Beta-A3 的 `input_snapshot` 模式，
  只保存摘要而非复制大对象。
- `story_macro.keyEvents` 与 `chapter_list.obligations` 必须覆盖作者的硬性目标；
  现有 `validate_chapter_plan_references` 的 `sourceEventId` 校验可以直接复用。
- 冲突（例如作者要求的事件与已冻结的后续章节事实矛盾）必须显式暂停并提示，
  不能静默改写作者目标，也不能让模型降级。
- 历史 Run 没有 `chapter_intent` 时保持 Beta-A3 的兼容路径。

## 验收用例

1. 保存目标 → 重新打开应用后仍能读取。
2. 打开 Run-A → 修改目标 → Run-A 仍使用旧版本；新 Run-B 使用新版本。
3. Planner 未覆盖作者硬性目标时，结构化校验失败并暂停，不进入正文生成。
4. 目标与已确认事实冲突时，提示作者选择，不自动改写。
5. 历史章节（无目标）仍能正常创作。
6. 最小 GUI：章节页可编辑、保存、确认目标，并显示当前版本。

## 不在本 PR 范围

- 新的世界观管理系统；
- 目标模板库、批量目标生成；
- 目标与 RAG 的自动融合；
- 任何数据迁移。

## 交付要求

- 独立 Draft PR，仍指向 `beta`；
- 附 `docs/acceptance/` 验证记录；
- 运行 `cargo fmt`、CI 等价 Clippy、`cargo test --workspace --exclude novel-http`、
  前端 typecheck 与单测；
- 不调用真实付费模型，不做人工 GUI 结论。
