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

- 现象：自动导演、任务中心写操作、知识库管理写操作、图像、短剧等仍调用 `/api/*`；任务中心运行只读投影和知识库文档列表已接 Tauri。
- 处理：集中适配器对未知路径返回 `NOT_MIGRATED` 501，不返回假成功。
- 影响：这些页面在纯 Tauri 发布模式下不可用；当前切片只保证书稿内核与基础模型/RAG 路径。
- 下一动作：按 `COMPATIBILITY.md` 逐模块迁移；不得以 Node sidecar 补齐。

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
