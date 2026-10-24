# P13 Acceptance

基线：`5dbe47e3`。

| 阶段 | 状态 | 说明 |
| --- | --- | --- |
| P13.0 代码审计 | VERIFIED（审计） | 修复索引重建/失效/删除/预算前置 |
| P13.1 RAG 索引生命周期 | IMPLEMENTED / PARTIAL | 来源版本、章节自动同步、重试、健康面板；无持久化后台重试队列 |
| P13.2 长篇记忆 | IMPLEMENTED / PARTIAL | 三类事实、Native 表单、修改失效；无身份/关系/地点独立结构 |
| P13.3 对抗评测 | PARTIAL | 8 case 确定性指标 + 50 case 合成检索；无真实语义标注 |
| P13.4 Native/性能 | PARTIAL | Director/Editor/SIGKILL、知识库 CRUD、1000 章性能；双窗口未验收 |
| P13.5 记忆抽取 | IMPLEMENTED / PARTIAL | 抽取→候选→人工确认→正式事实→Context；Native WDIO 通过；事实类型仅 3 类 |
| P13.6 冲突裁决 | PARTIAL | 来源失效/CAS + 同章人物冲突阻断；无完整冲突检测与裁决 UI |
| P13.7 知识库 Native | IMPLEMENTED / PARTIAL | Native 列表/筛选/新增/编辑/删除 + WDIO；无持久化索引任务/归档 |
| P13.8 大对抗集 | PARTIAL | p13-large-v1 50 章/50 问题；仅 Lexical/FTS，无 Vector/Hybrid 对照 |
| P13.9 真实性能 | PARTIAL | 100/500/1000 章 1.4k 字正文 + 8×10 并发检索；无内存峰值/冷启动/真实双窗口 |

不得将 P13.5–P13.9 全部标记 VERIFIED。

## 关键测试

```text
p13_memory_extraction                 4 passed
p13_large_benchmark                   1 passed
p13_scale_baseline                    1 passed
p13_story_memory                      通过
cargo test --workspace --exclude novel-http  264 passed, 0 failed（1345.54s）
clean worktree all-targets --no-run              PASS
current worktree all-targets                     BLOCKED by preserved untracked integration_runs.rs (不修改)
p13_adversarial_retrieval             1 passed
knowledge_lifecycle                   3 passed
memory-extraction WDIO                1 passing
knowledge CRUD WDIO                   1 passing
memory forms WDIO                     1 passing
Director WDIO                         4 passing
```

## 未完成

- P13.5：身份/别名、关系、地点、能力物品的独立事实结构；真实模型抽取质量。
- P13.6：地点冲突、死亡后行动、时间线循环、伏笔重复回收、裁决 UI。
- P13.7：持久化索引任务、重启恢复、归档、最后成功时间/失败原因的完整管理页。
- P13.8：Vector/Hybrid/RRF/Context Policy 同快照对照；人工语义标注；生成质量合同。
- P13.9：内存峰值、冷启动/预热、真实双窗口、系统输入法、真实 Provider。

## Native 构建

- `.app` 21.04 MiB
- DMG 9.4 MiB
- Commands / Handlers / Capabilities：82 / 82 / 82
- Production audit：PASS
