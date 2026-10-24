# 阻塞清单

## B-001 工作树中的未跟踪旧版运行测试

- 文件：`crates/novel-http/tests/integration_runs.rs`。
- 来源：Git 没有任何跟踪历史，属于用户工作树中的未提交文件；本次没有修改、移动、暂存或删除。
- 内容：9 个旧版 Run 生命周期测试，使用旧 HTTP 契约（`run_type`、`approved`、`resumed`、`confirm-event`）和旧 `TestContext` 方法（`create_test_novel`、`query_run_by_id`、`query_all_runs`、`query_confirmed_event`、带 novel_id 的 `count_runs`）。
- 阻塞：`cargo test --workspace --all-targets` 编译该测试目标时产生 22 个错误；当前新 Run API 已改为 commandId/contract/approval 语义，不能仅补辅助方法安全修复。
- 处理：保留原文件不动；已用分 crate + 已跟踪 HTTP 测试目标验证，当前通过 212 + 57 个测试。下一动作需用户决定保留、重写或删除该实验测试。
## B-002 真实模型调用未验证

- 现象：默认使用 Mock/本地 HTTP SSE 测试；没有可用的付费凭据，也不允许自行发起真实付费请求。
- 影响：Provider 真实 endpoint 的模型差异、usage 字段、限流行为仍需用户环境验收。
- 下一动作：在用户批准的测试 Key 和预算下做一次短请求，记录用量与错误分类。

## B-003 macOS GUI 交互未做自动化验收

- 状态：**BLOCKED**。
- 现象：已启动 Release `.app`、验证 SQLite/WebView Command 和 Channel；当前会话没有可靠的 macOS GUI 输入自动化环境，没有做输入法/自动保存/切换章节/撤销重做/粘贴的真实操作。
- 影响：WKWebView 中文输入法、Plate 自动保存、长文滚动和选择行为仍需人工检查。
- 未用 Web smoke 代替 GUI 验收。
- 下一动作：按 P3 清单做一次新建书→新建章→编辑→保存→切章→撤销/重做→粘贴→关闭重开读回的人工验收。

## B-004 大量旧 UI API 尚未 Tauri 化

- 现象：自动导演 UI、任务中心写操作、知识库管理写操作、图像、短剧等仍调用 `/api/*`；任务中心运行只读投影和知识库文档列表已接 Tauri；Rust 后端已新增结构化导演 Step 和 typed `openRun/approve/cancel/replay/subscribe` transport。
- 处理：集中适配器对未知路径返回 `NOT_MIGRATED` 501，不返回假成功。
- 影响：P8.4 已交付 Tauri Director MVP，但完整旧 Workspace、角色审阅、拒绝/重生成和真实 GUI 操作验收仍未完成；不能宣称完整页面闭环。
- 下一动作：按 `P8_NATIVE_UI_CONTRACT.md` 逐模块迁移；不得以 Node sidecar 补齐。

## B-005 macOS DMG 打包

- 现象：`cargo tauri build --target aarch64-apple-darwin --bundles app,dmg` 的 `.app` 阶段成功，`bundle_dmg.sh` 失败。
- 已绕过：使用 `hdiutil create -srcfolder ... -format UDZO` 从同一 `.app` 生成 8.8 MiB DMG，`hdiutil verify` 校验通过。
- 分类：本机构建环境的 Tauri DMG bundler/磁盘镜像脚本问题，不是 Rust 编译错误。
- 处理：保留成功的 `.app`；未尝试签名、公证或上传。
- 下一动作：在交互式 macOS 构建机或 CI 上重跑 DMG，检查 `hdiutil`/AppleScript/Finder 权限；完成后再做签名与公证。

## B-006 根级 `pnpm typecheck` 默认 Node 堆不足（已解决）

- 现象：默认 Node 堆下 `pnpm typecheck` 的 server `tsc` 进程 abort（exit 134）。
- 复现：`pnpm typecheck`；单独 server 同样失败。
- 解决：使用 `NODE_OPTIONS=--max-old-space-size=8192 pnpm typecheck` 后 server、client、desktop 三个 typecheck 全部通过。
- 已解决：根 `typecheck` 脚本和 Tauri CI 均设置 `NODE_OPTIONS=--max-old-space-size=8192`，`pnpm typecheck` 实际通过。

## B-007 P8 业务语义未完全迁移

