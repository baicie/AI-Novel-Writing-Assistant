# 阶段 3.3 生成流程集成设计方案

## 📋 当前分析

### 生成流程的两个关键 Mutation

在 `ChapterEditorShell.tsx` 中：

1. **previewMutation** - AI 修正方案生成
   - 输入：修正请求（选定的文本范围 + 修正指令）
   - 输出：多个候选修正方案
   - 触发点：line 493 的 `previewMutation.mutate(request)`

2. **continuationMutation** - 续写方案生成
   - 输入：光标位置 + 续写操作
   - 输出：多个候选续写方案
   - 触发点：line 529 的 `continuationMutation.mutate(request)`

### 当前的 Cloud-Only 问题

这两个 mutation 的 `mutationFn` 都直接调用云 API：
- previewMutation 调用 `previewChapterAiRevision()`
- continuationMutation 调用 `previewChapterCursorContinuation()`

这两个都是云 API（在 `@/api/novel` 中）。

---

## 🎯 设计方案

### **策略：最小化改动的适配层**

不改动现有 mutation 的结构，而是在 **API 层创建适配层**，让 API 层负责判断和路由。

#### **方案设计**：

```typescript
// 在 @/api/novel.ts 中创建新的包装函数

export async function previewChapterAiRevisionWithModeRouting(
  novelId: string,
  chapterId: string,
  request: AiRevisionRequest
) {
  const { shouldUseLocalGeneration } = await import("@/api/localCreation");
  
  if (shouldUseLocalGeneration(novelId)) {
    // 【本地模式】
    return handleLocalAiRevision(novelId, chapterId, request);
  } else {
    // 【云模式】
    return previewChapterAiRevision(novelId, chapterId, request);
  }
}

export async function previewChapterCursorContinuationWithModeRouting(
  novelId: string,
  chapterId: string,
  request: CursorContinuationRequest
) {
  const { shouldUseLocalGeneration } = await import("@/api/localCreation");
  
  if (shouldUseLocalGeneration(novelId)) {
    // 【本地模式】
    return handleLocalCursorContinuation(novelId, chapterId, request);
  } else {
    // 【云模式】
    return previewChapterCursorContinuation(novelId, chapterId, request);
  }
}
```

#### **优点**：

1. ✅ **ChapterEditorShell 无需改动** - 调用还是同一个函数
2. ✅ **API 层集中管理** - 所有路由逻辑在一个地方
3. ✅ **云模式完全兼容** - 不影响既有逻辑
4. ✅ **易于测试** - 可单独测试每个分支

---

## 🔄 实现流程

### **Step 3.3a：创建本地 AI 修正处理函数**

**位置**：`client/src/api/localCreation.ts`

**功能**：
- 将 AI 修正请求转换为 Tauri 命令
- 启动本地生成
- 订阅事件并转换为候选方案格式

```typescript
/**
 * 本地模式 AI 修正生成
 */
export async function handleLocalAiRevision(
  novelId: string,
  chapterId: string,
  request: AiRevisionRequest
): Promise<ChapterEditorResponse> {
  // 1. 启动本地生成
  const outcome = await startLocalGeneration({
    commandId: generateId(),
    novelId,
    targetChapterId: chapterId,
    provider: "opencode-go", // 或从 request 中获取
    model: request.model || "gpt-4",
    promptVersions: {}, // 从 request 映射
    maxAutomaticRetries: 3,
    endpoint: (await getLocalModelSetup())?.endpoint || "",
    allowLocalhostHttp: false,
  });

  // 2. 订阅事件
  const candidates: ChapterEditorCandidate[] = [];
  let isCompleted = false;

  await new Promise<void>((resolve) => {
    subscribeLocalGeneration(outcome.runId, (event) => {
      if (event.kind === "checkpoint_saved") {
        // 保存候选方案
        candidates.push({
          id: event.candidateId,
          // ... 其他字段
        });
      } else if (event.kind === "run_completed") {
        isCompleted = true;
        resolve();
      }
    });
  });

  // 3. 返回格式化响应
  return {
    data: {
      sessionId: outcome.runId,
      candidates,
      activeCandidateId: candidates[0]?.id ?? null,
      // ... 其他字段
    },
  };
}

/**
 * 本地模式光标续写生成
 */
export async function handleLocalCursorContinuation(
  novelId: string,
  chapterId: string,
  request: CursorContinuationRequest
): Promise<ChapterEditorResponse> {
  // 类似实现...
}
```

### **Step 3.3b：创建 API 包装函数**

**位置**：`client/src/api/novel.ts` 或新文件 `novelChapterGeneration.ts`

```typescript
import { shouldUseLocalGeneration, handleLocalAiRevision, handleLocalCursorContinuation } from "@/api/localCreation";

/**
 * 章节 AI 修正 - 支持本地和云模式
 */
export async function previewChapterAiRevisionWithRouting(
  novelId: string,
  chapterId: string,
  request: AiRevisionRequest
) {
  if (shouldUseLocalGeneration(novelId)) {
    return handleLocalAiRevision(novelId, chapterId, request);
  }
  return previewChapterAiRevision(novelId, chapterId, request);
}

/**
 * 光标续写 - 支持本地和云模式
 */
export async function previewChapterCursorContinuationWithRouting(
  novelId: string,
  chapterId: string,
  request: CursorContinuationRequest
) {
  if (shouldUseLocalGeneration(novelId)) {
    return handleLocalCursorContinuation(novelId, chapterId, request);
  }
  return previewChapterCursorContinuation(novelId, chapterId, request);
}
```

