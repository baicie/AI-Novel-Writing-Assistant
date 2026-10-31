# P17 架构与交互

## 1. 数据来源（全部只读消费现有 API）

| 区块 | 数据源 | 说明 |
|---|---|---|
| 章节树 | `getNovelDetail(novelId).chapters`（+ `volumes`，若存在） | 已加载的同一份数据，不新增请求 |
| 当前章正文 | `getChapterEditorWorkspace` | 已存在 |
| 保存状态 | `useChapterDraft`（local draft）+ `officialVersion` CAS | 已存在 |
| 诊断 | `workspace.chapterMeta / diagnostics` | 已存在 |
| Run 状态 | `getRunDetail / listRuns`（已有 IPC） | P17.1 接入 |

## 2. 组件边界

```
NovelChapterEdit（路由页，负责数据查询与导航副作用）
└── ChapterEditorShell（编排，781 行；本轮只加 props 透传，不重构）
    ├── ChapterEditorSidebar（信息 + 诊断）
    │   └── navigation/ChapterNavigator（本轮新增：卷/章树 + 搜索 + 上一/下一）
    ├── ChapterTextEditor（Plate，不动）
    └── ChapterEditorDirectorPanel（P17.1 改为页签容器）
```

新增纯逻辑集中在 `navigation/chapterNavigatorModel.ts`，可单测、无 React 依赖。

## 3. 交互规则

1. **切换章节**：若 `isDirty`，先 `confirm`（与 Shell 现有「离开编辑器」一致的口径），确认后才导航；未保存内容仍可从本地草稿恢复。
2. **当前章高亮**：`data-current` 属性 + 视觉态，供 E2E 断言。
3. **搜索**：标题或序号命中；空关键词返回全部。
4. **上一章/下一章**：按 `order` 排序后取相邻，边界禁用按钮。
5. **无卷信息**：归入「未分卷」，保证任何数据都能导航。

## 4. 风险与约束

- 章节切换会重建 Shell（`key={chapter.id}`），因此**不会**出现异步响应写错章节；
- 导航不触发新查询，只用已加载的 `detail.chapters`，避免切章时的请求风暴；
- 未保存保护复用既有 `isDirty`，不新造状态。

## 5. 后续（P17.1）

- 右侧页签容器 + Run 状态投影；
- 人物/世界观只读面板（复用现有查询，不新增记忆事实）；
- 小窗口布局与键盘导航；
- 新增 Native E2E：章节导航、未保存保护、AI 候选预览与应用、Run 恢复入口。
