# 阻塞清单

## B-001 工作树中的未跟踪测试文件

- 现象：`crates/novel-http/tests/integration_runs.rs` 引用了 `TestContext` 不存在的方法，导致 `cargo test --workspace --all-targets` 编译失败。
- 分类：用户未提交工作与当前测试辅助代码不匹配；该测试的 HTTP 契约也与 Rust 新 Run API 不一致。
- 处理：没有修改、删除或暂存该文件。全量测试使用分 crate 方式验证；最终报告会单独列明。
- 下一动作：用户决定是否保留该实验测试；确认后再补齐/重写测试辅助方法。

## B-002 真实模型调用未验证

- 现象：默认使用 Mock/本地 HTTP SSE 测试；没有可用的付费凭据，也不允许自行发起真实付费请求。
- 影响：Provider 真实 endpoint 的模型差异、usage 字段、限流行为仍需用户环境验收。
- 下一动作：在用户批准的测试 Key 和预算下做一次短请求，记录用量与错误分类。

## B-003 macOS GUI 交互未做自动化验收

- 现象：已启动 Release `.app`，确认进程运行、SQLite 初始化；没有做屏幕/输入法/粘贴/撤销自动化测试。
- 影响：WKWebView 的中文输入法、Plate 长文滚动和选择行为需要人工检查。
- 下一动作：按 P3 清单做一次新建书→新建章→保存→重启读回的人工验收。

## B-004 大量旧 UI API 尚未 Tauri 化

- 现象：自动导演、任务中心、知识库、图像、短剧等仍调用 `/api/*`。
- 处理：集中适配器对未知路径返回 `NOT_MIGRATED` 501，不返回假成功。
- 影响：这些页面在纯 Tauri 发布模式下不可用；当前切片只保证书稿内核与基础模型/RAG 路径。
- 下一动作：按 `COMPATIBILITY.md` 逐模块迁移；不得以 Node sidecar 补齐。

## B-005 macOS DMG 打包

- 现象：`cargo tauri build --target aarch64-apple-darwin --bundles app,dmg` 的 `.app` 阶段成功，`bundle_dmg.sh` 失败；DMG 文件未产出。
- 分类：本机构建环境的 Tauri DMG bundler/磁盘镜像脚本问题，不是 Rust 编译错误。
- 处理：保留成功的 `.app`；未尝试签名、公证或上传。
- 下一动作：在交互式 macOS 构建机或 CI 上重跑 DMG，检查 `hdiutil`/AppleScript/Finder 权限；完成后再做签名与公证。
