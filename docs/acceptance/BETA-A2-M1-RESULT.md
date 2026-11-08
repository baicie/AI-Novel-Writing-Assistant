# Beta-A2 M1 真实模型验收结果

## 结论

| 裁决项 | 结果 |
| --- | --- |
| M1 确定性检查 | **PASS** |
| M1 语义检查（授权代理阅读） | **PASS，待作者本人阅读评分** |
| 失败路径探针 | **PASS**（Reviewer 识别 → 一次修复 → 复验收敛） |
| 候选正文代理审阅 | **4/5，建议定向修改后定稿**；发现主样本的「一次修复收敛」是假收敛（见 2.3 与 [正文审阅](./BETA-A2-M1-CHAPTER-REVIEW.md)） |
| 作者修订稿写回候选 | **已写回**（新增 `author_edit` 候选，原始候选保留，gate 未 resolve；见正文审阅第 7 节） |
| M1 最终裁决 | **M1_PASS_PENDING_AUTHOR_REVIEW** |
| MANUAL_GUI | **NOT_RUN** |
| PUBLIC_BETA | **NOT_READY**（人工 GUI 未执行） |

本轮在 `beta` HEAD 上完成了一次有效的真实 M1 主样本与一次失败路径探针。
执行手册见 [BETA-A2-M1-RETEST-RUNBOOK.md](./BETA-A2-M1-RETEST-RUNBOOK.md)，
判定标准见 [P17.4.10-M1-RETEST-PACKAGE.md](../design/P17.4.10-M1-RETEST-PACKAGE.md)。

## 1. 基线与配置

| 项 | 值 |
| --- | --- |
| `beta` HEAD | `deae6660`（含 Provider 整包超时放宽） |
| 模型 | `deepseek-v4.1-flash` |
| endpoint | `https://gw.moonbyte.top:8443/v1`（供应商侧硬额度已由作者确认） |
| Planner | `story-macro-v2` + `chapter-list-v3` |
| Writer / Repair / Reviewer | `chapter-draft-v3` / `chapter-repair-v2` / `consistency-review-v2` |
| 上下文预算 / 输出预算 | 24,000 / 8,192；修复输出 16,384 |
| 自动重试 | 0；修复预算 1 |
| Provider 超时 | 300 秒；`max_attempts=1` |
| 调用上限 | 主样本 10，探针 3 |
| 数据目录 | `/tmp/beta-a1-m1/real`（与 Beta-A1 同一隔离目录，未写入仓库） |

## 2. 主样本

| 项 | 值 |
| --- | --- |
| Run | `c730c1fd-a4d9-41f6-ac55-e4e73d520de8` |
| 状态 | `waiting_approval`（最终确认门，未自动提交正文） |
| 暂停 | 无（`pause=None/None`） |
| 调用 | **8 次**，`unknown_usage=0` |
| token | prompt 50,430 / completion 97,229 |
| 规划门 | 2 次自动确认（人物阵容、卷策略），属执行器动作 |
| 候选 | 初稿 2,789 字 → 一次有界修复 2,810 字 |
| Reviewer | v1 报 1 条 high，v2 clean，质量债务 1 条 `resolved` |
| 产物 | `story_macro`、`character_cast`、`volume_strategy`、`chapter_list`、`context`、`chapter_draft`、`consistency_review` ×2、`repair_notes`、`chapter_approval` |

### 2.1 确定性检查

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| story_macro 有 keyEvents 且至少一条 hard | PASS | 17 条 keyEvents，覆盖第 1–12 章，全部 hard |
| 第 1 章义务非空、至少一条 hard、ID 唯一 | PASS | 6 条 hard：`ch1-req-01`、`ch1-ord-01`、`ch1-req-02`、`ch1-cau-01`、`ch1-for-01`、`ch1-con-01` |
| 每条本章 hard keyEvent 被覆盖 | PASS | KE-001…KE-006 逐条对应 6 条义务，无遗漏 |
| sourceEventId 指向本章事件 | PASS | 6 条义务的 `sourceEventId` 均属第 1 章 |
| 义务未改写规范事件 | PASS | description、participants、type 与 keyEvent 完全一致 |
| 无未来章节义务进入上下文 | PASS | `context.obligationIds` 与第 1 章计划完全一致 |
| `chapterPlanVersion` 与计划版本一致 | PASS | 均为 1 |
| Reviewer 引用合法义务 ID | PASS | `ch1-req-01` 存在于本 Run 计划 |
| Reviewer 证据可在当前候选定位 | PASS | 「他蹲下去，扒开雪。」逐字存在于初稿 |
| 修复后复验 | PASS | v2 `issues=0`，义务 ID 保持 |