### **Step 3.3c：在 ChapterEditorShell 中调用新的函数**

**位置**：`client/src/pages/novels/components/chapterEditor/ChapterEditorShell.tsx`

**改动点**：

```typescript
// 在 previewMutation 中改变 mutationFn

const previewMutation = useMutation({
  mutationFn: async (request: ReturnType<typeof buildAiRevisionRequest>) => {
    if (!chapter) {
      throw new Error("当前未选中章节。");
    }
    // 【改动】从 previewChapterAiRevision 改为新的包装函数
    return previewChapterAiRevisionWithRouting(novelId, chapter.id, request);
    // ↑ 替代：return previewChapterAiRevision(novelId, chapter.id, request);
  },
  // ... 其他 onMutate, onSuccess, onError 保持不变
});

// 类似改动 continuationMutation
const continuationMutation = useMutation({
  mutationFn: async (request: ChapterEditorCursorContinuationRequest) => {
    if (!chapter) {
      throw new Error("当前未选中章节。");
    }
    // 【改动】从 previewChapterCursorContinuation 改为新的包装函数
    return previewChapterCursorContinuationWithRouting(novelId, chapter.id, request);
    // ↑ 替代：return previewChapterCursorContinuation(novelId, chapter.id, request);
  },
  // ... 其他保持不变
});
```

### **Step 3.3d：添加本地模式的错误处理和取消**

在现有的错误处理中，添加模式感知的错误信息：

```typescript
const previewMutation = useMutation({
  // ... mutationFn ...
  onError: (error) => {
    const isLocal = shouldUseLocalGeneration(novelId);
    const prefix = isLocal ? "【本地生成】" : "";
    setSession((current) => ({
      ...current,
      status: "error",
      errorMessage: prefix + (error instanceof Error ? error.message : "AI 修正失败，请重试。"),
    }));
  },
});

// 取消逻辑 - 可选，如果需要支持取消的话
const handleCancel = async () => {
  if (shouldUseLocalGeneration(novelId) && session.sessionId) {
    await cancelLocalGeneration(session.sessionId);
  } else {
    // 既有云模式取消逻辑
  }
  resetSession();
};
```

---

## 📊 实现时间分解

| Step | 任务 | 工作量 | 难度 |
|------|------|--------|------|
| 3.3a | 本地生成处理函数 | 1h | 中 |
| 3.3b | API 包装函数 | 30 min | 低 |
| 3.3c | ChapterEditor 调用改造 | 30 min | 低 |
| 3.3d | 错误处理和取消 | 30 min | 低 |
| **验证** | TypeScript + E2E 测试 | 1h | — |
| **总计** | — | **~3.5h** | — |

---

## ⚠️ 技术风险与缓解方案

### **风险 1：事件订阅的异步复杂性**

**问题**：本地模式需要异步订阅事件，云模式是同步 API 调用。

**缓解**：
- 在包装函数中统一处理异步逻辑
- 返回相同格式的 Promise，对上层透明
- 使用 Promise 封装事件订阅

### **风险 2：响应格式差异**

**问题**：本地 Tauri 命令的响应格式与云 API 不同。

**缓解**：
- 在包装函数中做格式转换
- 确保返回相同的 `ChapterEditorResponse` 结构
- 添加类型检查和转换函数

### **风险 3：生成进度实时性**

**问题**：本地生成的进度事件需要实时展示。

**缓解**：
- 从 Tauri 订阅事件中提取进度信息
- 转换为 UI 可理解的格式
- 可选：添加本地模式的进度条

---

## ✅ 验收清单

### 代码质量
- [ ] TypeScript 编译零错误
- [ ] 新增函数有完整注释
- [ ] 类型签名一致

### 功能验收
- [ ] 本地模式能启动生成
- [ ] 云模式行为不变
- [ ] 切换模式后生成结果正确
- [ ] 错误处理清晰

### 集成验证
- [ ] ChapterEditorShell 改动最小（仅改 mutationFn）
- [ ] 既有云模式完全兼容
- [ ] 本地和云模式的 UI 表现一致

---

## 🎯 立即行动计划

**建议分两个 mini-session 完成**：

### **Session 1**（当前，~1.5h）
- 3.3a：编写本地生成处理函数
- 3.3b：编写 API 包装函数
- TypeScript 编译验证

### **Session 2**（后续，~1.5h）
- 3.3c：ChapterEditor 改造
- 3.3d：错误处理
- 完整端对端测试

---

## 📌 关键决策

### **为什么不直接改 ChapterEditorShell？**

**原因**：
- 该组件已有复杂的状态管理
- 直接改动会影响多个地方
- 较容易引入 bug

**替代方案**（现在的设计）：
- 在 API 层做分支
- ChapterEditorShell 调用新函数，其他不变
- 风险最小化

### **为什么选择包装函数而非拦截器？**

**原因**：
- 包装函数更明确、易维护
- 可清晰跟踪本地 vs 云的分支
- 便于单元测试

---

这个设计方案的核心是 **API 层路由，Component 层无感知**。

下一步是实现这个方案。你准备好开始了吗？

