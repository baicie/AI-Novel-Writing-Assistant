# Tauri 原生桌面内核边界

## Background

桌面端原先由 Electron 启动 Node/Express，再由 Prisma 访问 SQLite。迁移目标不是把 Node 进程塞进安装包，而是让 Tauri 进程直接承载 Rust 应用内核，发布包不依赖 Node Runtime。

## Decision

- 桌面发布路径只保留 Tauri 2 进程；默认不启动 Node sidecar，也不以本地 Axum/Express HTTP 作为前端主通信路径。
- React/Plate 页面继续使用原有页面和文档结构。所有 IPC 收口到 `client/src/platform/desktopTransport.ts`，核心旧 REST 形态由 `desktopApiAdapter.ts` 转成 Tauri Commands。
- Rust 依赖方向为 `src-tauri → novel-application → novel-domain`；`novel-storage-sqlite`、`novel-llm`、`novel-context` 实现端口。Domain 不引用 Tauri、SQLx 或系统时间。
- SQLite 是唯一本地事实源。数据库路径由 Tauri app data dir 管理；测试通过 `BIZ_NOVEL_DATA_DIR` 隔离。连接启用 WAL、外键和 busy timeout。
- 流式 AI 使用 Tauri `Channel`；Run 事件持久化到 SQLite，断线后按 seq 回放。模型调用必须有超时、取消、有限重试和输出预算。

## Current Rule

- 新业务能力先扩展 Rust Command / application use case，再在 TypeScript Transport 暴露；不要在页面组件中散落 `invoke()`。
- 旧页面暂时保留 REST 形态时，只能经集中适配器转换；未迁移接口必须返回明确的 `NOT_MIGRATED` 错误，不能返回空成功。
- 章节正文保存以 `officialVersion` 做 CAS。人工编辑标记 `user_edited` 后，AI promotion 必须 `Blocked`；不能自动覆盖作者正文。
- 删除有卷/章节的小说必须拒绝；旧库导入前必须完成一致性备份，旧库只读，不自动重放旧运行。

## Failure Modes

- Command 权限遗漏：构建成功但 WebView invoke 被 ACL 拒绝；必须保持 `build.rs` 命令列表、`generate_handler!` 与 capabilities allow 列表同步。
- 前后端 Body 差异：Axios 自定义 adapter 拿到的 `config.data` 已经是 JSON 字符串，必须先解析再映射；否则创建/保存会收到空字段。
- 模型凭据：桌面目前只有一个 OpenAI-compatible 凭据槽，Worker 的 provider 标签可能不同；凭据读取需要回退到统一 secret ref。
- 长文正文：不能只看 `cargo check`；必须做关闭数据库后重开的真实读写测试。
- DMG 打包：`bundle_dmg.sh` 依赖本机 Finder/AppleScript 环境；`.app` 编译通过不等同于 DMG 可发布。

## Related Modules

- `crates/novel-application/src/ports.rs`
- `crates/novel-storage-sqlite/src/novels.rs`
- `crates/novel-storage-sqlite/src/legacy_import.rs`
- `crates/novel-llm/src/provider.rs`
- `src-tauri/src/commands/`
- `client/src/platform/desktopTransport.ts`
- `client/src/platform/desktopApiAdapter.ts`
