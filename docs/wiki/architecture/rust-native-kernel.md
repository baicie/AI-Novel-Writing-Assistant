# 本地 Rust 创作内核

## 背景

现有桌面应用通过 Electron 携带 Node 运行时。创作事实、恢复和章节保护如果继续只活在 Node 进程里，离线编辑和崩溃恢复都会绑在那条运行时上。本地内核要把这些事实放进本机 SQLite，同时保留现有 React 写作界面。

旧库里的自动导演不变式需要保留：一本书同时只有一个活跃运行、合同一旦写入不再改写、控制态用版本比较更新、命令按编号幂等、质量问题记在章节上、作者改过的正文不能被候选覆盖、停止原因要能区分、事件序号在同一事务里分配。

## 决策

新内核与 Electron 并排，放在仓库根目录的 `crates/`、`src-tauri/`、`migrations/` 和 `migration-tools/`。不把旧 Prisma 库原地改成新表。导入工具只读打开旧库，写入一个新库，并用 `legacy_id_map` 记住旧编号。

领域层不感知 Tauri、SQLx、HTTP 和系统时间。桌面壳调用应用层，应用层通过端口读写；SQLite 实现这些端口。工作流判断保持纯函数。任务领取、模型调用和事务提交留在应用层，避免工作流模块和端口互相依赖。

模型调用按至少一次处理，不假装恰好一次。远端已经发出后如果进程中断，默认停下并把候选留下，不能自动再请求一次。作者明确要求重试时才重新调用，并受合同里的自动重试次数约束。用量未知时保持空值，不用 0 代替。

计划版本固定为 `novel-production-v1` 和 `chapter-chain-v1`。读到别的版本就停下等待查看，不能悄悄换成更新的计划。正文提示词必须精确匹配 `chapter_draft` / `chapter-draft-v1`。

## 当前规则

- 活跃状态是 `queued`、`running`、`waiting_approval`。旧状态 `waiting_gate` 对应 `waiting_approval`，`completed` 对应 `succeeded`。旧的 `running` 导入后变成 `paused`，并带上 `migration_review_required`。无法识别的状态不能变成成功。
- 领取只允许 `queued` 和 `running`，并且租约已经过期或仍是同一个工作者。`waiting_approval` 和 `paused` 不能被领取。
- 事件序号和状态变更在同一个事务里提交。`token_delta` 只在进程内广播，不写入事件表。
- 候选正文先写入 `chapter_candidate`。提升到正式章节时，章节保护必须仍是 `none`。导入的非空旧正文和作者保存都记为 `user_edited`。
- 恢复只看最新一次尝试。已经提交就按计划继续；作者接受候选就完成本步；作者要求重试才新开一次尝试；只是本地已开始还没发出远端请求时，可以沿用同一次尝试。远端已开始、候选已保存或已中断，都先停下。
- 进程在事务提交之后中断，和测试里于候选及调用记录提交后注入失败，留下的数据库状态相同。事务还没提交就被杀掉时，数据库会回到 `started`，恢复可以继续同一次尝试，不必再次请求模型。
- 质量债只追加记录，不改变运行状态，也不能单独挡住整本书后续章节。
- 模型地址默认只接受 HTTPS。本机 HTTP 必须单独允许。私网、链路本地和云元数据地址不能使用，地址里也不能带账号信息。
- 桌面命令标识是 `com.ai-novel.studio`，和现有 Electron 应用标识分开。默认不配置模型，离线也能打开本地资料。打包体积、签名和公证还不是这一阶段的完成条件。
- 前端类型目前手写在传输层，并与 Rust 事件 JSON 测试对齐。形状一旦继续分叉，应改为生成绑定，而不是在页面里各写一份。
- 内核不是 Tauri 安装包的产品主链路。自动导演、章节生产等全部产品能力由安装包内置的 Node 服务提供，见 [Tauri 桌面壳与 Node 本地服务 sidecar](./tauri-node-sidecar.md)。迁移要按模块替换对应的 HTTP 接口；迁移完成前，不能把 Rust 命令当作产品主入口，也不能把内核的阶段性完成写成后端已经迁移到 Rust。
- 桌面壳目前给内核接的是 `UnconfiguredProvider`，本地生成还没有接入真实模型。

## 示例

- 打开运行、生成草稿、在远端请求之后中断：下一次推进只把运行标成暂停，模型调用次数保持不变。作者改过正文后再接受候选，正式章节仍是作者正文。
- 人物设定这一步需要确认：运行停在 `waiting_approval` 后，后台循环不会再把它领走，也不会自动请求模型。
- 同一命令编号再次打开运行，返回原来的运行；换一个编号再开一本已有活跃运行的书，会被拒绝。

## 失败模式

- 把旧库路径同时当作新库路径会拒绝导入。导入报告里的未知表和未知列必须保留。
- 命令编号相同但请求内容不同，按冲突处理，不能覆盖上一次结果。
- 章节版本和打开时看到的版本不一致，保存失败，已有正文保持不变。
- 检索诊断只保存查询摘要和片段编号，不保存章节正文。
- 安装包大小、内存、冷启动、公证和召回 Hit@K 都还没有测量。不能把设计目标写成已经达到。

## 相关模块

- `crates/novel-domain`
- `crates/novel-workflow`
- `crates/novel-application`
- `crates/novel-storage-sqlite`
- `crates/novel-llm`
- `crates/novel-context`
- `crates/novel-prompt`
- `crates/novel-platform`
- `migration-tools`
- `src-tauri`
- `client/src/platform/desktopTransport.ts`
- `server/src/modules/director`

## 来源文档

- 本地内核设计：`../../design/2026-10-12-rust-native-kernel.md`
- 现有事实层边界：`./director-next-fact-layer.md`
- 桌面壳与 Node 服务的运行合同：`./tauri-node-sidecar.md`
