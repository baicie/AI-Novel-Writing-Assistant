# P13.2 Long-Term Story Memory

## 状态：NOT_STARTED

本轮已完成 P13.0/P13.1 审计与索引生命周期；尚未实施 P13.2。

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
