# P8 自动导演业务合同

基线审计：`59d472a6`，实现提交见 `6959dbfd`、`219ed033`。本文件只记录已从本地旧版代码核对的入口；未核对到的细节明确写为 PARTIAL，不按步骤名称猜测。

## 总表

| Step | 旧版入口 | 输入 | 旧 Prompt / Schema | Rust 当前 | 前端 | 验证 |
| --- | --- | --- | --- | --- | --- | --- |
| story_macro | `server/src/modules/director/steps/planning/storyMacro.ts`；`server/src/services/novel/storyMacro/StoryMacroPlanService.ts`；`server/src/app/director/productionComposition.ts` | Novel Seed、`storyInput`、项目上下文、锁定字段 | `server/src/prompting/prompts/novel/storyMacro.prompts.ts`，`novel.story_macro.decomposition` v1；`StoryMacroPlan` / `StoryExpansion` / `StoryDecomposition` | `story_macro` 结构化 Step，`story-macro-v1`，校验 expansion/decomposition/constraints/issues 并持久化 artifact | 旧 DirectorNext 页面仍 NOT_MIGRATED | `p8_director_loop`、4 个 schema validation 单测 |
| character_cast | `server/src/services/novel/characterPrep/characterCastGeneration.ts`；`server/src/app/director/productionComposition.ts` | Story Macro、角色槽位、项目人物约束 | `server/src/prompting/prompts/novel/characterPreparation.prompts.ts`，`novel.character.castAuto`；`characterCastAutoResponseSchema`（README 中成员 3–6、关系 2–12） | `character_cast` 结构化 Step，`character-cast-v1`，至少 3 人/2 关系；Gate 产物 draft，确认后 confirmed | 旧页面未接入；无拒绝/重新生成动作 | `p8_director_loop`、schema 单测 |
| volume_strategy | `server/src/services/novel/volume/volumeGenerationSchemas.ts`；`server/src/app/director/productionComposition.ts` | Story Macro、Character Cast、预计章节数、卷数范围 | `server/src/prompting/prompts/novel/volume/strategy.prompts.ts`，`novel.volume.strategy`；VolumeStrategy 结构 | `volume_strategy` 结构化 Step，`volume-strategy-v1`，volumeOrder 唯一、非空卷 | 旧页面未接入 | `p8_director_loop`、重复卷序号单测 |
| chapter_list | `server/src/app/director/productionComposition.ts`；`server/src/prompting/prompts/novel/volume/chapterList.prompts.ts` | Volume Strategy、Character Cast、目标章节范围 | `novel.volume.chapter_list` v9/v10；`VolumeChapterPlan` | `chapter_list` 结构化 Step，`chapter-list-v1`，chapterOrder 唯一、卷必须存在、人物引用必须存在 | 旧页面未接入 | `p8_director_loop`、空/重复/引用单测 |
| context | `server/src/prompting/prompts/novel/context/*`；旧 Context Broker/章节上下文策略 | 当前章节计划、Story Macro、Cast、世界/角色状态、已确认历史、用户知识库、Token Budget | 旧版多组 Context Block / Policy；Rust 尚未完整移植 | `context` 从上游 artifacts 组装 `storyMacro/characterCast/volumeStrategy/chapterPlan/targetChapterId/tokenBudget`；未接世界/角色状态/历史/知识库 | 旧页面未接入 | `p8_director_loop` 的 context artifact |
| consistency_review | `server/src/prompting/prompts/novel/review.prompts.ts`，`novel.review.chapter` v2；旧审校/质量链 | 候选正文、章节计划、人物、故事约束 | 旧版 Issue 结构；Rust `consistency-review-v1` 为兼容子集 | 结构化 `issues[]`（type/severity/evidence/suggestion），空数组合法，模型/解析失败 pause，不把失败当 clean | 旧页面未接入 | `p8_director_loop`（空问题）、malformed/非空单测 |
| bounded_repair | 旧版章节修复/质量债链路 | consistency issues、原始候选、修复预算 | 旧版 repair/acceptance 合同（未完整迁移） | `bounded_repair` 仍为占位 plumbing；未实现真实修复循环 | 无 | 未验证 |
| approval | `DirectorGate` / 旧 API approval | gate artifact、controlVersion、commandId | 旧 `waiting_gate`；Rust `waiting_approval` | `approve` 用 commandId + CAS 将 draft gate artifact 置 confirmed；无独立 reject 命令 | 旧 Gate UI 未接 Tauri | `p8_director_loop`、recovery gate test |
| commit | 旧章节提交/正文持久化 | chapter_approval、candidate、revision | 旧 chapter promote 语义 | `commit` promote candidate，保护 user_edited；CAS/事务 | 未接 Tauri UI | `p8_director_loop` 重开正文/revision |

## 逐 Step 合同

### Story Macro

