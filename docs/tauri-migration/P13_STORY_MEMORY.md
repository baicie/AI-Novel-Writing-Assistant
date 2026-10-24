# P13.2 Long-Term Story Memory

## 状态：IMPLEMENTED / PARTIAL

已实现最小长期记忆基础：

- 复用 character_state / timeline_event，新增 plot_thread。
- 事实字段：source_revision、status、updated_at。
- Context 新增 `story_facts` 来源。
- 人物状态按目标章节之前的最新有效状态选择。
- 时间线事件与未回收伏笔按目标章节过滤。
- 30 章合成测试：目标第 20 章使用“掌门”状态，不使用早期“外门弟子”、不使用未来“死亡”和未来事件；目标第 30 章可看到死亡与大战事实。

已实现写入 API（Tauri + Storage）：

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


## 失效测试

- upsert 第 18 章人物状态 → 目标第 20 章可见；
- 保存第 18 章新正式修订 → 该状态标记 superseded，目标第 20 章不再读取；
- 历史事实行保留，不删除。


## Native 记忆展示

Director 页面新增“长篇记忆”区域：

- 读取 `list_story_facts(novelId, beforeChapterOrder)`；
- 展示人物状态、时间线事件、伏笔；
- 显示章节、状态和来源版本；
- 不写入正式章节。

本轮新增作者时间线记录表单：

- 选择当前目标章之前的最近正式章节作为来源；
- 作者填写时间线事实；
- 调用 `upsert_timeline_event`；
- 刷新长篇记忆列表；
- 来源带 `ui@timestamp` 修订标记。

仍缺人物状态表单与伏笔编辑表单。


## 记忆编辑 Native UI（第 5 轮）

导演页长篇记忆区域新增：

- 时间线记录表单；
- 伏笔记录表单（标题/未解决线索）；
- 人物状态表单（人物选择/状态正文）；
- 均绑定前一正式章节与 `ui@timestamp` 来源修订；
- 保存后刷新长篇记忆列表；
- 不修改正式正文。

仍缺 AI 抽取确认与记忆冲突人工裁决 UI。
