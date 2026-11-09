# Provider 整包超时与流式中断

## 背景

`OpenAiCompatibleProvider` 不再用一个 `reqwest` 整包超时覆盖整个 SSE 响应，而是由
`ProviderPolicy` 分开控制连接、首字节、流式空闲和单次请求最大持续时间：

- `connect_timeout`：连接/TLS 建立等待；
- `first_byte_timeout`：收到响应头前的等待；
- `stream_idle_timeout`：流开始后两次数据之间允许的最长空闲；
- `request_timeout`：一次请求从发送到结束的最大持续时间，旧调用方没有设置时回退到
  `timeout`（默认 120 秒）。

流式生成（`stream: true`）会把正文/结构化 JSON 一边生成一边返回。持续有数据的长响应只受
`request_timeout` 约束；空闲或总时长超限会停止当前请求，并按是否已经发出请求保留可能计费语义。

后果有三点，都需要按“可能已计费”处理：

- 模型实际已经产生了部分输出，供应商可能计费，但本地拿不到 `usage`；
- Run 在 Planner 第一步就暂停，没有任何可用产物，不能当成“模型输出不合格”；
- 预算计数器（例如 M1 执行器的 `M1_MAX_CALLS`）只消耗 1 次，很容易被误读为“什么都没发生”。

## 决策

生产桌面与验收工具使用同一套 `OpenAiCompatibleProvider` 和 SSE 消费实现，差异只通过显式
`ProviderPolicy` 表达：

- `ProviderPolicy::default()` 保留 120 秒的兼容回退值，供旧调用方和测试策略使用；
- `ProtocolRouterProvider::new()` 使用 `ProviderPolicy::desktop_default()`：单次请求最多 300 秒，
  连接 15 秒、首字节 120 秒、流式空闲 45 秒；持续输出不会因为旧的 120 秒默认值被切断；
- `ProtocolRouterProvider::with_openai_policy(policy)` 仍只覆盖 OpenAI 兼容通道，Ollama 通道保持
  自身实现；
- M1 验收执行器显式设置单次策略并把 `max_attempts` 固定为 1。任何已发送但无法确认计费的
  失败都不走免费重试；用户取消在超时和重试之前生效。

`request_timeout` 是单次请求上限，不是无限等待许可。连接、首字节、空闲或总时长失败都必须
进入可诊断的 ProviderUnavailable/人工恢复路径，并保留 usage 未知或可能已计费的事实。

## 当前规则

- 诊断先区分连接失败、首字节超时、流式空闲超时、总时长超时和用户取消；不要只看一个
  `timeout` 数字。
- 新增需要长响应的入口时，用 `ProviderPolicy` 或 `with_openai_policy` 显式传入策略，并记录
  各阶段预算来源；不要在业务服务里另写请求循环。
- 持续有 SSE 数据的响应可以超过旧的 120 秒回退值，但不能超过 `request_timeout`；长时间没有
  新数据必须由 `stream_idle_timeout` 终止。
- 用户取消优先于阶段超时、总时长超时和重试；取消后不得发送下一次请求。
- 首字节等待、流式读写和总时长达到上限后，只要请求已经发出，就保留
  `billed: true` 或 usage unknown 语义；不能改成免费重试或成本为零。
- `max_attempts` 必须是有界值。响应已开始、计费未知或发生超时的生成不自动重发；只有明确
  可安全重试且未产生新收费的 HTTP 失败才可使用剩余次数。
- 用无凭据的 `GET {base}/v1/models` 只能确认短链路可达，不能证明长生成能在阶段预算内完成；
  Mock SSE Server 测试必须覆盖持续输出、空闲、总时长和取消。

## 示例

推荐：验收执行器显式放宽，并记录实际生效值。

```rust
ProviderPolicy {
    timeout: Duration::from_secs(300),
    request_timeout: Some(Duration::from_secs(300)),
    connect_timeout: Duration::from_secs(15),
    first_byte_timeout: Duration::from_secs(120),
    stream_idle_timeout: Duration::from_secs(45),
    max_attempts: 1,
    ..ProviderPolicy::default()
}
```

禁止：

- 把 `ProviderPolicy::default().timeout` 直接改成 300 秒，让所有生产请求都变慢、变贵。
- 在流中断后自动重发同一请求来“碰运气”，把未知计费叠成多次真实计费。
- 把 `provider_unavailable` 暂停当成模型输出质量问题，进而在 Prompt 或 schema 上做无依据修改。

## 失败模式

- 真实模型在 120 秒边界被切断 → 看起来像“模型不支持 / 输出不合格”，实际是客户端超时。
- 网关模型列表全部绿色，但长生成仍失败 → 用短探测结果替代长链路结论。
- 失败后 usage 显示 0 次调用 → 误判为“没有发出请求”，实际至少一次请求可能已计费。
- 把预算单位与 Provider 重试叠加 → 实际请求数超过 `M1_MAX_CALLS` 而无人察觉。

## 相关模块

- `crates/novel-llm/src/provider.rs`（`ProviderPolicy`、阶段超时、单次总时长与有界重试）
- `crates/novel-llm/src/router.rs`（正式桌面 `desktop_default` 与显式覆盖）
- `crates/novel-llm/src/sse.rs`（`consume_sse_with_idle_timeout`、取消和 `billed` 语义）
- `crates/novel-llm/tests/gateway.rs`（Mock SSE 长流、空闲、取消测试）
- `src-tauri/src/bin/m1_run.rs`（验收执行器的策略覆盖）

## 来源文档

- Beta-A2 M1 复测执行手册：`../acceptance/BETA-A2-M1-RETEST-RUNBOOK.md`
- Beta-A2-R2 发布阻断修复：`../acceptance/BETA-A2-R2-RELEASE-BLOCKERS.md`
