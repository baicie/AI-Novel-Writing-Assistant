# P7 工程审计台账

审计基线：`3f2b8a83`（本地 HEAD 审计开始时），工作树仅保留用户未跟踪的 `crates/novel-http/tests/integration_runs.rs`。
审计口径：只把本地实际代码/实际执行结果记为 VERIFIED；未执行的真实 Provider、真实 GUI、强制 Channel Lag 场景显式标记 NEEDS_MANUAL_VERIFICATION 或 BLOCKED。

## P7.1 Tauri IPC

| ID | 状态 | 严重度 | 代码位置 | 具体失效场景 | 复现 / 测试 | 修复提交 | 剩余风险 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| IPC-01 手写 IpcChannel / transformCallback | NOT_APPLICABLE | - | `client/src/platform/desktopTransport.ts:1` | 历史版本可能伪造回调 ID；当前代码使用 `@tauri-apps/api/core` 的 `Channel`/`invoke`，`rg` 无 `IpcChannel`/`transformCallback` | `rg "IpcChannel|transformCallback" client/src` = 无结果 | 1df6acb1 | 仍需防止后续新增手写 IPC |
| IPC-02 saveModelSetup 参数合同 | NEEDS_MANUAL_VERIFICATION | 中 | `desktopTransport.ts:678`；`src-tauri/src/commands/model.rs:14-24,96-136` | 参数命名为 `allowLocalhostHttp/apiKey`，Rust serde camelCase 接收；静态合同一致，但没有在真实 OS Keychain 上执行保存/读取 | 代码检查 + `pnpm typecheck`；未执行写入 Keychain | 1df6acb1 | Keychain 权限/删除行为需人工验证 |
| IPC-03 Channel broadcast Lagged 丢状态 | FIXED | 高 | `src-tauri/src/commands/run.rs:123-166` | broadcast 容量 256，消费者滞后时过去直接忽略 Lagged，可能丢 checkpoint/终态 | 新逻辑从 `last_seq` 调用 `director.replay` 补拉持久化事件；真实 Channel 基础 smoke 通过；强制 Lag 未自动化 | 1df6acb1 | 未做强制 Lag 故障注入，标 NEEDS_MANUAL |
| IPC-04 withGlobalTauri 过宽 | FIXED | 中 | `src-tauri/tauri.conf.json:13` | 全局 Tauri API 增加无必要暴露面 | 改为 `false`；Release Channel smoke 通过 | 1df6acb1 | 若未来依赖全局 API 需重新评估 |
| IPC-05 Command/handler/capability 对齐 | VERIFIED | 高 | `src-tauri/build.rs`；`src-tauri/src/lib.rs`；`src-tauri/capabilities/default.json` | 注册名/权限不一致会导致 403 或虚拟命令 | 脚本比对：52 commands / 52 handlers / 52 allow permissions，差集 0 | 1df6acb1 | 数量一致不等于业务权限最小化，见 IPC-04 |
| IPC-06 CSP / 远端内容 | VERIFIED | 高 | `tauri.conf.json:15-16` | 生产仅 self/ipc/asset/data/blob，远程模型请求在 Rust；dev CSP 单独放行 Vite | 配置检查 + Release bundle audit | 1df6acb1 | 未做 CSP 旁路扫描 |
| IPC-07 事件序号去重 | IMPLEMENTED | 中 | `client/src/platform/desktopTransport.ts:247-249,515-527` | `eventKey` 对非 token 事件按 kind+seq 去重；`replay(afterSeq=0)` 补拉 | 代码检查 + workflow replay 测试 | 1df6acb1 | `startGenerationAwaitCheckpoint` 不单独去重，但不提交正文 |
| IPC-08 Channel 释放/重载 | NEEDS_MANUAL_VERIFICATION | 中 | `desktopTransport.ts:498-506,586-595`；`run.rs:130-170` | 终态从 `liveChannels` 删除，Rust 订阅在终态/发送失败后退出；页面销毁/重载未做专门 E2E | 真实 WebView Channel smoke 只验证一条消息 | 1df6acb1 + 2ac0fbce | 需页面卸载/重载测试 |
| IPC-09 真实 WebView Channel | VERIFIED | 高 | `scripts/smoke-tauri-channel.sh`；`main.tsx` smoke hook | Rust `smoke_channel_message` -> official `Channel` -> React callback | `scripts/smoke-tauri-channel.sh` 输出 `channel smoke ok kind=run_started seq=1` | 1df6acb1 | 测试命令只用于 smoke，不是业务 API |

## P7.2 LLM Gateway / SSE

