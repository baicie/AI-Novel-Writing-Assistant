# Beta-A3 审校可信度、正式桌面创作稳定性与作者验收闭环

## 1. 基线与分支

| 项目 | 值 |
| --- | --- |
| 起始 `beta` | `0a51abc06515866ea7197e8ef4fa8b3c07d6c388` |
| 起始 `main` | `eff69f229727ab90f4102c05cfc705116cb1ded5` |
| 依赖基线 | PR #27、PR #28 均已合并；开始时开放 PR 为 0 |
| 起始 beta CI | `Tauri CI` #42486187950 PASS（`0a51abc0`） |
| 开发分支 | `fix/beta-a3-reviewer-and-desktop-acceptance` |
| 直接修改 `main` / `beta` | 否 |
| 章节目标契约 | 未纳入本 PR，见第 13 节与后续独立 PR 建议 |

本轮没有调用真实付费模型，没有重跑 M1/M2–M10，没有发布、打 Tag 或合并。

## 2. 审查的核心模块

- `crates/novel-application/src/obligations.rs`：义务解析、审校归因、证据定位、可信度分流。
- `crates/novel-application/src/worker.rs`：Repair 触发条件、story_macro 输入、审校产物落库。
- `crates/novel-application/src/worker/worker_validation.rs`：审校 JSON 归一化与字段校验。
- `crates/novel-prompt/src/lib.rs`：`consistency-review-v2` 契约措辞。
- `crates/novel-application/src/director.rs` / `dto.rs`：`RunContract` 输入快照。
- `crates/novel-llm/src/provider.rs` / `sse.rs` / `router.rs`：请求阶段超时、SSE 空闲、取消。
- `src-tauri/src/lib.rs`：正式桌面 Provider 组合根。
- `src-tauri/src/bin/m1_run.rs`：M1 验收探针。
- `scripts/acceptance/writeback-author-edit-candidate.py`：验收写回工具边界。

## 3. 真实复现的缺陷

### 3.1 Reviewer 缺席误判会触发无效修复（P1）

失败先行用例在修复前真实失败：

- `obligations::tests::suspected_missing_element_waits_for_author_confirmation`
  —— 旧实现把 `type=missing_element` 计为 hard issue（`issues=1`）。
- `obligations::tests::insufficient_explanation_stays_visible_without_becoming_clean`
  —— 旧实现把 `needsAuthorConfirmation` 问题计入可修复问题。
- `obligations::tests::explicit_general_advisory_never_counts_as_hard_repair`
  —— 未带义务 ID 的一般建议被计为 hard。
- `obligations::tests::mixed_findings_keep_repairable_and_manual_counts_separate`
  —— 真实冲突与疑似缺席无法分开计数。
- `obligations::tests::confirmed_missing_element_remains_repairable`
  —— 旧归一化输出缺少 `manualReviewCount` 等字段。

同时发现一个实现细节缺陷：归一化产物中的 `violationType: null` 会遮蔽 `type=missing_element`，
导致疑似缺席被误判为可修复；已修正为仅在有字符串值时优先使用 `violationType`。

### 3.2 正式桌面 Provider 只有 120 秒整包超时（P1）

`ProtocolRouterProvider::new()` 供正式桌面使用，旧的 `reqwest::Client::builder().timeout(...)`
是包含响应体的整包截止时间，持续输出但总时长超过 120 秒会被本地切断；SSE 消费也没有空闲
超时，长时间无数据只能等到整包截止。修复前的 Mock SSE 用例无法编译（策略字段不存在），
说明该能力当时不存在。

### 3.3 Run 打开后修改小说简介会污染首次 story_macro（P1）

失败先行用例：创建 Run-A 后把简介从“描述 A”改为“描述 B”，旧实现会通过
`worker.rs::structured_prompt("story_macro")` 中的 `get_novel()` 读取到“描述 B”。
修复前该用例停在 `paused`（并暴露测试夹具人物引用错误，修正夹具后确认了真实缺陷路径）。

### 3.4 验收写回脚本缺少产品态校验（P2）