- 状态：**PARTIAL**。
- 已迁移：Story Macro、Character Cast、Volume Strategy、Chapter List 的结构化最小合同；Context artifact 组装；Consistency Review 结构化 issues；Gate/Commit。
- 未迁移：旧版逐字 Prompt/完整 Schema、拒绝/重新生成、人工重规划与受影响下游局部失效、World/State/History/Knowledge Context、完整 Bounded Repair、质量债闭环、真实 Provider 验证。
- Bounded Repair 已 fail-closed：审校有问题时暂停，不生成假修复产物或提交正文。
- 处理：后续按 Step 增补合同测试，不得用 `{ok:true}` 或固定 `issues:0` 冒充。

## B-008 P8.4 真实 UI 操作验收未完成

- 状态：**PARTIAL / BLOCKED**。
- 已完成：Tauri Director 路由、启动表单、状态/事件/产物/候选展示、approve/cancel/resume 按钮、运行历史；Release WebView 路由与启动表单 smoke 通过。
- 未完成：通过真实按钮启动 Run、审批、取消、恢复、完成后重开历史的自动化或人工操作验收；真实模型配置/Provider 未验证。
- 处理：保留 `TauriDirectorPage` 和契约测试；人工 GUI/真实 Provider 验收前不标记 P8.4 为 VERIFIED。

## B-009 Native Director 真实按钮点击自动化

- 状态：**VERIFIED（核心操作）；边缘用例 PARTIAL**。
- 已建立 macOS WDIO embedded WebDriver：WebdriverIO 9.31.9、`@wdio/tauri-service` 1.5.0、`tauri-plugin-wdio` 1.5.0、`tauri-plugin-wdio-webdriver` 1.5.0。
- 已通过真实按钮：启动、Character Cast Gate、Volume Strategy Gate、Chapter Approval、Commit、Cancel、Safety、页面刷新、独立进程重启。
- 未覆盖：双击/双窗口、Channel 断线重连、过期版本 UI、中文拼音输入法 composition、撤销/重做。
- 稳定性：director E2E 连续 3 轮通过（14–24s），无 flaky。
- E2E 使用隔离临时数据目录与本机 HTTP/SSE Mock Provider，不使用真实付费模型。

## B-010 中文输入法 composition 与双窗口人工验收

- 状态：**BLOCKED / PARTIAL**。
- 已自动验证：本地草稿 SQLite 跨进程恢复、SIGKILL 恢复、保存竞态、切章保存。
- 未自动验证：macOS 拼音 composition 候选/光标、真实双窗口冲突 UI、窗口关闭按钮、AI Commit 与人工保存同秒冲突。
- 处理：按 `EDITOR_ACCEPTANCE.md` 执行人工验收；不得把普通 WebDriver 中文字符输入当成输入法组合输入通过。

## B-011 P9 Context / Repair / Decision 剩余能力

- P9.1 Context：真实基础来源已接入；旧版完整 Context Broker、知识库/RAG、独立用户补充设定、真实 tokenizer 未迁移。
- P9.2 Review/Repair：单次有界修复 + 质量债已实现；修复超时/SSE/取消/崩溃 E2E、residual pause 后 tick 错误传播仍需处理。
- P9.3 Decisions：Reject/Regenerate/Confirm After Edit 后端已实现，Reject/Regenerate Native E2E 通过；Confirm After Edit 编辑 UI、双窗口/断线/审批取消竞态未完成。
- 不得将 P9 当前 Mock 证据宣称为完整旧版自动导演或真实付费 Provider 验收。

## B-012 P9 最终剩余

- P9.1 旧 Context Broker/RAG/独立用户补充设定/真实 tokenizer 未迁移。
- P9.2 repair timeout、SSE 中断、取消后迟到、进程崩溃恢复未专门 E2E；residual pause 后 tick 可能返回一次 Conflict。
- P9.3 Confirm After Edit 编辑 UI、Regenerate 的过期 Gate/双窗口/断线竞态 E2E 未完成。
- 非 Mock 的真实 Provider、真实写作质量与完整旧版功能未验收。

## B-013 P10 剩余

- P10.1 repair timeout/SSE EOF/cancel-after-repair、repair candidate 落库后进程崩溃、re-review 后崩溃、Lease 过期 repair 专项测试未完成。
- P10.2 accepted quality debt、issue 跨审校可靠关联、完整 Context Broker/RAG、promptText 导出审计未完成。
- P10.3 双窗口同时编辑、编辑中取消、保存候选失败、正式章节被外部更新未自动化。
- 真实 Provider/计费/真实用户库未验收。

## B-014 P11 未完成

- P11.1 Repair 跨进程 SIGKILL、Lease 过期、真实 timeout、重复 Cancel 未完成；Gate 保持 PARTIAL。
- P11.2 Candidate Edit SQLite Journal 已实现并通过 SIGKILL 恢复；两窗口/Gate 变化/失败注入仍未完成，Gate PARTIAL。
- P11.3 Review Issue Occurrence / accepted Command+UI / 稳定跨轮关联未实施；Gate PARTIAL。
- 不得把真实 HTTP EOF/Cancel 测试宣称为完整跨进程恢复。

