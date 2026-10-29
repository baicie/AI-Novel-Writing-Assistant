# P13.6 Memory Conflict & Revision Reconciliation

## 状态

**PARTIAL**

已实现（本轮）：

- 正式 Chapter Revision 变化后，来源事实 superseded；
- Confirm 时 CAS + 章节版本/hash 校验；
- 候选 Confirm/Reject/Defer 版本 CAS；
- truthfulness 与 status 分离；
- 歧义人物不能确认；
- 章节修改后候选保持 proposed，不自动提升；
- 同一人物同一剧情位置出现不兼容状态时，第二次 Confirm 返回 Conflict，候选保持 proposed，需人工裁决。

未完成：

- 裁决 UI（Confirm New / Keep Existing / Mark Disputed / Supersede）

- 同一人物同时间多地点的自动冲突检测；
- 死亡后无解释行动检测；
- 时间线循环/先后关系检测；
- 伏笔重复回收检测；
- 双方言论冲突与叙事视角区分；
- Confirm New / Keep Existing / Mark Disputed / Supersede 的完整裁决 UI；
- 重抽取旧事实对比与历史裁决保留。

当前不能标记 VERIFIED。
