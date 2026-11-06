# 章节义务契约

## 背景

P17.4.9 的 M1 真实模型复验暴露了一个契约缺口：章节计划只给 Writer 一段摘要，
Writer 可以把“主角亲眼撞见老铁匠被灭口”改写成“主角到达时老铁匠已经死亡”。两者都包含
“老铁匠死亡”，但事件参与者、因果和后续影响完全不同。Reviewer 能发现这个 high 级矛盾，
但 Planner 与 Writer 之间没有结构化、可追踪的义务，Reviewer 也只能用自然语言描述问题。

## 决策

在 `chapter_list` 产物的每个章节对象上增加 `obligations` 数组，把“本章必须发生/禁止发生”
的关键约束作为规划数据传递，而不是继续堆叠 Prompt 文案。

义务只承载会改变剧情因果、参与者、时间顺序或已确认历史事实的约束；普通文学描写不升级
为硬约束。语义是否符合仍由 Reviewer 与人工/授权模型评测判断，确定性代码只负责数据完整性、
传递、引用校验和 fail-closed。

## 当前规则

- 字段：`id`、`chapterOrder`、`type`、`description`、`enforcement`、`participants`、
  `source`、`evidence`。
- `type` 允许：`required_event`、`forbidden_event`、`participant`、`causal_constraint`、
  `ordering_constraint`、`continuity_constraint`。
- `enforcement` 只允许 `hard` / `advisory`；模型缺省按 hard。
- `id` 全书唯一。模型未提供时，系统按 `ch{chapterOrder}-{类型缩写}-{序号}` 生成；
  显式重复 ID 直接拒绝，避免审校引用歧义。
- 义务位于目标章节对象内；上下文只选取目标章节计划，未来章节的义务不会进入当前上下文。
- Planner 使用 `chapter-list-v2`，上下文包含 `story_macro`、卷策略与人物阵容。
- 目标章节计划缺失时，计划驱动流程明确失败并暂停；预览入口返回 blocked reason。
  自由创作使用 `chapter-draft-v1`，不读取章节上下文，保持原有行为。
- Writer 上下文新增必选 `chapter_obligations` 来源；它参与 token 预算，不允许被静默裁剪。
  `context` 产物记录 `chapterPlanVersion`、`obligationIds`、`obligationsText`，供追踪。
- Reviewer 使用 `consistency-review-v2`，只接收目标章节计划；违反 hard 义务的 issue 必须
  带 `obligationId`，且 ID 必须存在于当前计划。引用不存在的 ID 会暂停，不进入修复。
- Reviewer 返回非法 JSON 时直接 DataIntegrity 停止，不把审校 JSON 修复成空 issues。
- Repair 使用 `chapter-repair-v2`，收到义务文本与冲突，明确禁止通过修改计划来迁就正文。
- 历史产物不迁移、不改写；旧计划没有 `obligations` 字段时按“无结构化义务”兼容读取。

## 示例

计划：

```text
第1章：林动亲眼撞见老铁匠被杀。
```

义务：

```json
{
  "id": "ch1-req-1",
  "chapterOrder": 1,
  "type": "required_event",
  "description": "林动亲眼撞见老铁匠被杀",
  "enforcement": "hard",
  "participants": ["林动", "老铁匠"],
  "source": "chapter_plan",
  "evidence": "第1章计划"
}
```

Writer 写成“林动到达时老铁匠已经死亡”违反该义务：林动不再是目击者。Reviewer 应返回
带 `obligationId: ch1-req-1` 的 issue，并引用候选正文原句。

## 失败模式

- Planner 没有输出 `obligations`：新计划按旧计划兼容读取，Writer 没有硬约束。M1 复测必须
  检查 Planner 是否实际输出义务；这是模型质量门禁，不是代码可以自动补齐的内容。
- Reviewer 不填写 `obligationId`：语义违反仍可能被发现，但无法确定性关联到义务；M1 评测
  应把“hard 义务违反是否引用正确 ID”作为指标。
- 审校 JSON 无效：直接暂停，不进入空 issues 的“通过”路径。
- 未来章节义务：目标计划之外的内容不得进入 Writer/Reviewer 上下文。
- 旧 `context` 产物没有 `obligationsText`：Repair 使用占位说明，不伪造义务。

## 相关模块

- `crates/novel-application/src/obligations.rs`：归一化、渲染、审校引用校验。
- `crates/novel-application/src/context.rs`：目标章节计划、义务来源、ContextTrace。
- `crates/novel-application/src/worker.rs`：Planner/Writer/Reviewer/Repair 接线。
- `crates/novel-prompt/src/lib.rs`：`chapter-list-v2`、`consistency-review-v2`、
  `chapter-repair-v2`。

## 来源文档

- `docs/design/P17.4.10-CHAPTER-OBLIGATION-CONTRACT.md`
- `docs/design/P17.4.9-CI-AND-M1-ROOT-CAUSE.md`
