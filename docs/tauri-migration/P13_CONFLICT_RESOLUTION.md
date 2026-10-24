# P13-C1 Memory Conflict Resolution

## 状态：PARTIAL

## 已实现的数据合同

- `memory_conflict`：冲突身份、类型、实体、章节、候选 ID、状态、版本、描述。
- `memory_conflict_decision`：commandId、裁决、候选、原因、操作者、冲突版本、时间。
- 候选仍存于 `memory_extraction_candidate`，不物理删除。
- 裁决后冲突 status=resolved，version+1；重复 commandId 返回既有结果。
- 裁决动作：
  - `confirm_new` / `supersede_with_evidence`：确认候选，写入正式事实；
  - `keep_existing` / `reject_extraction`：拒绝候选；
  - `mark_disputed`：候选转为 deferred，冲突标记已处理，不进入 confirmed Context。
- 旧版本 expectedVersion 返回 Conflict。
- 冲突历史保留在 decision 表。

## 检测范围

已实现：

1. 同一人物/实体、同一章节、同一 factType 的多个候选值不一致；
2. 候选来源章节 revision 已过期（stale_revision）。

未实现：

- 同一剧情时间的不兼容地点；
- 死亡后无解释行动；
- 时间线循环；
- 伏笔重复回收；
- 自由文本语义冲突。

因此不宣称完整 P13-C1 冲突检测；当前只覆盖可明确判定的候选冲突和过期来源。

## Native UI

Director 长篇记忆面板：

- 冲突列表、冲突类型、章节、候选数；
- 采用新事实 / 保留现有 / 标记争议 / 拒绝抽取；
- 已裁决数量。

未完成：来源原文证据展示、裁决历史详情。

Native WDIO：`conflict 1 passing`（真实冲突出现、采用新事实、裁决持久化）。

## 测试

```text
p13_memory_extraction 5 passed
- conflicts_are_detected_and_resolved_with_command_idempotency
```

同时修复了 `plot_thread` 在章节修订后未标记 superseded 的旧缺陷。

## 限制

- `supersede_with_evidence` 当前等价于确认新候选，未实现旧事实 supersede 链。
- 没有按剧情时间/地点/死亡状态的结构化字段，因此不把关键词判断伪装为冲突检测。

## P14.0-A 原子性修复

- 裁决、候选确认/拒绝、事实写入、旧事实 supersede、decision、conflict version 更新已合并为单个 SQLite 事务。
- `supersede_with_evidence` 必须关联旧 fact_ids，并标记旧事实 superseded。
- `keep_existing`/`reject_extraction` 拒绝全部冲突候选。
- `memory_conflict_decision.request_hash` 绑定 commandId 与请求参数；相同 commandId 不同参数返回 Conflict。
- 新增并发/stale/supersede/回滚测试。