脚本只校验候选 ID 与 SHA，没有校验 Run 是否仍在章节确认门、Gate 是否为
`chapter_approval`。正式产品路径 `confirm_candidate_after_edit` 已具备状态、Control Version、
Gate 类型、CAS 与命令幂等校验，验收脚本属于旁路。

## 4. 修复前后差异

### 4.1 Reviewer 可信度分流

修复前：

- 所有引用 hard 义务的问题（以及无义务 ID 的问题）都计入 `issues`，直接进入有界修复。
- `missing_element` 与真实矛盾使用同一证据策略；局部引文存在即视为问题成立。
- 审校只拿到候选正文前 20,000 字。

修复后（`obligations.rs` / `worker.rs` / `worker_validation.rs` / `consistency-review-v2`）：

- 每个 issue 增加确定性归一化字段：`obligationEnforcement`、`reviewConfidence`、
  `automationDisposition`。
- `confidence` 只接受 `confirmed_violation` / `suspected_absence` / `insufficient_evidence`；
  无显式值且 `type=missing_element` 时按 `suspected_absence` 处理。
- 疑似缺席、证据不足或模型显式要求作者确认的 hard 问题进入 `manual_confirmation`：
  保留在审校产物与 quality debt，`needsAuthorConfirmation=true`，不触发自动修复。
- 显式 `enforcement=advisory` 的问题进入 `advisory`，不触发 hard repair。
- 只有 `automationDisposition=repair` 的问题计入 `issues`；新增 `manualReviewCount`；
  状态区分 `clean` / `advisory_only` / `needs_author_confirmation` / `issues_found`。
- `validate_review_evidence` 现在校验**所有** issue 的 evidence 必须来自当前候选正文，
  不只是带义务 ID 的 hard 项；但明确不把引文存在当成违规成立的证明。
- Reviewer 提示词改为要求完整阅读候选正文，并给出三类可信度与人工确认语义；Worker 不再
  把候选正文截断到 20,000 字。
- AI-first 边界：语义判断仍由模型给出；确定性代码只做已结构化输出的分类、计数与证据定位，
  没有新增关键词匹配或语义兜底。

**明确的能力边界**：系统没有可靠的自然语言理解能力去证明“全文确实缺少某要素”。
因此疑似缺席不会被自动判为 PASS，也不会被自动修复；它保留为人工确认项。

### 4.2 Provider 阶段超时

修复前：

- `ProviderPolicy { timeout, max_attempts, ... }`，`reqwest` 整包 120 秒。
- 流式读取无空闲超时；超时统一映射为 `Timeout`（计费语义为未知）。

修复后：

- `ProviderPolicy` 新增 `request_timeout: Option<Duration>`、`connect_timeout`、
  `first_byte_timeout`、`stream_idle_timeout`；旧调用方未设置 `request_timeout` 时回退到
  `timeout`（120 秒），保持兼容。
- `ProtocolRouterProvider::new()` 使用 `ProviderPolicy::desktop_default()`：单次请求最多 300 秒、
  连接 15 秒、首字节 120 秒、流式空闲 45 秒；正式桌面与验收工具共用同一 Provider 实现。
- 每次尝试由 `tokio::time::timeout(total_request_timeout)` 包裹；首字节单独计时；SSE 由
  `consume_sse_with_idle_timeout` 在两次数据之间计时。
- 取消优先级最高：在发送前、等待首字节、流式读取和重试退避中都先检查 `CancellationToken`。
- 请求已发出后的首字节/空闲/总时长失败返回 `Unavailable { billed: true }`，保留无法确认计费
  的语义；不会因网络失败无界重试，`max_attempts` 仍为有界值。
- SSRF、Endpoint、凭据安全校验未改动。

### 4.3 Run 输入快照

- `RunContract` 新增可选 `input_snapshot { title, description, novel_updated_at }`；
  `open_run` 在同一事务前读取小说详情并写入合同。
- `story_macro` 只消费本 Run 的快照；历史合同没有快照时保持明确的兼容路径（读取当前详情），
  不迁移旧数据、不改写历史产物。
