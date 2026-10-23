# P8 自动导演业务兼容矩阵

基线：`1df6acb1`（P7 修复提交）。P8 只迁移最小纵向闭环，不迁移图像、短剧、移动端和云端协作。

| 原版能力 | Rust/Tauri 当前状态 | 证据 / 边界 |
| --- | --- | --- |
| 创建小说 | VERIFIED | `NovelCreate` + Tauri `create_novel_with_setup`；Adapter smoke |
| 初始化导演 Run | VERIFIED | `open_run`，单书单活跃 Run，RunContract 冻结 |
| 故事宏观规划 | IMPLEMENTED（占位 Step） | Worker `story_macro` 产出 artifact；不是原版 Prompt 语义 |
| 人物阵容 | IMPLEMENTED（Gate 占位） | `character_cast` draft artifact + approval；未生成真实人物卡 |
| 卷策略 | IMPLEMENTED（Gate 占位） | `volume_strategy` draft artifact + approval；未生成真实卷规划 |
| 章节计划 | IMPLEMENTED（占位 Step） | `chapter_list` artifact；未接原版章节规划 Prompt |
| 选择章节 | PARTIAL | `target_chapter_id` 驱动 chapter scope；UI 选择器未迁移 |
| 上下文组装 | PARTIAL | context artifact 占位；RAG/context 服务存在但未接入此 Step |
| Mock LLM 章节草稿 | VERIFIED | `MockLlm` 真实 Runtime/Worker/SQLite 路径；非真实 Provider |
| 一致性审校 | IMPLEMENTED（占位） | `consistency_review` 固定 `issues:0`；不是真实审校 |
| 局部修复/质量债 | PARTIAL | `quality_debt` 数据层和 `record_debt` 测试；P8 纵向未接修复循环 |
| 人工确认 Gate | VERIFIED（后端测试） | P8 test 中多次 `approve`；UI Gate 未验收 |
| 正式正文提交 | VERIFIED | `commit` Step promote candidate；P8 test 重开正文一致 |
| 重启后恢复 | VERIFIED（应用层） | P8 test 重开 SQLite 读取正文；真实 GUI 重开 UI 未验 |
| Prompt 版本语义 | PARTIAL | 仅 `chapter_draft` 走 Prompt Registry `chapter-draft-v1`；其他 Step 尚未接原版 Prompt |
| Durable Workflow | VERIFIED（最小链） | 状态机/事件/CAS/Gate/checkpoint/取消/恢复 |
| Memory / 事件补拉 | VERIFIED（应用层） | replay/seq；broadcast Lag 有持久化补拉 |
| Tauri 自动导演 UI 全链路 | NOT_STARTED | 旧 AutoDirector UI 仍返回 `NOT_MIGRATED`；本轮不迁移大型 UI |
| 图像/短剧/漫画/移动端 | NOT_STARTED | 明确不在 P8 范围 |
