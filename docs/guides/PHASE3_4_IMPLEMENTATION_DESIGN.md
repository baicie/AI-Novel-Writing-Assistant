# 阶段 3.4 本地生成完善设计方案

## 📋 数据结构理解

### 关键类型定义（已确认）

**GenerationEvent** - 后端生成过程中发送的事件
```typescript
type GenerationEvent =
  | { kind: "run_started"; runId: string; seq: number; occurredAt: number }
  | { kind: "step_started"; runId: string; seq: number; stepId: string; occurredAt: number }
  | { kind: "token_delta"; runId: string; stepId: string; text: string }
  | { kind: "checkpoint_saved"; runId: string; seq: number; stepId: string; candidateId: string; occurredAt: number }
  | { kind: "needs_approval"; runId: string; seq: number; stepId: string; occurredAt: number }
  | { kind: "step_succeeded"; runId: string; seq: number; stepId: string; occurredAt: number }
  | { kind: "run_paused"; runId: string; seq: number; stepId?: string; occurredAt: number }
  | { kind: "run_failed"; runId: string; seq: number; occurredAt: number }
  | { kind: "run_completed"; runId: string; seq: number; occurredAt: number }
```

**CommandOutcome** - 生成启动后返回的结果
```typescript
interface CommandOutcome {
  runId: string;      // 生成任务ID
  status: string;     // 状态
  replayed: boolean;  // 是否重放了历史事件
}
```

**StartGeneration** - 启动生成的请求
```typescript
interface StartGeneration {
  commandId: string;
  novelId: string;
  targetChapterId: string;
  provider: string;      // "opencode-go" 等
  model: string;         // "gpt-4" 等
  promptVersions: Record<string, string>;
  tokenBudget?: number;
  maxAutomaticRetries: number;
  endpoint: string;      // API 端点
  allowLocalhostHttp: boolean;
}
```

### 关键事件流

1. **run_started** - 生成开始
2. **step_started** - 某个步骤开始（如修正、续写等）
3. **token_delta** - 接收到新的 token（进度反馈）
4. **checkpoint_saved** - 保存了一个候选方案（关键事件）
5. **step_succeeded** - 步骤成功完成
6. **run_completed** - 整个生成完成

---

## 🎯 实现方案

### 设计原则

1. **最小改动** - 仅改占位函数的实现
2. **清晰映射** - 将 GenerationEvent 映射为 ChapterEditorResponse
3. **兼容性** - 返回的格式与云 API 完全一致
4. **错误处理** - 适当处理事件流中的错误

### Step 3.4a：完善 handleLocalAiRevision

**目标**：启动本地生成，订阅事件，收集候选方案

```typescript
/**
 * 本地模式 AI 修正生成 - 真实实现
 */
export async function handleLocalAiRevision(
  novelId: string,
  chapterId: string,
  request: unknown,
): Promise<any> {
  // 1. 类型转换 - 强制转换为预期的请求结构
  const aiRequest = request as {
    source?: string;
    scope?: string;
    instruction?: string;
    selection?: { from: number; to: number; text: string };
    provider?: string;
    model?: string;
    temperature?: number;
    [key: string]: unknown;
  };

  // 2. 获取模型配置
  const modelSetup = await getLocalModelSetup();
  if (!modelSetup) {
    throw new Error("本地模型配置未找到。请在设置中配置 API Key。");
  }

  // 3. 构建本地生成请求
  const startGenRequest = {
    commandId: generateId(),
    novelId,
    targetChapterId: chapterId,
    provider: aiRequest.provider || "opencode-go",
    model: aiRequest.model || "gpt-4",
    promptVersions: {
      source: aiRequest.source || "freeform",
      scope: aiRequest.scope || "selection",
      instruction: aiRequest.instruction || "",
    },
    maxAutomaticRetries: 3,
    endpoint: modelSetup.endpoint,
    allowLocalhostHttp: modelSetup.allowLocalhostHttp,
  };

  // 4. 启动生成并收集候选
  const candidates: ChapterEditorCandidate[] = [];
  let sessionError: string | null = null;
  
  try {
    await startLocalGeneration(startGenRequest, (event) => {
      if (event.kind === "checkpoint_saved") {
        // 每次保存候选时添加到列表
        candidates.push({
          id: event.candidateId,
          source: aiRequest.source || "freeform",
          scope: aiRequest.scope || "selection",
          // 后续内容需要从其他事件或查询获取
          content: "", // TODO: 从其他地方获取生成的文本
          applied: false,
        });
      } else if (event.kind === "run_failed") {
        sessionError = "生成失败，请重试。";
      }
    });
  } catch (error) {
    throw new Error(
      error instanceof Error ? error.message : "本地生成启动失败"
    );
  }

  if (sessionError) {
    throw new Error(sessionError);
  }

  // 5. 返回 ChapterEditorResponse 格式
  return {
    data: {
      sessionId: startGenRequest.commandId,
      candidates,
      activeCandidateId: candidates[0]?.id ?? null,
      status: "ready",
    },
  };
}

/**
 * 本地模式光标续写生成 - 真实实现
 */
export async function handleLocalCursorContinuation(
  novelId: string,
  chapterId: string,
  request: unknown,
): Promise<any> {
  // 类似实现...
  // 逻辑基本相同，只是构建的 promptVersions 中 source 为 "continuation"
}
```