- 旧实现入口：上述 planning service 与 `productionComposition.story_macro` handler。
- 输入：小说 Seed（当前 Rust 用小说标题+描述作为 `story_input`）、故事描述、项目上下文；旧版还使用项目设定/锁定字段。
- Prompt：`novel.story_macro.decomposition` v1；Rust 使用 `story-macro-v1`，Prompt 语义按旧版“故事引擎”约束重写为同一 schema 族，不是逐字复制。
- Schema：`expansion`（expanded_premise、protagonist_core、conflict_engine、conflict_layers、mystery_box、emotional_line、setpiece_seeds、tone_reference）+ `decomposition`（selling_point、core_conflict、main_hook、progression_loop、growth_path、major_payoffs、ending_flavor）+ `constraints` + `issues`。
- 持久化：`workflow_artifact(type=story_macro, scope=book, status=confirmed)`；payload 保存规范化 JSON；版本由 artifact version 记录。
- 审批：非 gateable；用户修改保护尚未在此 Step 接入。
- 失败/重试：模型错误 pause；非法 JSON/schema mismatch pause，不静默成功；未实现有限自动修复。
- Rust 状态：**IMPLEMENTED（结构化；旧 Prompt 语义 PARTIAL）**。
- 前端：**NOT_STARTED**（旧 DirectorNext 仍走旧 API）。

### Character Cast

- 旧入口：character cast generation + `productionComposition.character_cast`。
- 输入：Story Macro、角色槽位、项目人物约束。
- Prompt：`novel.character.castAuto`；Rust `character-cast-v1`。
- Schema：`members`（至少 3）与 `relations`（至少 2）；成员含 name/role/castRole/storyFunction/personality/outerGoal/innerNeed；关系含 sourceName/targetName/surfaceRelation/conflictSource。
- 持久化：`character_cast` draft artifact；Gate 确认后 confirmed；人物事实表未写入，规划产物与人物事实保持分离。
- 审批：Gate `character_cast`；确认当前可用；拒绝/重新生成按钮和后端 reject 命令未实现。
- 失败：schema mismatch pause；上游缺失 blocked。
- Rust 状态：**IMPLEMENTED（生成/确认）；PARTIAL（拒绝、重新生成、人物事实落库）**。
- 前端：**NOT_STARTED**。

### Volume Strategy / Chapter Plan

- 旧入口：`server/src/prompting/prompts/novel/volume/strategy.prompts.ts`、`chapterList.prompts.ts` 与 production composition。
- 输入：Macro、Cast、章节预算。
- Schema：卷 `volumes[]`（volumeOrder/title/roleLabel/coreReward/planningMode）；章节 `chapters[]`（chapterOrder/title/volumeTitle/summary/characterNames）。
- 持久化：`volume_strategy`、`chapter_list`；目前为 book scope 的 draft/confirmed gate artifact，不写旧版 Volume/ChapterPlan 关系表。
- 审批：两个 gateable Step；Gate 确认后继续。
- 校验：空章节、重复 order、未知卷、未知人物会被拒绝；单测覆盖空/重复，P8 纵向覆盖正常引用。
- 重新规划/人工编辑保护/受影响下游局部失效：**NOT_IMPLEMENTED**。
- Rust 状态：**IMPLEMENTED（结构化最小合同）；PARTIAL（旧版卷/章节关系与重规划语义）**。

### Context Assembly

- 旧入口：`server/src/prompting/prompts/novel/context/*`、旧 Context Broker 策略。
- 当前 Rust：从上游 artifact 组装固定 JSON；Token Budget 仅记录，不做完整优先级裁剪。
- 未接入：世界设定表、角色状态、已确认历史章节正文、用户知识库选择、Context Trace 敏感正文脱敏。
- 状态：**PARTIAL**。

### Consistency Review / Bounded Repair

- 旧入口：`novel.review.chapter` v2 及旧质量/修复链。
- Rust：模型必须返回 `{"issues":[...]}`；每项校验 type/severity/evidence/suggestion；空数组表示 clean；模型错误或 JSON/schema 错误 pause，不写 `issues:0`；payload 同时记录 `issues` 数量供 workflow guard 使用。
- 未实现：真实旧版审校 Prompt 的完整问题分类、bounded repair、质量债写入、人工修订历史。
- 状态：**IMPLEMENTED（审校最小合同）；PARTIAL（旧版质量链/修复）**。

### Approval / Commit

- Rust API：`open_run`、`approve(commandId)`、`cancel`、`resume(acceptCandidate)`。
- `waiting_gate` 映射为 Rust `waiting_approval`；CommandId 幂等；controlVersion 不直接暴露给前端时由 `approve` 的 CAS 与 command replay 保证幂等。
- `approve` 只确认 gate artifact；`cancel` 不确认；拒绝/重新生成缺少独立命令。
- Commit 只 promote 候选；`user_edited`/protected 正文不可覆盖。
- 状态：**IMPLEMENTED（approve/cancel/commit）；PARTIAL（reject/regenerate）**。

## 兼容性结论

- Rust 已消灭 `story_macro`、`character_cast`、`volume_strategy`、`chapter_list`、`context`、`consistency_review` 的 `{ok:true}` / 固定 `issues:0` 占位。
- 目前是“旧版核心 schema 的可验证子集”，不是逐字复刻旧 Prompt；因此不能宣称业务语义 100% 兼容。
- 旧版丰富 Prompt、World/State/Knowledge Context、Bounded Repair、用户重规划和 Director UI 仍是后续工作。
