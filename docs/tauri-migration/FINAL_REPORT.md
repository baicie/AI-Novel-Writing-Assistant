# Tauri 2 + Rust 迁移最终报告

报告时间：2026-10-23（UTC+08:00）
分支：`migration/tauri-2-rust`
基线：`8a1beae6`

## 1. 当前迁移完成状态

**总体：PARTIAL，不能宣称完全迁移或生产可用。**

本次交付完成了一个可运行、可测试的 Tauri 2 + Rust 书稿内核切片：桌面进程不再启动 Node sidecar 或本地 Axum 服务；小说/卷/章节、章节修订、模型网关、基础 RAG 和最小 durable workflow 已通过 Rust 测试，并产出了 macOS arm64 `.app`。React/Plate 仍保留，核心书稿 REST 调用由集中式 Tauri Transport 适配器转成 Commands。

未完成的部分是旧应用的大量业务模块（自动导演 UI 全链路、知识库管理、图像、短剧、模型路由、任务中心完整投影等）与真实模型/GUI 验收。

## 2. Electron 与 Tauri 功能兼容矩阵

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| 桌面启动壳 | VERIFIED | Tauri 2 release `.app` 启动；隔离数据目录初始化 SQLite；WebView 成功调用 bootstrap Command |
| Node/Express 依赖 | VERIFIED | Tauri release `.app` 内没有 Node/server/sidecar 资源；默认启动路径不再启动本地 HTTP server |
| 小说列表/创建/详情/更新/删除 | VERIFIED（核心路径） | Rust Commands + SQLx；删除有卷/章的书会拒绝，避免误删；人物列表由 `list_characters` 只读投影提供 |
| 卷 CRUD | VERIFIED（内核） | 卷序唯一约束、含章节删除拒绝、事务更新 |
| 章节 CRUD/正文保存 | VERIFIED（核心路径） | CAS 版本检查、用户正文保护、重启读回 |
| 章节修订 | VERIFIED（读取/追加） | 只追加 `chapter_revision`；尚无恢复旧版本的独立 UI |
| 自动保存/章节切换 | VERIFIED（保存命令链路） | Renderer → Tauri `save_chapter` → SQLite 保存/重开 smoke 通过；Plate 自动保存交互仍待人工验收，工作区附属字段仍为 PARTIAL |
| 模型设置/密钥 | VERIFIED（Command） | OS Keychain + SQLite provider credential |
| AI 流式生成 | VERIFIED（Mock/网关） | Tauri Channel、SSE、取消、超时、有限重试、token 上限；真实付费调用未执行 |
| Durable Workflow | VERIFIED（最小章节链） | 状态机/事件/CAS/命令幂等/Gate/checkpoint/恢复测试通过 |
| 旧数据导入 | VERIFIED（工具） | 双库一致性备份、只读旧库、ID 映射、关系/正文校验、未支持字段报告 |
| SQLite FTS5 + 向量适配 | VERIFIED（内核） | 中文词项 FTS5、余弦检索、RRF/上下文预算测试 |
| 导入导出 | PARTIAL | Rust JSON/Markdown/TXT 整书导出 Command 已接下载适配器；分章/其他范围未接入 |
| 自动导演/任务中心/知识库/图像/短剧 UI | PARTIAL | 任务中心的迁移运行列表由 `list_all_runs` 只读提供；自动导演、知识库、图像、短剧等未迁移 API 明确返回 501 `NOT_MIGRATED`，不静默假成功 |
| macOS arm64 `.app` | VERIFIED | `target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app`，19.29 MiB |
| macOS arm64 DMG | VERIFIED（未签名） | Tauri `bundle_dmg.sh` 在当前环境失败；使用 `hdiutil` 从同一 `.app` 生成并校验的 DMG 成功，8.8 MiB；未签名/未公证 |
| Windows x64 / Linux x64 | NOT_STARTED | 尚未做目标平台原生依赖和打包验证 |

## 3. Rust 后端已迁移模块

- `novel-domain`：小说/章节/卷/保护规则/Run 状态机，不依赖 Tauri、SQLx。
- `novel-application`：小说、章节、卷、修订、运行、任务概览用例；事务边界和端口。
- `novel-storage-sqlite`：SQLx SQLite 仓储、WAL/外键/busy timeout、嵌入迁移、旧库导入。
- `novel-llm`：OpenAI-compatible provider、SSE、错误分类、超时、有限重试、输出预算。
- `novel-workflow`：生产计划、Run 恢复决策、步骤/门语义。
- `novel-context`：中文词项、FTS query、余弦向量、RRF、上下文预算。
- `novel-platform`：OS keychain、日志、JSON/Markdown 导出。
- `src-tauri`：窗口、最小 capabilities、应用数据目录、Commands、Channels；Node sidecar 代码已从活动树移除。
- `migration-tools`：旧 Prisma SQLite 到新 SQLite 的单向导入 CLI。