| ID | 状态 | 严重度 | 代码位置 | 具体失效场景 | 复现 / 测试 | 修复提交 | 剩余风险 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| LLM-01 无 `[DONE]` 的 EOF | FIXED | 高 | `crates/novel-llm/src/sse.rs:62-128` | 旧代码 EOF 直接返回成功，可能提交截断正文 | `eof_without_done_is_not_success`；`incomplete_stream_is_rejected_without_retry` | 1df6acb1 | 需要真实 Provider 异常流复验 |
| LLM-02 流中途截断 | FIXED | 高 | `sse.rs:83-97,122-127` | 已开始流后 EOF/读取错误必须返回 billed 错误 | 新增 EOF/error 测试 | 1df6acb1 | 未覆盖所有 Provider 自定义流格式 |
| LLM-03 `[DONE]` / normal EOF / error event 语义 | FIXED | 高 | `sse.rs:18-49,115-127` | 显式 error 事件原被忽略；现在返回 `InvalidResponse` | `explicit_error_event_is_not_ignored` | 1df6acb1 | 结构化错误分类仍可细化 |
| LLM-04 usage 缺失 | VERIFIED | 中 | `sse.rs:51-59`；`provider.rs:25-36` | `None` 保持未知，不伪造 0 | `missing_usage_stays_unknown`；Worker usage 保留 None | 1df6acb1 | - |
| LLM-05 取消网络读取 | FIXED | 高 | `sse.rs:79-82`；`provider.rs:197-216` | 旧代码在 `next_line().await` 上不能及时响应取消 | `cancel_stops_before_reading_the_rest`；Worker cancel test | 1df6acb1 | 真实 socket 层取消需人工/故障注入 |
| LLM-06 429/503/timeout 与有界退避 | IMPLEMENTED | 中 | `provider.rs:186-231`；`error.rs:24-38` | 429/503 可安全重试；超时可能已计费，故意不重试 | `retries_5xx_once_then_streams`；超时无重试代码审查 | 1df6acb1 | 真实 Provider 限流/计费无法本地验证 |
| LLM-07 非幂等生成不确定计费重试 | FIXED | 高 | `provider.rs:201-215`；`error.rs:28-34` | 连接中断/超时不再自动重发；SSE 截断不重试 | 代码 + SSE/503 测试 | 1df6acb1 | 真实计费状态不可能本地证明 |
| LLM-08 无界 Token 队列 | FIXED | 高 | `provider.rs:63,160`；`sse.rs:62-65`；`worker.rs:301`；`generation.rs:84` | 旧 `UnboundedSender` 可无限占用内存；改为 bounded `mpsc::channel(64)`，发送 await 背压 | `cargo test -p novel-llm`；workspace tests | 1df6acb1 | 64 容量是保守值，需压力测试 |
| LLM-09 并发 Run 取消串扰 | VERIFIED（现有测试） | 高 | `worker.rs:44,107-110,292-303` | cancel map 按 runId 键；现有并发/取消测试 | `cancel_generation_during_draft...`；full application tests | 1df6acb1 | 多 Worker 独立进程未做 |
| LLM-10 密钥泄漏 | VERIFIED（静态） | 高 | `provider.rs:248-275`；`generation.rs`；release log audit | `redact_secrets`；未发现 key 日志/导出 | `redact_keys...`；bundle/log scan | 1df6acb1 | trace/异常后端未做全量扫描 |
| LLM-11 Endpoint HTTPS/loopback/private/DNS/redirect/proxy | IMPLEMENTED | 高 | `endpoint.rs:7-89`；`provider.rs:128-134,169-170` | 拒绝 userinfo、非 HTTPS、内网字面量；DNS 解析后检查私网；禁用自动重定向和系统代理；HTTPS loopback 也要求显式允许 | endpoint tests + gateway local tests | 1df6acb1 | 真实 DNS rebinding/代理环境未做 E2E；`no_proxy` 会影响依赖系统代理的用户 |

## P7.3 SQLite / 保存 / 导入

| ID | 状态 | 严重度 | 代码位置 | 具体失效场景 | 复现 / 测试 | 修复提交 | 剩余风险 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| DB-01 旧库只读 | VERIFIED | 高 | `legacy_import.rs:70-76`；`invariants.rs` | 源库被写入 | `read_only` 源连接拒绝 INSERT；源文件字节不变 | 1df6acb1 | - |
| DB-02 一致性备份/WAL | VERIFIED | 高 | `legacy_import.rs:354-375`；invariants | WAL 源库备份遗漏或损坏 | 源库设 WAL；备份可打开且 Novel count=1；目标重复导入备份存在 | 1df6acb1 | 跨文件系统/磁盘满未测 |
| DB-03 路径规范化 | VERIFIED | 高 | `legacy_import.rs:45-58` | 源=目标被覆盖 | `import_rejects_source_used_as_target` | 1df6acb1 | 符号链接边界未穷举 |
| DB-04 重复导入/部分失败 | VERIFIED | 高 | `legacy_import.rs:80-120`；invariants | 重复数据/半提交 | 重复导入 count 不变；孤章节失败后 target 0 rows，transaction rollback | 1df6acb1 | 真实旧库列差异未覆盖 |
| DB-05 章节 CAS / user_edited | VERIFIED | 高 | `novels.rs:921-975`；roundtrip tests | 并发保存覆盖作者正文 | 版本冲突/保护测试；保存 smoke | 1df6acb1 | 两个窗口并发未做 GUI |
| DB-06 自动保存/切章/退出恢复 | NEEDS_MANUAL_VERIFICATION | 高 | `ChapterEditorShell`；smoke save | 切章时未完成保存可能丢内容 | Application save/reopen smoke；无真实 GUI | 1df6acb1 | 需人工 UI |
| DB-07 revision 与正式正文/SHA | VERIFIED | 高 | `legacy_import.rs` verify；invariants | 修订与正文不一致 | 正文 SHA-256、chapter_revision 计数/内容测试 | 1df6acb1 | - |
| DB-08 list_novels_paginated total 过滤 | FIXED | 高 | `novels.rs:53-66` | total 忽略 status/narrative_form/writing_mode，分页错 | `paginated_novel_total_honors_all_filters`（混合过滤/分页/空结果） | 1df6acb1 | 极端 limit=0 由 clamp 处理 |
| DB-09 旧 Run 不自动重放 | VERIFIED | 高 | `legacy_import.rs` import_runs；invariants | 迁移时产生模型费用 | 状态 paused；`remote_call_count`=0 | 1df6acb1 | 真实旧 Run 变体未全量 |