### 2.2 语义检查（授权代理阅读）

| 检查 | 结果 | 证据摘要 |
| --- | --- | --- |
| 林动在灭口发生时在场 | PASS | 「绕到侧面，从后窗翻进去……缩进风箱后的阴影里」「从风箱和墙的缝隙里看出去」 |
| 老铁匠当场被杀而非此前已死 | PASS | 灰衣人索要账册无果后「右手的窄刀从老人肋下送了进去」，血落在炉灰上冒热气 |
| 目击成为卷入旧案的因果起点 | PASS | 灰衣人搜身未果后逼近风箱，林动携玉逃出，身后被追；结尾「雪雾里传来不急不缓的脚步声」 |
| 顺序正确 | PASS | 废井拾玉 → 携玉去铺 → 目睹灭口 → 被发现 → 逃亡 |
| 未提前泄露后续真相 | PASS | 未交代主谋、玉佩来历与林动身世；只留账册、炉号与「藏好」两个谜面 |
| 可读性 | PASS（代理评估） | 雪夜氛围稳定，视角统一，追逐钩子有效；作者本人评分未记录 |

### 2.3 观察项

1. **Reviewer 的首条 high 判定前提为假（代理正文审阅确认）**：v1 以「他蹲下去，扒开雪。」
   作为违反 `ch1-req-01`（拾玉）的证据，suggestion 要求「确保废井地点要素在正文中出现」。
   但初稿首句即为「林动在废井边捡到一块暖玉」，全篇「废井」出现 4 次；`ch1-req-01` 要求的
   「遇雪不冷、炉火显纹」在初稿中也已具备。**该义务在初稿里本就满足。**
   初稿 → 候选的全部差异只有 3 行措辞（首句加「镇西」「井台旁」、井口描述加「井台上
   积着厚雪」、动作句改为「他蹲在井口石板旁」），没有改变任何事件、顺序或参与者。
   结论：本轮**不能**把「一次修复收敛」当作 Repair 能力的证据——修复是在为不存在的缺口
   补字，复验自然通过。Repair 的收敛能力仍只由失败路径探针（人工构造的违规稿）覆盖，
   真实候选上的误判修复未被验证。完整分析见
   [BETA-A2-M1-CHAPTER-REVIEW.md](./BETA-A2-M1-CHAPTER-REVIEW.md) 第 3 节。
2. **关键事件粒度变细**：本轮 Planner 把第 1 章拆成 6 层义务（拾玉、顺序、目击、因果、
   禁止事后到达、不提前泄密），并规划到第 12 章。相比 Beta-A1 的 6 条事件型 keyEvents，
   约束更完整，也让 Writer 的正文直接落在「风箱后目击」这一具体场景上。
3. **写作质量受义务驱动**：正文出现了账册、炉号、旧案押运等由 keyEvents 派生的细节，
   说明义务不只是审校清单，也实际影响了正文内容组织。

## 3. 失败路径探针

用主样本的冻结上下文与一份「事后到达」的违规候选，验证 Reviewer 的识别能力与一次有界
修复是否收敛。共两轮，第二轮为有效结果。

### 3.1 第一轮：探针自身缺陷（已修复）

第一轮 `M1_PROBE_REVIEW issues=5 reviewer_cites_req1=false`，判定 `M1_FAIL_REVIEWER`。
核查发现**Reviewer 的行为是正确的**，失败来自探针断言：