## 4. 前端已接入的真实功能

- `client/src/platform/desktopTransport.ts`：唯一 Tauri invoke/Channel 边界，使用官方 `@tauri-apps/api/core`。
- `client/src/platform/desktopApiAdapter.ts`：对旧页面保留 REST 形态，但把核心小说/章节/卷/设置/任务读取请求转成 Tauri Commands；未实现路径返回 `NOT_MIGRATED` 501。
- 小说列表、创建小说基础字段、详情读取、章节保存、章节列表、模型密钥设置、任务概览/恢复候选读取已接入。
- 章节编辑器继续使用现有 Plate 文档和快捷保存；工作区投影由 `get_chapter_editor_workspace` Tauri Command 返回；保存使用 `officialVersion` CAS。小说创建在桌面模式跳过云端 workflow bootstrap。
- 未迁移的旧页面不会被假成功掩盖；错误会显式显示。

## 5. SQLite 和旧数据迁移状态

- 新库路径：Tauri app data dir 下的 `novel.sqlite`；已启用 WAL、`foreign_keys=ON`、5 秒 busy timeout。
- migrations 由 Rust 内嵌加载，事务逐条执行并记录 `schema_migration`。
- 长正文验证：>1MB 中文正文写入、版本冲突、关闭并重开读取、用户保护正文阻止 AI promotion。
- 旧库导入：源库只读；导入前分别生成源库/目标库 `VACUUM INTO` 备份；保留旧 ID 映射；校验小说/世界/角色/卷/章节数量、小说世界归属、章节归属、正文 SHA-256 和正文内容；未支持表/列写入报告。
- 旧 running Run 转为 `paused + migration_review_required`，不自动重放。

## 6. LLM / Workflow / RAG 状态

- LLM：Mock SSE、503 重试、预算拒绝、取消测试通过；真实 Provider 只实现 OpenAI-compatible 协议，Anthropic/Ollama 未实现。
- Workflow：最小章节生产链已可 create → tick → draft checkpoint → approval → commit；中断后保留候选、人工编辑保护和恢复测试通过。不是 LangGraph 逐行翻译。
- RAG：SQLite FTS5 + 本地向量 BLOB/余弦适配层，提供 `index_knowledge`、`search_knowledge`、`search_knowledge_vector` Commands。未接 Qdrant，也没有完整知识库管理 UI。

## 7. 已执行的测试与实际结果

| 命令 | 结果 |
| --- | --- |
| `cargo check --workspace` | PASS |
| `cargo fmt --check -p ...`（迁移 crate） | PASS |
| `cargo test --workspace --exclude novel-http` | PASS：204 tests |
| `cargo test -p novel-application` | PASS：152 unit + 6 recovery |
| `cargo test -p novel-storage-sqlite` | PASS：4 + 4 tests |
| `cargo test -p novel-llm` | PASS：8 + 3 tests |
| `cargo test -p novel-domain -p novel-workflow` | PASS：13 + 6 tests |
| `cargo test -p novel-http --test integration_novels/integration_chapters/endpoint_tests/integration` | PASS：12 + 9 + 25 + 9 tests（已跟踪测试目标） |
| `pnpm --filter @ai-novel/client typecheck` | PASS |
| `pnpm typecheck`（根级） | PASS；根脚本已设置 `NODE_OPTIONS=--max-old-space-size=8192` |
| `pnpm --filter @ai-novel/client build` | PASS（有既有大 chunk 警告） |
| `pnpm build`（根级） | PASS（`NODE_OPTIONS=--max-old-space-size=8192`，有既有大 chunk 警告） |
| `pnpm --filter @ai-novel/client test` | PASS：180 tests |
| `cargo clippy`（非 `-D warnings`） | PASS，存在既有警告 |
| `cargo tauri build --target aarch64-apple-darwin --bundles app` | PASS，19.29 MiB `.app` |
| `cargo tauri build ... --bundles app,dmg` | `.app` PASS；Tauri `bundle_dmg.sh` 失败 |
| `hdiutil create/verify` 手动 DMG | PASS：8.8 MiB，校验和 VALID，内含 `.app` |
| `scripts/audit-tauri-release.sh` | PASS：`.app` 无 Node/Electron/sidecar 文件，主二进制未链接 Node/Electron 库 |
| 隔离数据目录启动 release `.app` | PASS：进程运行、SQLite 4 个迁移、prompt seed、WebView bootstrap Command 日志；主进程 RSS 约 67–74 MB |
| Release 路由 smoke（`/novels`） | PASS：预置 1 本书后，`NovelList` 实际调用 `list_novels` 并返回 count=1 |
| Release 路由 smoke（章节编辑器） | PASS：预置章节后，编辑器路由实际调用 `get_novel`、`get_chapter` 和原生 `get_chapter_editor_workspace` |
| Renderer 保存 smoke | PASS：Render → Tauri Commands → SQLite 实际完成建书、建章、`save_chapter`（expectedVersion=0）、重开读取；返回 version=1、content=正文 |
| Adapter 创建/保存 smoke | PASS：`apiClient` Adapter 的 `POST /novels` → `POST /chapters` → `PUT /chapters/:id` → `GET`，并 `PUT /novels/:id` 更新简介/风格复审开关；version=1、content=正文 |
| Release 路由 smoke（小说编辑页） | PASS：`/novels/:id/edit` 实际调用 `get_novel`、`list_characters`，页面无未捕获异常 |
| Release 路由 smoke（新建小说页） | PASS：`/novels/create` 加载，实际调用 `list_novels`，无未捕获异常 |
| Release 路由 smoke（设置页） | PASS：`/settings` 加载，无未捕获异常；模型密钥命令由 Transport 提供 |
| Release 路由 smoke（运行记录） | PASS：预置 1 个 queued Run 后，`/tasks` 实际调用 `list_all_runs` 并返回 count=1；带 `kind/id` 查询时列表与详情投影均加载，无未捕获异常 |
| Release 路由巡检（扩展页面） | PASS（仅渲染/不崩溃）：`/tasks`、`/settings/models`、`/worlds`、`/knowledge`、`/genres`、`/story-modes`、`/base-characters` 加载无未捕获异常；未迁移 API 仍返回 501 |
| 迁移 CLI 隔离数据库 | PASS：备份、ID 映射、正文校验、unsupported 报告、旧 running → paused |
| `cargo test --workspace --all-targets` | BLOCKED：用户未跟踪测试文件 `crates/novel-http/tests/integration_runs.rs` 与当前 TestContext API 不匹配 |

