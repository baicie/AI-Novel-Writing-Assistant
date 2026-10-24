# P13-C2 Durable Knowledge Index Jobs

## 状态：PARTIAL

## Schema

`knowledge_index_job`：

- id / commandId（幂等）
- novelId / sourceType / sourceId / sourceRevision / indexVersion
- status：pending / running / retry_wait / succeeded / failed
- attemptCount / maxAttempts / nextRunAt
- lastErrorCategory / lastError
- leaseOwner / leaseUntil
- createdAt / updatedAt

## 实现

- 入队：commandId 唯一，重复投递返回既有 job；
- 领取：CAS pending/retry_wait + next_run_at，写 lease；
- 完成：只允许 lease owner 完成；
- 失败：attempt+1；未达上限进入 retry_wait，达上限 failed；
- 恢复：启动命令扫描 lease 过期 running → retry_wait；
- 手动重试：`retry_index_job` 重新领取并执行 `retry_index_document`；
- UI：索引队列、状态、尝试次数、错误、手动重试、刷新队列。

## 测试

```text
p13_index_jobs 2 passed
- enqueue_is_idempotent_and_claim_has_execution_lease
- failure_backoff_recovery_and_max_attempts_are_persisted
```

## 未完成

- 章节保存事务尚未自动 enqueue 索引 job（当前手动重建会创建 job）；
- 未实现 sourceRevision 与 job 的强校验/旧 job 不覆盖新 revision；
- 未在真实 Tauri 启动时自动运行恢复循环；
- 未做 SIGKILL/重启 Native WDIO；
- Embedding 失败分类仅保留文本错误。

因此 G5 仍为 PARTIAL。
