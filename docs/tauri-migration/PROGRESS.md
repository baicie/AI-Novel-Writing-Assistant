# 迁移进度

- 当前阶段：P0–P2/P4/P5 最小切片已验证，P3/P6 部分完成
- 当前分支：`migration/tauri-2-rust`
- 启动时间：2026-10-23T00:46:01+08:00
- 截止时间：2026-10-23T10:00:00+08:00
- 基线：`8a1beae6`
- 用户未跟踪文件：`crates/novel-http/tests/integration_runs.rs`，没有修改或删除。
- 最新本地提交：代码最新提交 `77bbfe41`；P7/P8 审计文档紧随
- 最终 app：`target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app`（19.46 MiB）
- 最终 smoke：隔离数据目录启动成功；WebView 调用 bootstrap Command；SQLite 4 个迁移、1 条 prompt seed；主进程 RSS 约 67–74 MB；release 路由 smoke 已验证设置页、新建小说页、小说列表、小说编辑页（含人物列表）、章节编辑器页面；`/tasks` 已验证迁移运行列表；`/settings/models`、`/worlds`、`/knowledge`、`/genres`、`/story-modes`、`/base-characters` 仅验证渲染不崩溃，未迁移 API 仍返回 501。
- 可复现脚本：`scripts/smoke-tauri-route.sh`、`scripts/smoke-tauri-save.sh`、`scripts/smoke-tauri-adapter.sh`（均使用隔离临时数据目录，不触碰用户数据库）。

## 已完成并验证

### P0 Tauri 壳

- `src-tauri` 启动路径已切换为纯 Rust：不再创建 Node sidecar，也不再启动进程内 Axum 作为桌面默认路径。
- Tauri 2 主窗口加载 React/Vite；生产使用 `client/dist`，开发使用 `127.0.0.1:5173`。
- 窗口启动时注入 `window.__AI_NOVEL_RUNTIME__`，桌面运行模式由 Tauri 检测。
- 应用数据目录使用 Tauri app data dir（测试可用 `BIZ_NOVEL_DATA_DIR` 隔离）。
- CSP 默认仅允许自身、Tauri IPC、asset/data/blob 资源；HTTP/WS 仅开发模式 CSP 放行 Vite。
- 最小 capabilities 已列出全部命令 allow 权限。
- 证据：`cargo check -p novel-desktop` 通过；`cargo tauri build --debug --no-bundle --target aarch64-apple-darwin` 通过；`scripts/audit-tauri-release.sh` 确认 `.app` 无 Node/Electron/sidecar 文件或链接库。

### P1 书稿 SQLite

- SQLx SQLite 打开时启用 WAL、`foreign_keys=ON`、5s busy timeout、4 连接池；migration 0004 补充生成后风格复审开关。
- 小说：创建、读取详情、列表、更新（事务）、删除命令；人物列表只读投影由 `list_characters` 提供。
- 卷：创建、列表、更新、删除；删除有章节的卷会拒绝。
- 章节：创建（可绑定卷）、读取、列表、正文保存、元数据更新、删除；正文保存和删除均做 `officialVersion` CAS。
- 修订：`chapter_revision` 追加写入，`list_chapter_revisions` 可读取。
- 手工编辑后 `protection=user_edited`；AI `promote_candidate` 在保护状态下返回 `Blocked`。
- 前端统一 `desktopTransport` + 核心 REST 兼容适配器，`NovelList`/创建页/章节编辑器核心读写改走 Tauri Commands；Adapter 的创建/保存/重开、小说更新和 TXT 导出全链路 smoke 已通过。
- 证据：`cargo test -p novel-storage-sqlite` 12 个测试通过，包含 >1MB 中文正文写读、关闭重开、版本冲突、保护正文、卷关联、章节删除、设定建书；`scripts/smoke-tauri-save.sh` 已验证 Renderer → Tauri `save_chapter` → SQLite 的真实保存/重开链路。

### P2 LLM 网关