已跟踪 Rust 测试合计 259 个通过；前端 180 个测试通过；真实模型调用、macOS 输入法/粘贴/撤销/长文滚动和完整 GUI 交互未执行，需用户验收。

## 8. 构建产物及路径

- macOS arm64 application bundle：`target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app`（19.29 MiB）
- Release executable：`target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app/Contents/MacOS/novel-desktop`
- DMG：`target/aarch64-apple-darwin/release/bundle/dmg/Biz Novel Studio-0.1.0-arm64-manual.dmg`（8.8 MiB，未签名）
- 迁移报告与文档：`docs/tauri-migration/`
- 路由冒烟脚本：`scripts/smoke-tauri-route.sh`（支持 `TAURI_ROUTE`/`TAURI_EXPECT_LOG`）
- 保存冒烟脚本：`scripts/smoke-tauri-save.sh`
- Adapter 冒烟脚本：`scripts/smoke-tauri-adapter.sh`
- Release bundle 审计脚本：`scripts/audit-tauri-release.sh`
- 隔离 smoke 数据目录在 `/tmp/biz-novel-tauri-*`，不是用户生产数据。

## 9. 剩余阻塞和已知风险

- 未跟踪测试文件阻塞全工作区 `--all-targets` 编译；没有修改或删除用户文件。
- Tauri DMG 脚本失败；手动 `hdiutil` DMG 已生成并校验，但 `.app`/DMG 未签名、未公证，不能作为公开分发包。
- 真实模型 endpoint、额度、429、usage 字段未验证；不做无限真实调用。
- 仅核心书稿页面接入 Tauri；自动导演/任务中心写操作/知识库/图像/短剧仍走未迁移路径。
- 未做 Windows/Linux 交叉打包；`better-sqlite3`/Node 旧路径仍存在于 Electron workspace，但不在 Tauri release 内。
- Tauri release 的 WKWebView 输入法、粘贴、撤销、长文本滚动和选择行为需人工验收。
- 续写来源字段等旧模型字段在桌面适配器中未完整持久化，已由 501 边界或兼容矩阵限制，不能宣称全量等价；`postGenerationStyleReviewEnabled` 已补入 SQLite migration 0004。

## 10. 后续继续执行的具体任务

1. 由用户确认并处理 `crates/novel-http/tests/integration_runs.rs`；恢复全 workspace `--all-targets`。
2. 在用户批准的测试凭据/预算下验证真实 OpenAI-compatible 流式调用与取消。
3. 完成剩余旧 UI API → Tauri Command 映射，优先自动导演运行投影、任务中心只读详情、章节工作区附属数据。
4. 补齐续写来源字段和其他旧模型字段迁移。
5. 修复/重做 DMG 打包、签名/公证；在 macOS arm64 上完成人工输入法与长文验收。
6. 再评估 Windows x64/Linux x64；不要以 Node sidecar 补缺口。
