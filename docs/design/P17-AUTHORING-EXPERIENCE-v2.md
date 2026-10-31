# P17 作者工作台体验设计 v2

> 修订原因：v1 把「章节编辑器」当成待重建的空白页，与仓库现状不符。本版先对齐已有能力，再谈增量。

## 1. 能力矩阵（基于真实代码，HEAD `3e8387ed` 之前）

| 能力 | 状态 | 证据位置 |
|---|---|---|
| Plate 富文本编辑器 | **已实现** | `ChapterTextEditor.tsx`（456 行） |
| 三栏工作台骨架 | **已实现** | `ChapterEditorShell.tsx`（781 行） |
| 选区 AI 改写 | **已实现** | `SelectionAIFloatingToolbar.tsx` + `review/selectionState.ts` |
| 光标处续写 | **已实现** | `CursorAIFloatingToolbar.tsx` |
| AI 候选生成与 Diff | **已实现** | `AIDiffPanel.tsx`、`useCandidateReview.ts`、`useCandidateAcceptance.ts` |
| 段落/行内 Diff 与选择性应用 | **已实现** | `selectionState.ts` + `selectionState.test.mjs` |
| 质量诊断与段落定位 | **已实现** | `ChapterEditorSidebar` 的诊断卡 + `handleFocusDiagnostic` |
| 本地草稿恢复 | **已实现** | `document/useChapterDraft.ts` + `draftState.test.mjs` |
| 正式版本冲突保护 | **已实现** | `officialVersion` CAS + `hasExternalChange` |
| 章节版本历史抽屉 | **已实现** | `ChapterVersionDrawer.tsx` |
| **整本小说的卷/章导航** | **本轮新增** | `navigation/ChapterNavigator.tsx`、`chapterNavigatorModel.ts` |
| 右侧统一工作区页签 | **缺失** | 目前只有 `ChapterEditorDirectorPanel` 单一面板 |
| 从编辑器查看当前 Run 与恢复入口 | **部分实现** | 面板内有生成入口，缺「当前 Run 状态 + 合法操作」聚合 |
| 人物与世界观只读关联 | **缺失** | 需要只读投影 |

## 2. 目标重述

不再是「重建章节编辑器」，而是：

**让已有章节编辑器成为整本小说的统一作者工作台** —— 在同一个页面里完成「定位章节 → 编辑正文 → AI 改写并预览 → 确认落库 → 处理运行状态」。

## 3. 三个增量方向

1. **左：整本小说导航**（本轮已交付切片）
   卷/章分组、当前章高亮、搜索、上一章/下一章、字数与版本状态；切换正文前必须确认未保存修改。

2. **右：统一工作区**（下一阶段）
   把 `ChapterEditorDirectorPanel` 提升为可切换页签容器：AI 修改 / 审校问题 / 人物与世界观 / 运行与恢复。每个页签只读自己已有的数据源，不做跨源拼装。

3. **中：编辑器增强**（下一阶段）
   长文排版、版本冲突可视化、快捷键、AI 处理中保护未保存内容。**不引入第二套编辑器引擎**。

## 4. 明确不做

- 不重写 Plate 编辑器；
- 不新建 LLM 调用通道（复用 `/chapter-editor/*` 与 Run 命令）；
- 不在前端伪造 Run/进度状态；
- 不为通过测试添加测试专用入口。
