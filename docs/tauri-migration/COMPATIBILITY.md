# Electron → Tauri 兼容矩阵

状态口径：VERIFIED / IMPLEMENTED / PARTIAL / BLOCKED / NOT_STARTED。

| 能力 | 旧实现 | Tauri/Rust 状态 | 证据 / 边界 |
| --- | --- | --- | --- |
| 桌面启动 | Electron main + Node server/sidecar | VERIFIED | Tauri 2 debug/release build；隔离数据目录启动后完成 SQLite 迁移并保持进程运行 |
| 基础 IPC | preload + Express HTTP | VERIFIED | `desktopTransport.ts` 使用 `@tauri-apps/api/core` `invoke`/`Channel`；核心 REST 只通过集中适配器进入 |
| 小说列表/创建/详情/更新/删除 | Prisma/Express | VERIFIED（核心路径） | Tauri Commands + SQLx；更新使用单事务；Adapter 创建/更新/章节保存 smoke 通过；前端 `NovelList`/创建页接入 |
| 卷 CRUD | Prisma/Express | VERIFIED（后端 + 前端 Transport） | 卷序冲突、含章节删除拒绝有测试；旧页面尚未全部改为卷 API |
| 章节 CRUD | Prisma/Express | VERIFIED（核心路径） | 创建/读取/更新/删除；CAS；编辑器保存与 Volume 绑定 |
| 章节修订 | Prisma/Express | VERIFIED（读取/追加） | `chapter_revision` 只追加；版本列表命令；恢复操作尚未做独立 UI |
| 长正文 | Prisma/Express | VERIFIED | >1MB 中文正文写入、关闭、重开读回测试 |
| 自动保存 / 章节切换 | 前端 + Tauri Command | VERIFIED（保存命令链路） | Renderer → Tauri `save_chapter` → SQLite 保存/重开已验证；Plate 自动保存交互仍待人工验收，工作区附属字段仍为 PARTIAL |
| 模型设置 / 密钥 | Express + Prisma | VERIFIED（命令） | OS keychain + `provider_credential`；前端本地 key manager 使用 Transport |
| 基础设置 | Express + Prisma | PARTIAL | 小说基础字段、生成后风格复审开关、模型设置已迁移；模型路由、任务策略等未迁移 |
| AI 流式生成 | Express SSE / LangGraph | VERIFIED（Mock + 网关） | reqwest SSE、Tauri Channel、超时、取消、有限重试、token 上限测试；未做真实付费调用 |
| Run 工作流 | TS LangGraph/导演 | VERIFIED（最小内核） | Run 状态机、事件、CAS、Gate、checkpoint、幂等、取消、恢复测试 |
| 旧数据导入 | Prisma/旧 SQLite | VERIFIED（工具） | 只读旧库、双库一致性备份、ID 映射、世界/角色/卷/章节归属、数量/关系/正文 hash 校验、未支持项报告 |
| FTS5 检索 | 服务型 RAG/Qdrant | VERIFIED（基础能力） | FTS5 中文 bigram/词项测试、向量余弦适配、RRF/预算组装测试 |
| 导入导出 | Express | PARTIAL | Rust JSON/Markdown/TXT 导出 Command，整书下载适配器已接；分章/其他导出范围尚未映射 |
| 图像/短剧/漫画/知识库 UI | Node/Prisma | NOT_STARTED | 明确返回 `NOT_MIGRATED`，不静默成功 |
| 自动导演全链路 | Node/LangGraph | PARTIAL（后端纵向） | Rust Runtime/SQLite/Mock 已走通最小纵向闭环；真实 Prompt 语义、旧 AutoDirector UI、真实 Provider 未迁移 |
| macOS arm64 Release Bundle | Electron Builder | VERIFIED（本机，未签名） | `.app` 19.35 MiB；手动 `hdiutil` DMG 8.8 MiB 校验通过；签名/公证未完成 |
| Windows x64 / Linux x64 | Electron | NOT_STARTED | 需目标平台原生依赖与打包验证 |
