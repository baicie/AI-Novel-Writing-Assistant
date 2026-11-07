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
- **新计划必须显式输出义务**：每章都要有 `obligations` 数组，且至少一条 `hard`。
  缺失、空数组或全部 advisory 会先触发一次结构化修复；仍不合格则 DataIntegrity 暂停，
  正文不会开始生成。历史计划仍走宽松读取路径，不迁移、不改写。
- 目标章节计划缺失时，计划驱动流程明确失败并暂停；预览入口返回 blocked reason。
  自由创作使用 `chapter-draft-v1`，不读取章节上下文，保持原有行为。
- Writer 上下文新增必选 `chapter_obligations` 来源；它参与 token 预算，不允许被静默裁剪。
  `context` 产物记录 `chapterPlanVersion`、`obligationIds`、`obligationsText`，供追踪。
- Writer 使用 `chapter-draft-v3`（历史 Run 保留 v2）：v3 模板自带义务槽位，
  hard 与 advisory 分区渲染；v2 不再被新 Run 使用，但模板保持可重放。
- Reviewer 使用 `consistency-review-v2`，只接收目标章节计划；违反 hard 义务的 issue 必须
  带 `obligationId`，且 ID 必须存在于当前计划。引用不存在的 ID 会暂停，不进入修复。
- **Context 按 Run 冻结**：每个 Run 只读取自身 `run_id` 写入的 Context。旧 Context 通过
  产物自带的 `run_id` 归属；如果旧 Context 没有 `chapterPlanVersion`，用 ContextTrace 中
  `chapter_list` 来源哈希定位版本。定位失败就暂停，不能改用最新计划。
- **hard 违反的证据必须可定位**：evidence 去除空白后要能在当前候选正文中找到；
  找不到就 fail-closed。该校验只证明“引用存在”，不判断语义是否正确。
- **advisory 不阻塞**：仅引用 advisory 的 issue 不触发修复、不阻塞章节完成，只作为质量提示。
- Reviewer 返回非法 JSON 时直接 DataIntegrity 停止，不把审校 JSON 修复成空 issues。
- Repair 使用 `chapter-repair-v2`，收到义务文本与冲突，明确禁止通过修改计划来迁就正文；
  hard 必须保留，advisory 可以合理取舍。

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

- Planner 没有输出 `obligations`：新计划先走结构化修复；修复后仍缺失就暂停，Writer 不会
  在没有剧情契约的情况下开始。历史计划（契约升级前冻结）仍然兼容读取。
- Planner 输出了空数组或全部 advisory：按“没有 hard 义务”拒绝，理由与缺失相同。
- Reviewer 不填写 `obligationId`：语义违反仍可能被发现，但无法确定性关联到义务；M1 评测
  应把“hard 义务违反是否引用正确 ID”作为指标。
- 审校 JSON 无效：直接暂停，不进入空 issues 的“通过”路径。
- 审校 evidence 不在当前候选：暂停，不进入修复；引用历史正文或编造原句都属于这一类。
- 同章有多个 Run：只使用本 Run 的 Context；本 Run Context 缺失时失败，不借用其他 Run 的
  上下文或更新后的计划。
- 未来章节义务：目标计划之外的内容不得进入 Writer/Reviewer 上下文。
- 旧 `context` 产物没有 `obligationsText`：Repair 使用占位说明，不伪造义务。
- 只有 advisory 未实现：记录质量提示，章节继续；不要把它写成 hard 冲突或 replan。

## 相关模块

- `crates/novel-application/src/obligations.rs`：归一化、渲染、审校引用校验。
- `crates/novel-application/src/context.rs`：目标章节计划、义务来源、ContextTrace。
- `crates/novel-application/src/worker.rs`：Planner/Writer/Reviewer/Repair 接线。
- `crates/novel-prompt/src/lib.rs`：`chapter-list-v2`、`consistency-review-v2`、
  `chapter-repair-v2`。

## 来源文档

- `docs/design/P17.4.10-CHAPTER-OBLIGATION-CONTRACT.md`
- `docs/design/P17.4.9-CI-AND-M1-ROOT-CAUSE.md`