- 只复制标题、简介和更新时间，不复制大对象，不重复保存计划产物。

### 4.4 验收写回脚本

- 新增 Run 状态守卫：必须 `waiting_approval`，Gate 必须包含 `chapter_approval`；
  否则拒绝写入。
- 保留候选 ID + SHA CAS 守卫与“只追加候选/事件、不改动原始候选”的语义。
- 文档明确：正式作者修改必须走 `confirm_candidate_after_edit`，脚本只用于已结束的验收 Run。
- 已写入的 M1 历史候选不受影响；原始 M1 证据未重写。

### 4.5 M1 探针

- 新增 `validate_review_contract`：校验 issues 数组存在、必填字段、义务 ID 必须来自当前 Run、
  high 问题必须有义务 ID、issueId 不重复、enforcement/confidence 合法、evidence 来自当前候选。
- 新增 `has_new_high_issues`：复验出现新的 high 问题时判定失败。
- 新增修复截断检查：空输出或以省略号结尾的修复候选不通过。
- 结果拆分 `deterministicContract` / `modelReview` / `authorReading`；`authorReading` 固定为
  `NOT_RUN`，不再把 `issues=0` 写成小说质量通过。
- 仍保持 3 次有界调用（review → repair → re-review），不自动调用真实模型。

## 5. Reviewer 误报回归结果

离线 fixture（全部通过）：

| 场景 | 用例 | 结果 |
| --- | --- | --- |
| 履约正文被误报 missing_element | `suspected_missing_element_waits_for_author_confirmation` | 不触发修复，人工确认，不冒充 clean |
| 正文确实完全缺少事件 | `confirmed_missing_element_remains_repairable` | 可修复，`issues=1` |
| 事件存在但顺序错误 | `mixed_findings_keep_repairable_and_manual_counts_separate` | 真实冲突与疑似缺席分开计数 |
| 亲眼目击被改写为事后到达 | `hard_issue_evidence_must_be_locatable_in_the_candidate` + `known_issue_review` | 证据可定位，进入修复 |
| Reviewer 引用不存在的证据 | `review_evidence_must_be_locatable_in_the_current_candidate` | fail-closed 暂停，不修复 |
| 引用正确文本但解释不足 | `insufficient_explanation_stays_visible_without_becoming_clean` | 人工确认，不 clean |
| 同一义务在不同段落满足 | 由 `confirmed_missing_element_remains_repairable` 与 evidence 定位覆盖 | 不因局部引文否定全文 |
| 多个问题部分误报、部分真实 | `mixed_findings_keep_repairable_and_manual_counts_separate` | 只修复真实冲突 |
| 一般建议 | `explicit_general_advisory_never_counts_as_hard_repair` | advisory，不修复 |
| 修复引入新问题 | `p11_repair_faults` 残留问题用例 + M1 探针 `has_new_high_issues` | 停在安全点，不提交 |

Worker 集成用例：

- `suspected_missing_element_is_manual_review_without_auto_repair`：Run 成功结束、没有
  `chapter_repair` 调用、审校产物 `manualReviewCount=1`、quality debt 可追踪。
- `advisory_only_review_does_not_trigger_repair_or_block_completion`：advisory 不修复、不阻塞。
- `review_evidence_must_be_locatable_in_the_current_candidate`：伪造证据暂停。
- `unknown_review_obligation_id_pauses_without_repair`：未知义务 ID 暂停。

安全暂停语义未被破坏：P11 租约围栏、持久取消、迟到修复结果、未知计费暂停全部通过。

## 6. Provider 超时与取消矩阵

Mock SSE Server（`crates/novel-llm/tests/gateway.rs`）：

