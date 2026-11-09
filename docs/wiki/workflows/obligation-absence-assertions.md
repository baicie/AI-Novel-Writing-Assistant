# 义务违反的缺席断言需要可证伪

## 背景

章节审校（Reviewer）有两类判定：

1. **矛盾类**：正文写了与义务相反的内容（例如义务要求「在场目击」，正文写成「到达时人已死」）。
   这类判定可以指向具体句子，前提是「我读到了这句话」。
2. **缺席类**：正文缺少义务要求的内容（`missing_element` / `omitted_required_event`）。
   这类判定的前提是「我在全文里没有找到」。

第 2 类的判断比第 1 类脆弱得多。Beta-A2 M1 复测中出现过一次实证：

- 义务 `ch1-req-01` 要求「在废井旁拾玉，遇雪不冷，贴近炉火浮出微光纹路」；
- Reviewer 报 high，type=`missing_element`，suggestion 要求「确保废井地点要素在正文中出现」；
- 而正文**首句**就是「林动在废井边捡到一块暖玉」，全篇「废井」出现 4 次，
  另外两项要求也都已写出。

结果：这条义务在初稿里本就满足，修复只是补了三处措辞，复验自然通过。整条
「审校 → 修复 → 复验」链路跑通，但**没有验证到任何真实的修复能力**。

现有确定性防线（evidence 必须逐字来自当前候选正文）**挡不住这一类错误**：那条 evidence
确实逐字存在，也确实位于相关段落。**证据可定位 ≠ 断言为真。**

## 决策

把「缺席」当成需要完整候选正文和可信度标记的断言，而不是默认成立的事实。

- 审校保持 AI-first：语义判断仍由 AI 给出，不引入关键词匹配或固定词表替代自然语言理解。
- `evidence` 的确定性校验只证明引文确实来自当前候选正文，不能把引文存在当成缺席成立的证明。
- 直接矛盾类问题（`contradiction`、`continuity_violation`、`ordering_violation`、
  `causal_violation`）可以在 AI 明确断言且证据可定位时进入有界修复。
- `missing_element` 必须在完整候选正文上判断。局部证据无法支持全文缺席时，结果进入人工确认，
  不触发自动修复，也不伪装成 clean。

## 当前规则

- `consistency-review-v2` 要求审校完整阅读候选正文，并使用以下可信度字段：
  - `confidence=confirmed_violation`：全文足以支持确定违反；
  - `confidence=suspected_absence`：局部证据不足以证明全文缺席；
  - `confidence=insufficient_evidence`：解释或证据不足以支持结论；
  - `needsAuthorConfirmation=true`：保留人工确认路径。
- 没有明确 `confidence` 的 `missing_element` 默认按 `suspected_absence` 处理；确定性代码不会替模型
  判断同义表达是否满足义务。
- 每个 issue 的 `evidence` 都必须能在当前候选正文中定位。不存在的引文、历史正文引文和空证据
  直接 fail-closed，不能转成 PASS。
- `enforcement=advisory` 的建议只进入提示和 quality debt，不进入 hard repair。对 hard 义务，
  系统写入 `automationDisposition=repair`、`manual_confirmation` 或 `advisory`，并以 `issues`
  只统计可自动修复项；`manualReviewCount` 统计需要作者确认的项。
- 缺席类问题不得单独构成「一次修复收敛」的证据。验收报告里若要声称 Repair 能力，必须说明
  被修的问题在修复前确实成立；`M1_READY_FOR_AUTHOR_REVIEW` 也不等于作者阅读通过。
- 章节审计与自动导演链路的非阻断规则不变：疑似缺席是可追踪的质量债和人工确认项，不升级为
  replan 或全局失败。

## 示例

反例（Beta-A2 M1 实际发生）：

```text
义务：ch1-req-01 在废井旁拾玉、遇雪不冷、炉火显纹
审校：high / missing_element / evidence「他蹲下去，扒开雪。」
建议：确保废井地点要素在正文中出现
事实：正文首句「林动在废井边捡到一块暖玉」，全篇「废井」4 次
结论：前提为假；该义务已满足
```

正确做法：把完整候选正文提供给审校；如果模型不能确认全文缺席，输出
`confidence=suspected_absence` 与 `needsAuthorConfirmation=true`。系统把该判定保留为人工确认项，
不触发修复，也不计入自动修复收敛证据。

## 失败模式

- 自动化门禁全绿，但审校在为一个不存在的缺口提意见 → 修复消耗 token 却不改任何实质内容。
- 把这种「跑通」写进验收报告，导致 Repair 能力被高估。
- 用关键词兜底去判断「要素在不在」来修这个问题 → 违反 AI-first 规则，且换成同义词就失效。

## 相关模块

- `crates/novel-application/src/obligations.rs`（义务模型、审校问题归因）
- `crates/novel-application/src/worker/worker_validation.rs`（章节计划与义务覆盖校验）
- `crates/novel-prompt/src/lib.rs`（`consistency-review-v2`）
- `src-tauri/src/bin/m1_run.rs`（验收执行器的探针断言）

## 来源文档

- Beta-A2 M1 候选正文审阅：`../acceptance/BETA-A2-M1-CHAPTER-REVIEW.md`
- Beta-A2 M1 验收结果：`../acceptance/BETA-A2-M1-RESULT.md`