### Step 3.4b：添加生成事件的订阅和收集

**问题**：上面的实现有个缺陷 - 无法获取生成的实际文本内容（content）。

**解决方案**：需要查询或从事件中组装文本

```typescript
/**
 * 本地生成事件处理器 - 组装文本内容
 */
function createEventProcessor() {
  const textBuffer: Record<string, string> = {};
  const events: GenerationEvent[] = [];

  return {
    process(event: GenerationEvent) {
      events.push(event);

      if (event.kind === "token_delta") {
        // 累积 token 为文本
        if (!textBuffer[event.stepId]) {
          textBuffer[event.stepId] = "";
        }
        textBuffer[event.stepId] += event.text;
      }
    },

    getText(stepId: string): string {
      return textBuffer[stepId] || "";
    },

    getCheckpoints(): Array<{
      candidateId: string;
      stepId: string;
      text: string;
    }> {
      return events
        .filter((e) => e.kind === "checkpoint_saved")
        .map((e: any) => ({
          candidateId: e.candidateId,
          stepId: e.stepId,
          text: textBuffer[e.stepId] || "",
        }));
    },
  };
}
```

### Step 3.4c：改进的完整实现

```typescript
export async function handleLocalAiRevision(
  novelId: string,
  chapterId: string,
  request: unknown,
): Promise<any> {
  const aiRequest = request as {
    source?: string;
    scope?: string;
    instruction?: string;
    selection?: { from: number; to: number; text: string };
    provider?: string;
    model?: string;
    [key: string]: unknown;
  };

  const modelSetup = await getLocalModelSetup();
  if (!modelSetup) {
    throw new Error("本地模型配置未找到。请在设置中配置 API Key。");
  }

  const processor = createEventProcessor();
  const commandId = generateId();

  try {
    await startLocalGeneration(
      {
        commandId,
        novelId,
        targetChapterId: chapterId,
        provider: aiRequest.provider || "opencode-go",
        model: aiRequest.model || "gpt-4",
        promptVersions: {
          source: aiRequest.source || "freeform",
          scope: aiRequest.scope || "selection",
          instruction: aiRequest.instruction || "",
        },
        maxAutomaticRetries: 3,
        endpoint: modelSetup.endpoint,
        allowLocalhostHttp: modelSetup.allowLocalhostHttp,
      },
      (event) => processor.process(event)
    );
  } catch (error) {
    throw new Error(
      error instanceof Error ? error.message : "本地生成失败"
    );
  }

  const checkpoints = processor.getCheckpoints();
  if (checkpoints.length === 0) {
    throw new Error("生成未产生任何候选方案，请重试。");
  }

  return {
    data: {
      sessionId: commandId,
      candidates: checkpoints.map((cp) => ({
        id: cp.candidateId,
        source: aiRequest.source || "freeform",
        scope: aiRequest.scope || "selection",
        content: cp.text,
        applied: false,
      })),
      activeCandidateId: checkpoints[0]?.candidateId ?? null,
      status: "ready",
    },
  };
}
```

---

## 📊 实现时间分解

| Step | 任务 | 工作量 | 难度 |
|------|------|--------|------|
| 3.4a | 完善 handleLocalAiRevision | 45 min | 中 |
| 3.4b | 完善 handleLocalCursorContinuation | 30 min | 中 |
| 3.4c | 添加事件处理和文本累积 | 30 min | 低 |
| **验证** | TypeScript + 集成测试 | 1h | — |
| **总计** | — | **~2.5h** | — |

---

## ⚠️ 技术风险

### **风险 1：异步事件流的完整性**

**问题**：事件可能没有全部接收到。

**缓解**：
- Tauri 的 `startGeneration` 函数已处理事件重放
- 使用 `replayRunEvents` 获取已完成的事件

### **风险 2：响应格式不匹配**

**问题**：本地返回的格式与云 API 可能有差异。

**缓解**：
- 明确类型转换（`as ChapterEditorCandidate`）
- 在测试中验证响应格式

### **风险 3：文本累积不完整**

**问题**：某些 token 可能丢失。

**缓解**：
- 使用集合确保每个 token 只计算一次（已在 startGeneration 中实现）

---

## ✅ 验收清单

### 代码质量
- [ ] TypeScript 编译零错误
- [ ] 函数有完整注释
- [ ] 错误消息清晰

### 功能验收
- [ ] 能启动本地生成
- [ ] 能收集候选方案
- [ ] 能返回正确的格式
- [ ] 错误处理适当

### 集成验收
- [ ] ChapterEditorShell 接收正确的响应
- [ ] 候选方案显示在 UI 中
- [ ] 可接受或拒绝候选方案
- [ ] 云模式仍正常工作

---

## 🎯 立即行动

**准备好开始实施吗？**

建议的实施顺序：
1. 实现 createEventProcessor()
2. 完善 handleLocalAiRevision()
3. 完善 handleLocalCursorContinuation()
4. TypeScript 编译验证
5. 集成测试

