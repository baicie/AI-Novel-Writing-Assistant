# Provider 整包超时与流式中断

## 背景

`OpenAiCompatibleProvider` 通过 `reqwest::Client::builder().timeout(policy.timeout)` 设置超时，
这是**整个请求（含响应体）的总截止时间**，不是连接超时或首字节超时。默认 `120s`。

流式生成（`stream: true`）会把正文/结构化 JSON 一边生成一边返回。只要总时长超过该截止
时间，本地就会主动切断已经开始的流，`consume_sse` 返回
`LlmError::Unavailable { billed: true, reason: "模型流在返回中途断开" }`，上层把它映射成
`pause_signal=provider_unavailable`、`pauseKind=manual_recovery`。

后果有三点，都需要按“可能已计费”处理：

- 模型实际已经产生了部分输出，供应商可能计费，但本地拿不到 `usage`；
- Run 在 Planner 第一步就暂停，没有任何可用产物，不能当成“模型输出不合格”；
- 预算计数器（例如 M1 执行器的 `M1_MAX_CALLS`）只消耗 1 次，很容易被误读为“什么都没发生”。

## 决策

产品默认的 120 秒保持不变：它保护生产链路不被单个慢请求长期占用，也避免重试放大费用。

需要更长整包超时的场景（本地验收执行器、长结构化产物）通过**显式策略**覆盖，而不是调高
全局默认：

- `ProviderPolicy` 已包含 `timeout` / `max_attempts`，`OpenAiCompatibleProvider::with_policy`
  是既有入口；
- `ProtocolRouterProvider::with_openai_policy(policy)` 只覆盖 OpenAI 兼容通道，Ollama 通道
  保持自身默认，产品启动路径不受影响；
- 验收执行器把 `max_attempts` 固定为 1：调用预算是按真实请求占用的，Provider 内部重试会让
  一个预算单位产生多次计费请求，破坏“请求前门禁”的可信度。

## 当前规则

- 不要通过调大全局默认超时来修复单次验收失败；先判断是“模型慢”还是“网关/上游切断”。
- 新增需要长超时的入口时，用 `with_policy` / `with_openai_policy` 显式传入，并在调用方注释
  说明原因与默认值来源。
- 遇到 `模型流在返回中途断开` 时按以下顺序判断：
  1. 失败时刻与请求开始时间的差值是否正好等于 `policy.timeout`（正好相等即本地超时）；
  2. 错误是“中途断开”（已收到 delta）还是“没有开始”（未收到 delta）——后者才是鉴权、
     路径或网络问题；
  3. 用无凭据的 `GET {base}/v1/models` 确认链路可达（返回 401 说明网络与路径正常）；
  4. 网关的“协议检测/模型列表”只用短请求，不能证明长生成能在超时内完成。
- 该错误必须保留 `billed: true` 语义，不能为了让结果看起来干净而改成 `billed: false`。
- 重试属于新的付费调用，必须重新取得授权；不要设置成自动重试。

## 示例

推荐：验收执行器显式放宽，并记录实际生效值。

```rust
ProviderPolicy {
    timeout: Duration::from_secs(300),
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

- `crates/novel-llm/src/provider.rs`（`ProviderPolicy`、`with_policy`、总超时）
- `crates/novel-llm/src/router.rs`（`with_openai_policy`）
- `crates/novel-llm/src/sse.rs`（`consume_sse` 的中断判定与 `billed` 语义）
- `src-tauri/src/bin/m1_run.rs`（验收执行器的策略覆盖）

## 来源文档

- Beta-A2 M1 复测执行手册：`../acceptance/BETA-A2-M1-RETEST-RUNBOOK.md`
- Beta-A2-R2 发布阻断修复：`../acceptance/BETA-A2-R2-RELEASE-BLOCKERS.md`
