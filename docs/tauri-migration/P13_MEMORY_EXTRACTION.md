# P13.5 AI Story Memory Extraction

## 状态

**IMPLEMENTED / PARTIAL**

## 纵向链路

正式 Chapter Revision
→ Prompt `memory-extraction-v1`
→ OpenAI-compatible 结构化输出
→ 严格 JSON/证据校验
→ `memory_extraction_candidate`（proposed）
→ 人工 Confirm / Reject / Defer / 修改
→ 正式 character_state / timeline_event / plot_thread
→ Context `story_facts`。

AI 不能直接写 confirmed 事实。

## 数据模型

migration 0012：

- `memory_extraction_job`
  - commandId 唯一（幂等）
  - novel/chapter/chapterVersion/sourceContentHash
  - extractionVersion/provider/model/endpoint/status/error
- `memory_extraction_candidate`
  - factType：character_state / timeline_event / plot_thread
  - truthfulness：narrator_fact / character_claim / rumor / hypothesis / plan / author_asserted
  - entityName/entityId、value、evidenceQuote/evidenceStart、confidence
  - status：proposed / confirmed / rejected / deferred
  - version（CAS）、ambiguous、decisionReason

状态与真实性是两个独立字段。

## 校验

- 章节必须有正式正文；
- 证据必须是正文逐字子串；
- 未知 factType/truthfulness 直接失败；
- 人物名必须唯一匹配，否则 ambiguous=true；
- 非法 JSON 直接失败，不保存候选；
- 远端失败只失败任务，不自动重试；
- 重复 commandId 返回既有任务，不重复候选。

## 人工确认

- Confirm：CAS candidate.version + 校验来源章节版本/正文 hash。
- Reject：必须填写原因。
- Defer：保留候选。
- 修改：CAS 更新 value。
- 非 narrator_fact/author_asserted 即使 Confirm 也只写 disputed 事实，Context 不读取。
- 章节已变化：Confirm 返回 Conflict，候选保持 proposed。

## Native UI

导演页长篇记忆新增：

- “从本章提取记忆”
- 候选列表：类型、真实性、版本、状态、证据
- 确认 / 修改 / 稍后 / 拒绝
- 候选写入后刷新长篇记忆。

## 测试

```text
cargo test -p novel-application --test p13_memory_extraction
3 passed

- extraction_is_candidate_only_and_confirm_reject_are_versioned
- stale_chapter_revision_blocks_confirm
- invalid_model_output_fails_without_candidates
```

覆盖：

- 人物状态候选确认后进入 Context；
- 传闻 Confirm 后不进入 confirmed Context；
- 身份歧义不能确认；
- Reject 持久化；
- 重复抽取幂等；
- 章节修改后 Confirm 冲突；
- 非法输出失败且无候选。

## 未完成

- 抽取 Prompt 只覆盖三类事实，尚未覆盖身份别名/关系/地点/能力物品的独立结构。
- 未记录独立 memory invocation/usage 表（job 保留 provider/model/endpoint）。
- Native 抽取按钮尚未 WDIO。
- 未接真实付费模型。


## Native 抽取 WDIO（本轮）

```text
test:memory-extraction 1 passing
```

流程：

1. 创建小说/章节并保存正式正文；
2. Director 启动；
3. 点击“从本章提取记忆”；
4. E2E Mock Provider 返回 timeline 候选；
5. UI 显示候选；
6. 点击确认；
7. `list_story_facts` 返回确认事实。