## B-015 P11.1-H0 后续缺口

- Repair Candidate SIGKILL checkpoint 已验证；Re-review 完成后 SIGKILL、RemoteStarted 未知、Lease 过期旧 Worker、真实 timeout、持久化 Cancel 与 Candidate 竞争仍未完成。
- P11.2 Candidate Edit Journal 未开始。
- P11.3 Quality Debt 生命周期未开始。

## B-017 Candidate Journal 双窗口

- Data contract：不同 writer 独立记录、CAS、清理隔离、旧草稿发现已通过 SQLite 测试。
- Native 双窗口同时编辑未完成：嵌入式 WebDriver 当前测试驱动只控制单窗口；标记 PARTIAL。
- Gate 变化 UI 仅禁用恢复/确认，未做独立视觉验收。

## B-018 P11.1-H1

- v3 Repair Candidate 已保存安全续跑已通过（P11.1-H1）。
- RemoteStarted 未知仍安全暂停。
- 尚未覆盖 Re-review 中途崩溃、Lease Fencing、真实 Provider timeout、持久化 Cancel 竞争。

## B-019 P11.1-H2-A

- Lease Fencing 写入完整性已通过两个 Worker 竞争与 Repository 级测试（`e6ee266d`）。
- 仍未覆盖：真实进程重启后的 Lease 接管、Re-review 中途崩溃、Provider timeout、持久化 Cancel 与 Commit 竞争。

## B-020 P11.4-A

- 自动续约、长请求、重复 Claim 已通过。
- 仍未完成：持久化 Cancel 竞争、Re-review 中途崩溃恢复、Candidate/Attempt/Invocation 单事务。

## B-021 P11.4-B/C

- 持久化 Cancel 竞争、Re-review 完成后 SIGKILL、Candidate/Attempt/Invocation 原子落库已通过。
- 仍未覆盖：Cancel 与 Commit 同一毫秒的数据库级竞争、Re-review Provider 返回后未落地时的中途崩溃自动复用、真实付费 Provider timeout。

## B-022 P11.3

- Occurrence/稳定 key/accepted CAS 已实现。
- 未完成：同一问题跨两轮审校端到端测试、完整质量债历史 UI、resolved 与 Review Artifact 外键。

## B-023 P13

- P13.0/P13.1 索引生命周期已修复；SearchHit 未返回 sourceRevision。
- P13.1 正式章节保存未自动触发索引重建。
- P13.2 最小故事事实读取/Context/30 章测试已实现；写入 Command/UI、事实状态流转、早期章节修改失效未完成。
- P13.3 稳定身份/accepted 重现测试已补齐；完整对抗检索集与指标未完成。
- P13.4 1000 章 Context/索引性能基线已采集；Native 验收未执行。
- 正式章节保存/晋升/删除已自动同步索引；知识管理 UI 与后台重试未完成。

- P13.2 写入 Command 已有，记忆管理 UI 与重新抽取未完成。
- P13.3 仅有小型确定性 adversarial fixture，无大规模标注集/真实模型指标。

- P13.2 故事事实写入与章节修改失效已实现；记忆管理 Native UI/重新抽取未完成。
- P13.4 最新 Director E2E 4 passing；知识库管理 UI 与真实双窗口仍未验收。

- P13.1 失败重试 API 已实现；索引状态/重试 UI 未完成。
- P13.2 记忆只读 Native 面板已实现；编辑 UI 未完成。

- P13.4 最新 Director E2E 4 passing；记忆编辑/索引重试 UI 未验收。

- P13.2 已支持导演页记录时间线事实；人物状态/伏笔编辑 UI 未完成。

- P13.4 最新 Director E2E 4 passing；知识库索引状态 UI、记忆人物状态/伏笔表单未完成。

- P13.2/P13.1 基础编辑/重试 Native UI 已有；AI 抽取、真实双窗口、大规模 adversarial 集未完成。

- P13.4 1000 章增量索引收敛已测（6ms）；并发检索/真实双窗口未测。

- P13.2 三种记忆表单 UI 已有但未做 WDIO 提交验证。

- P13.2 时间线表单已通过 Native WDIO；人物状态/伏笔表单尚未 WDIO。

- P13.4 Director 4 + 记忆表单 WDIO 通过；人物状态/伏笔表单 WDIO 未做。

- 记忆时间线/伏笔表单已 WDIO；人物状态表单因缺少 Native 创建人物入口未做。

- 记忆时间线/伏笔/人物状态表单均已 WDIO；AI 抽取/冲突裁决未完成。
