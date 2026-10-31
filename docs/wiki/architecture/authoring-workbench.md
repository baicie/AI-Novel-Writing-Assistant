# 章节作者工作台（P17）

## Background

章节编辑器原本只解决「写这一章」：Plate 编辑器 + 选区 AI + 候选 Diff + 诊断 + 本地草稿。整本小说的导航、运行状态与恢复入口散落在其他页面，作者要来回跳。

P17 的目标不是重写编辑器，而是让它成为整本小说的统一作者工作台。

## Decision

1. 编辑器、选区 AI、光标续写、候选 Diff、诊断、本地草稿、版本 CAS **全部复用**，不引入第二套编辑器引擎。
2. 左侧新增「整本小说章节导航」，只消费已加载的 `novel detail.chapters`，不新增查询。
3. 右侧把原有 `ChapterEditorDirectorPanel` 提升为四页签工作区：AI 修改 / 审校问题 / 人物与世界观 / 运行与恢复。
4. 「运行与恢复」的动作**严格取自后端 `legalActions`**，未识别的动作直接忽略；前端不得臆造入口，也不得直接改数据库跳状态。

## Current Rule

- 章节切换统一走 `shouldBlockChapterSwitch(isDirty, confirm)`：有未保存修改必须先得到用户明确同意；按钮与 `Alt+←/→` 共用同一套判定。
- 章节导航上的「待处理 / 生成中」标记来自 `buildRunAttention(listRuns(novelId))`：按章节取最近一次 Run，只有 `waiting_approval / paused / failed` 才标为待处理。
- 人物与世界观面板只读现有数据，不生成新的记忆事实。
- 「AI 修改」页签内的组件与状态保持原样，改动只能通过 Shell 传入的 props。

## Examples

- 有未保存修改时点「下一章」→ 弹确认；拒绝则停留，接受才导航；未保存内容仍可从本地草稿恢复。
- 某章 Run 处于 `paused` → 导航里该章显示「待处理」；进入「运行与恢复」只显示后端允许的动作。

## Failure Modes

- 在导航里自行拼接 Run 状态 → 用户看到不存在的进度，属于禁止行为。
- 把新的业务逻辑继续塞进 781 行的 `ChapterEditorShell` → 应下沉到 `navigation/`、`rightPanel/` 目录下的独立模块。
- 章节切换不检查未保存状态 → 静默丢内容，属于 P0。

## Related Modules

- `client/src/pages/novels/NovelChapterEdit.tsx`
- `client/src/pages/novels/components/chapterEditor/ChapterEditorShell.tsx`
- `.../chapterEditor/navigation/`（章节导航）
- `.../chapterEditor/rightPanel/`（统一工作区）
- `.../chapterEditor/document/useChapterDraft.ts`（本地草稿）

## Source Documents

- `docs/design/P17-AUTHORING-EXPERIENCE-v2.md`
- `docs/design/P17-ARCHITECTURE-AND-INTERACTIONS.md`
- `docs/reviews/P17.0-IMPLEMENTATION-REVIEW.md`
