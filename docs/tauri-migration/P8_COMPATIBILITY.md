# P8 自动导演业务兼容矩阵

基线：`59d472a6`；结构化实现 `6959dbfd`；P8.4 MVP 在 `5ed98ec0` 之后。P8 只迁移最小纵向闭环，不迁移图像、短剧、移动端和云端协作。

| 原版能力 | Rust/Tauri 当前状态 | 证据 / 边界 |
| --- | --- | --- |
| 创建小说 | VERIFIED | `NovelCreate` + Tauri `create_novel_with_setup`；Adapter smoke |
| 初始化导演 Run | VERIFIED | `open_run`，单书单活跃 Run，RunContract 冻结 |
| 故事宏观规划 | IMPLEMENTED（结构化最小合同） | 真实 LLM 接口、`story_macro` artifact、expansion/decomposition/issues Schema 校验；旧 Prompt 逐字语义仍 PARTIAL |
| 人物阵容 | IMPLEMENTED / PARTIAL | 至少 3 人/2 关系、Gate 确认；未接拒绝/重新生成/人物事实表 |
| 卷策略 | IMPLEMENTED / PARTIAL | volumes Schema、唯一 volumeOrder、Gate；未写旧版卷关系表 |
| 章节计划 | IMPLEMENTED / PARTIAL | chapters Schema、唯一 chapterOrder、卷/人物引用校验；未接重规划与人工编辑保护 |
| 选择章节 | IMPLEMENTED（Tauri MVP） | TauriDirectorPage 使用已有章节选择；多章节范围不支持 |
| 上下文组装 | PARTIAL | 已组装上游 artifacts 与 tokenBudget；世界/角色状态/历史/知识库/完整裁剪未接入 |
| Mock LLM 章节草稿 | VERIFIED | `MockLlm` 真实 Runtime/Worker/SQLite 路径；非真实 Provider |
| 一致性审校 | IMPLEMENTED（最小合同） | 结构化 issues、空 clean、失败/非法 JSON pause；旧版完整审校与问题样本 PARTIAL |
| 局部修复/质量债 | FAIL_CLOSED / NOT_IMPLEMENTED | 审校有问题时暂停，不生成假修复/不提交；真实修复循环未接 |
| 人工确认 Gate | VERIFIED（后端）/ PARTIAL（UI） | P8 test 多次 approve + stale expectedVersion 拒绝；Tauri MVP 有确认按钮并发送 expectedVersion；真实 UI 点击验收未做；拒绝/重新生成未实现 |
| 正式正文提交 | VERIFIED（后端）/ PARTIAL（UI） | commit promote candidate；P8 test 重开一致；Tauri MVP 完成后提供正文入口，真实 GUI 未验收 |
| 重启后恢复 | VERIFIED（应用层）/ BLOCKED（GUI） | P8 test 重开 SQLite 读取正文；真实 GUI 重开 UI 未验 |
| Prompt 版本语义 | PARTIAL | 已新增 5 个 Director PromptAsset；是旧版 schema 的可验证子集，不是逐字复刻 |
| Durable Workflow | VERIFIED（最小链） | 状态机/事件/CAS/Gate/checkpoint/取消/恢复 |
| Memory / 事件补拉 | VERIFIED（应用层） | replay/seq；broadcast Lag 有持久化补拉 |
| Tauri 自动导演 UI 全链路 | PARTIAL | 新 TauriDirectorPage/History/transport 已接真实命令；Release 路由/启动表单 smoke 通过；完整 UI 审批/取消/恢复与旧 Workspace 未完成 |
| 图像/短剧/漫画/移动端 | NOT_STARTED | 明确不在 P8 范围 |