- OpenAI-compatible `reqwest` 调用；SSE 增量解析；Tauri `Channel` 用于文本和 Run 事件。
- 单请求硬超时、最多 2 次有界重试、输出 token 硬上限（默认 4000，绝对上限 32000）。
- 429 / 5xx / 网络 / 取消 / 401 分类；流开始后失败不自动重试，避免重复计费。
- Worker 取现有 keychain 凭据；Tauri `generate_text` 支持实时 delta/done/error 事件和按 requestId 取消。
- 证据：`cargo test -p novel-llm` 11 个测试通过，含本地 HTTP SSE、503 后重试、预算拒绝；`novel-application` 的 Mock 流式事件/checkpoint 测试通过。

### P4 旧库导入

- 旧库只读打开（`query_only=ON`）；导入前对源库和目标库分别做 `VACUUM INTO` 一致性备份。
- 旧 ID 保留并写 `legacy_id_map`；`World`、`Character`、`VolumePlan`/`VolumeChapterPlan` 导入为世界、角色、卷及章节归属；旧 `Novel.worldId` 映射到新 `novel.world_id`；running/waiting 旧 Run 不自动重放，转 `paused + migration_review_required`。
- 校验小说/章节数量、章节关系、正文 SHA-256 与正文内容；未支持字段/表写入报告。
- 证据：`cargo test -p novel-storage-sqlite --test invariants` 通过，含 >1MB 旧正文、世界/角色、卷/章节归属、源库只读、备份文件、正文 hash、重复导入、失败回滚、旧 Run 不重放。

### P5 Durable Workflow

- Run 状态机、持久化事件、RunContract、命令幂等、CAS、Approval Gate、step checkpoint、取消与恢复已在 Rust application/storage/workflow 层存在。
- 单书单活跃 Run 由部分唯一索引保证。
- 证据：`cargo test -p novel-application` 158 个测试通过；`cargo test -p novel-application --test recovery` 6 个测试通过；`novel-domain` 13 个、`novel-workflow` 6 个测试通过；`/tasks` 路由 smoke 实际调用 `list_all_runs`，预置 queued Run 时返回 count=1；带 `kind/id` 查询时详情投影也加载；`/knowledge` 实际调用 `list_knowledge_documents`，预置文档返回 count=1。

## 最近失败/未决

- `cargo test --workspace --all-targets` 仍会被用户未跟踪的 `crates/novel-http/tests/integration_runs.rs` 阻塞；该文件引用不存在的测试辅助方法，不是本次改动。
- P3 核心书稿路径已在 release `.app` 中验证：小说列表路由实际调用 `list_novels`，章节编辑器路由实际调用 `get_novel`、`get_chapter` 和原生 `get_chapter_editor_workspace`；任务中心写操作、自动导演、知识库、图像等仍是未迁移 API，适配器会返回明确 501 而不是静默假成功。
- Tauri 真实 GUI 交互尚未由用户验收；只做了编译、构建与启动/握手验证。
- Tauri `bundle_dmg.sh` 失败；手动 `hdiutil` DMG 已生成并校验（8.8 MiB），但未签名/未公证。
- 自动导演、任务中心写操作、知识库管理、图像/短剧等旧页面仍未迁移。

## 下一个最小任务

- 继续完成自动导演/章节工作区附属 API 的 Command 映射；补齐续写来源字段。
- 运行最终 fmt/check/clippy、前端 build、Tauri release bundle 与人工 GUI 验收。

## 发布说明

本次提交是桌面内核迁移切片，不是可直接发布的用户版本；按仓库规则未写入用户可见 release notes。待 UI 验收、签名和 DMG 完成后再合并到 `beta` 并补发布说明。

## P8 当前进度

- P8.1–P8.3 结构化最小后端切片：IMPLEMENTED；P8.4 UI：NOT_STARTED；P8.5 异常覆盖：PARTIAL。
- `cargo test --workspace --exclude novel-http`：217 tests PASS。
- P8 E2E：`p8_director_loop` 使用 ScriptedLlm 真实结构样本，走 Rust Runtime/SQLite/Gate/Commit；不是真实 Provider。
- 详细合同：`P8_BUSINESS_CONTRACTS.md`；验收：`P8_ACCEPTANCE.md`。