| 场景 | 期望 | 结果 |
| --- | --- | --- |
| 持续输出，总时长超过旧 120 秒回退值 | 正常完成，不重试 | PASS `continuous_stream_can_exceed_legacy_timeout_without_being_cut_off` |
| 首字节前长时间无响应 | `billed: true`，不重试 | PASS `first_byte_timeout_is_billed_unknown_and_is_not_retried` |
| 流式输出中途长时间无数据 | `billed: true`，不重试 | PASS `idle_stream_timeout_is_billed_unknown_and_is_not_retried` |
| 持续输出但整体超过单次上限 | `billed: true`，不重试 | PASS `total_request_timeout_stops_a_continuously_progressing_stream` |
| 用户主动取消 | `Cancelled`，优先级最高 | PASS `cancellation_wins_over_a_slow_stream` |
| 5xx 且未计费 | 有界重试一次 | PASS `retries_5xx_once_then_streams` |
| 流中途断开 | `billed: true`，不重试 | PASS `incomplete_stream_is_rejected_without_retry` |
| 推理强度透传 | 请求体包含所选档位 | PASS `reasoning_effort_is_forwarded_when_selected` |

未执行：真实付费模型长响应、正式桌面 GUI 内长流式创作（见第 13 节）。

## 7. Run 作者输入冻结验证

`crates/novel-application/tests/p17_4_10_run_freeze.rs`：

- `story_macro_uses_the_run_snapshot_after_novel_description_changes`：
  1. 创建小说，简介“描述 A”；
  2. 打开 Run-A；
  3. 修改简介为“描述 B”；
  4. 推进 Run-A：story_macro 提示词只包含“描述 A”，合同 `contract_json` 只包含“描述 A”；
  5. 再打开 Run-B：合同包含“描述 B”。
- `planner_book_inputs_stay_frozen_after_author_updates`：既有计划产物冻结仍通过。
- `run_contract_rejects_a_half_pinned_planner_pair`：成对版本约束仍通过。

历史合同无 `input_snapshot` 时保持兼容分支；未要求数据迁移。

## 8. 章节目标持久化状态

**未实现（NOT_IMPLEMENTED）**。当前作者章节目标仍主要依赖小说简介与结构化大纲，没有独立的
`ChapterIntent` 持久化契约、冻结版本和最小 GUI 编辑入口。

未在本 PR 强行实现的原因：该能力需要新增作者输入模型、Planner 消费与硬性覆盖校验、
Run 冻结字段、历史兼容和前端编辑入口，属于跨阶段契约变更；按任务说明可拆为第二个独立
Draft PR。Reviewer 与 Provider 两项 P1 已优先完成并通过回归，避免大迁移阻塞主线修复。

## 9. 编辑候选提交一致性

- 正式产品路径 `confirm_candidate_after_edit` 已验证：CAS（候选 ID + SHA）、
  Control Version、Gate 必须为 `chapter_approval`、命令幂等、事务内 ResolveGate、
  正式正文只在确认后提交。用例：`p10_confirm_edit`、`p166_multi_provider_director_http`、
  `p166_i_candidate_crash`。
- 验收脚本新增 Run 状态与 Gate 类型守卫；仍不经产品 API，只在已结束验收 Run 上使用。
- 原始 M1 候选与历史证据未被覆盖或删除。

## 10. 自动化测试结果

在本机（macOS，2026-11-09）实际执行：

| 检查 | 结果 | 说明 |
| --- | --- | --- |
| `cargo fmt --check`（CI 范围包） | PASS | |
| `cargo clippy --workspace --exclude novel-http --lib -- -D warnings` | PASS | CI 等价命令 |
| `cargo test --workspace --exclude novel-http` | PASS | 全量 workspace（CI 等价） |
| `cargo test -p novel-storage-sqlite` | PASS | |
| `cargo test -p novel-storage-sqlite --features test-harness` | PASS | 3 个真实 SIGKILL 跨进程用例 |
| `pnpm --filter @ai-novel/client typecheck` | PASS | 安装锁定依赖后 |
| `pnpm --filter @ai-novel/client test` | PASS | 268 个 Node 测试 |
| `pnpm --filter @ai-novel/desktop typecheck` | PASS | |
| Tauri Bundle 构建 | NOT_RUN（本地） | 由 PR 上的 GitHub `Tauri CI` bundle 作业验证 |
| 发布脚本单测 | NOT_RUN（本地） | 未修改脚本；由 GitHub CI 运行 |

