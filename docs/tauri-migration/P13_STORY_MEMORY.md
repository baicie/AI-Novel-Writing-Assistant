# P13.2 Long-Term Story Memory

## 状态：IMPLEMENTED / PARTIAL

已实现最小长期记忆基础：

- 复用 character_state / timeline_event，新增 plot_thread。
- 事实字段：source_revision、status、updated_at。
- Context 新增 `story_facts` 来源。
- 人物状态按目标章节之前的最新有效状态选择。
- 时间线事件与未回收伏笔按目标章节过滤。
- 30 章合成测试：目标第 20 章使用“掌门”状态，不使用早期“外门弟子”、不使用未来“死亡”和未来事件；目标第 30 章可看到死亡与大战事实。

已实现写入 API：

- `upsert_character_state`
- `upsert_timeline_event`
- `upsert_plot_thread`
- `list_story_facts`
- 作者修改正式章节时，旧 source_chapter/Revision 派生事实标记 superseded，Context 不再读取。

仍未完成：

- 记忆管理 Native UI 与 AI 抽取确认链路。
- proposed / disputed / superseded 状态流转。
- 已实现章节修改后的派生事实 superseded；仍缺重新抽取/人工核对 UI。
- 地点迁移、能力限制、关系变更的完整领域字段。
- 别名/同名人物解析。

## 现有基础

- `world_fact` / character 表
- 正式 Chapter Revision
- Context `character_facts` / `world_facts`
- `chapter_arc_summary`

## 需要新增

- Character State：身份、别名、关系、地点、状态、能力、生死、来源章节。
- Timeline / Event：事件身份、参与人、时序、地点、结果、来源。
- Plot Thread：伏笔首次出现、推进状态、回收证据。
- confirmed / proposed / disputed / superseded 事实状态。
- 按目标章节顺序的有效时间查询。
- 30 章合成事实库与作者修改早期章节失效测试。

未实施前不得宣称长篇剧情记忆完成。