- 探针把期望义务 ID 写死为 `ch1-req-1`（旧计划 ID），而本 Run 的义务 ID 是
  `ch1-req-01` / `ch1-req-02`；
- 5 条问题的 `obligationId` 全部来自本 Run 计划，`evidence` 全部逐字来自违规候选
  （逐条校验通过），其中 `ch1-for-01`（禁止事后到达）判 critical，正是本轮要考的点。

修复：新增 `expected_violation_ids()`，从本 Run 的 `chapter_list` 派生第 1 章第一条
`forbidden_event` 与 `required_event` 的 hard 义务 ID；计划里没有可断言 ID 时退化为
「任意一条带合法 ID 且证据可定位的违反」。输出字段改名
`reviewer_cites_expected` / `residual_expected`。

同时删除了 `witness_marker` 关键词判定（要求正文含「亲眼」且含「灰斗篷/凶手」）：修复稿
用「看见」「灰衣人」，语义上已补上风箱后目击，硬编码词表只会产生假阴性。该判定改由复验
阶段的 AI 审校结论承担。

### 3.2 第二轮：有效结果

| 项 | 值 |
| --- | --- |
| 期望 ID | `forbidden=ch1-for-01`，`required=ch1-req-01` |
| Reviewer | 4 条 issue，全部 critical，ID 合法，evidence 全部逐字可定位 → **PASS** |
| Repair | 2,115 字，输出未触顶（预算 16,384） |
| 复验 | `issues=0`，无残留期望项 → **PASS** |
| 裁决 | `M1_READY_FOR_AUTHOR_REVIEW` |
| 调用 | 3 次；review1 7,280/3,515，repair 8,178/9,330，review2 8,737/10,731 |

修复后正文确实补上了在场目击：林动被周铁山推到风箱后，从缝隙看见灰衣人索要账册、行凶、
取走旧册，随后被发现并逃亡。语义检查通过。

另附一条观察：复验返回 `issues=0`，但复验模型对修复稿的一次阅读中，若严格按
`ch1-req-01` 的「遇雪不冷、贴近炉火浮出微光」逐项核对，修复稿只写了「不冰手」与
「靠近炉口火光透进去」，表述与原义务措辞不完全对应。这属于措辞等价而非事件缺失，
记为质量观察，不改判。

## 4. 本轮付费调用汇总

| 阶段 | 请求数 | 结果 |
| --- | --- | --- |
| 第一次主样本（`7a7829ab`） | 1 | 120 秒整包超时中断，`usage` 未知，可能已计费 |
| 主样本（`c730c1fd`） | 8 | 完成，usage 完整 |
| 探针第一轮（探针缺陷） | 1 | Reviewer 完成，探针断言失败 |
| 探针第二轮 | 3 | 完成，usage 完整 |
| **合计** | **13** | 12 次拿到 usage，1 次未知 |

主样本与探针均在调用上限内，未触发 `M1_BUDGET_STOP`。费用以供应商账单为准；
本地只记录 token，不写入仓库。

## 5. 未完成与后续

1. **作者本人阅读评分**：候选停在最终确认门，未提交正文。需要作者按复测包评分维度记录
   1–5 分与修改成本；本轮只有代理阅读结论。
2. **人工 GUI 验收**：`MANUAL_GUI_NOT_RUN`，不能用本轮结果替代。
3. **Reviewer 的 `missing_element` 类判定需要加固**：本轮已实证一次前提为假的判定
   （见 2.3 观察项 1）。建议在义务链路上增加「缺席断言可证伪」检查，并区分
   `missing_element` 与 `contradiction` 两类问题的默认严重度。
4. **M2–M10**：`NOT_RUN`，需单独授权与评测计划。
5. **P1 数据模型缺口**：作者「当前章节目标」仍无独立持久化字段（沿袭 Beta-A2 记录）。

## 6. 未改动项

- 未修改 `main`，未创建 Tag/Release，未推送本轮提交。
- 未进入 M2–M10，未进入 P17.5。
- 产品默认 Provider 超时仍为 120 秒；300 秒只作用于验收执行器。