## P7.4 Workflow

| ID | 状态 | 严重度 | 代码位置 | 具体失效场景 | 复现 / 测试 | 修复提交 | 剩余风险 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| WF-01 单书单活跃 Run | VERIFIED | 高 | `workflow.rs:25-40`；index | 两个活跃 Run | existing approval/conflict test | 1df6acb1 | - |
| WF-02 Lease 领取/CAS | VERIFIED | 高 | `workflow.rs:115-170`；`workflow_lease.rs` | 两个 Worker 同时写 | `expired_lease_can_be_reclaimed_and_stale_worker_cannot_commit` | 1df6acb1 | 独立进程时钟漂移未测 |
| WF-03 旧 Lease 过期 | VERIFIED | 高 | 同上 | 旧 Worker 提交 | 同一测试 stale expected version -> Conflict | 1df6acb1 | - |
| WF-04 Approval Gate | VERIFIED | 高 | `worker.rs`；recovery tests | 自动越过人工门 | `approval_wait_is_not_claimed...` | 1df6acb1 | UI gate 操作未验收 |
| WF-05 user_edited 保护 | VERIFIED | 高 | `novels.rs:921-975`；recovery | AI 覆盖作者 | interrupted/user_edited tests | 1df6acb1 | - |
| WF-06 checkpoint/commit 中断恢复 | VERIFIED（应用层）；NEEDS_MANUAL（独立进程） | 高 | `worker.rs`；recovery tests | 进程崩溃丢候选/重复调用 | `interrupted_draft...`、`cancel_generation...` | 1df6acb1 | 真实进程 kill 未做 |
| WF-07 不确定计费不重复生成 | IMPLEMENTED | 高 | `provider.rs`；`worker.rs` | 超时后重复请求 | timeout/SSE no-retry code + tests | 1df6acb1 | 真实 Provider 无法本地证明 |
| WF-08 Event seq 补拉 | VERIFIED | 高 | `director.rs replay`；`run.rs lag recovery` | 丢关键事件 | replay tests + channel smoke | 1df6acb1 | 强制 Lag 未自动化 |
| WF-09 迁移旧 Run 不自动执行 | VERIFIED | 高 | import tests | 迁移产生未知副作用 | remote_call_count=0 | 1df6acb1 | - |
| WF-10 事务失败无半提交正文 | VERIFIED | 高 | `workflow.rs` transactions；import rollback | 半提交 | import failure rollback；storage tests | 1df6acb1 | - |

## P7.5 质量 Gate

| 检查 | 状态 | 结果 |
| --- | --- | --- |
| `cargo test --workspace --exclude novel-http` | PASS | 212 tests |
| 已跟踪 `novel-http` 目标 | PASS | 57 tests |
| `cargo test --workspace --all-targets` | BLOCKED | 用户未跟踪旧版 `integration_runs.rs`，22 个编译错误；未修改 |
| `pnpm typecheck` | PASS | 根级 |
| `pnpm build` | PASS | 既有大 chunk 警告 |
| 前端测试 | PASS | 180 |
| `cargo fmt --check` | PASS | 迁移 crates |
| `cargo clippy` | PASS（非 `-D warnings`） | 既有警告 |
| Tauri app build | PASS | 见验收文档 |
| Release bundle audit | PASS | 无 Node/Electron/sidecar |
| Channel real WebView smoke | PASS | `channel smoke ok kind=run_started seq=1` |
| 真实 GUI 输入法/自动保存/撤销/粘贴 | BLOCKED | 无可靠 GUI 输入自动化；不得以 Web smoke 代替 |
| 真实付费 LLM | BLOCKED | 无批准凭据/预算 |