说明：首次前端检查失败是仓库缺少 `node_modules`（`typescript`/`vite` 未安装），执行
`pnpm install --frozen-lockfile` 后重跑通过；该环境问题不代表代码失败。

## 11. GitHub CI 证据

- 起始基线：`Tauri CI` run `42486187950` PASS（`0a51abc0`）。
- 本分支 CI：在本报告提交并推送后由 GitHub Actions 实际运行；结果与本报告第 16 节
  （PR 描述）中的链接为准。
- 在 CI 完成之前，本报告不声称本分支已通过 GitHub CI。

## 12. M1 质量结论及边界

- 保留 Beta-A2 已有真实结果：`PASS_PENDING_AUTHOR_REVIEW`。
- 本轮未重跑 M1，未消耗付费模型额度。
- M1 探针已加强，但增强后的探针未在真实模型上执行；离线契约测试已覆盖新断言。
- `issues=0` 只代表本次模型审校未发现问题，不代表剧情质量通过；作者阅读评分仍为
  `NOT_RUN`。

## 13. 人工 GUI 验收状态

`MANUAL_GUI_NOT_RUN`。

最短可执行清单（等待作者本人验收）：

1. 新建小说，创建第一章，设置本章目标。
2. 配置一个可用的离线 Mock Provider（或本地模型），开始一次创作。
3. 查看 Planner 计划与章节义务。
4. 查看生成候选，修改候选正文。
5. 确认并保存正式正文。
6. 关闭应用后重新打开，确认正文与历史候选保留。
7. 制造一次“疑似缺少要素”的审校结果，确认它显示为待作者确认而不是自动重写。
8. 检查审校问题与质量债务可追踪。

自动化测试不能替代真人体验，本轮未声称完成人工验收。

## 14. 未完成的 P0/P1/P2

| 项目 | 状态 | 说明 |
| --- | --- | --- |
| P1 Reviewer 缺席误判 | 已完成离线闭环 | 真实模型复测未执行 |
| P1 正式桌面 Provider 稳定性 | 代码与 Mock SSE 完成 | 真实长响应与 GUI 未验证 |
| P1 Run 作者输入冻结 | 完成 | 历史合同兼容 |
| P1 作者章节目标契约 | 未实现 | 建议独立 Draft PR |
| P2 作者修订写回 | 产品路径已验证，脚本已加守卫 | 未提供 CLI 复用 Application API 的桥接 |
| P2 M1 探针增强 | 完成 | 未真实重跑 |

## 15. 最终裁决

- ENGINEERING：READY_FOR_REVIEW（以本分支 GitHub CI 实际结果为准）
- REVIEWER_RELIABILITY：PARTIAL（确定性契约与离线回归通过；真实模型复测未执行）
- DESKTOP_PROVIDER：PARTIAL（同一 Provider 实现与 Mock SSE 矩阵通过；真实桌面长响应未验证）
- RUN_INPUT_FREEZE：PASS
- CHAPTER_INTENT：NOT_IMPLEMENTED
- AUTHOR_EDIT_FLOW：PASS（正式产品 API 验证；验收脚本仅保留为受限工具）
- M1：保持 `PASS_PENDING_AUTHOR_REVIEW`，作者阅读评分 `NOT_RUN`
- MANUAL_GUI：NOT_RUN
- PUBLIC_BETA：NOT_READY

## 16. 下一阶段 Beta RC 建议

1. 在获得授权后，用真实模型重跑一次 M1 与一次“疑似缺席”定向样本，核对人工确认路径。
2. 用正式桌面 + Mock Provider 做一次超过 120 秒的持续流式创作，确认 UI 进度与取消。
3. 实现 `ChapterIntent` 独立 Draft PR：持久化、Run 冻结、Planner 硬性覆盖、最小编辑入口。
4. 由作者完成第 13 节人工 GUI 清单后，再评估 PUBLIC_BETA。
