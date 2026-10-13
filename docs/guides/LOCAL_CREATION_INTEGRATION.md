# 阶段 3 本地创作 UI 集成指南

## 概述

本文档说明如何在 Desktop UI 中集成本地创作模式选择器和密钥管理。

## 已实现的组件

### 1. Hook：useLocalCreationMode

**文件**：`client/src/hooks/useLocalCreationMode.ts`

**用途**：管理本地创作模式的 localStorage 存储

```typescript
const { mode, setMode, isLocalMode, isCloudMode } = useLocalCreationMode(novelId);

// mode: 'local' | 'cloud'
// setMode: (mode) => void
// isLocalMode: boolean
// isCloudMode: boolean
```

**特点**：
- 按小说存储模式（`local-creation-mode:${novelId}`）
- 默认为云创作模式（兼容性）
- 自动错误处理

---

### 2. API 层：localCreation.ts

**文件**：`client/src/api/localCreation.ts`

**导出函数**：
- `getLocalModelSetup()` - 读取密钥配置
- `saveLocalModelSetup(input)` - 保存 API Key
- `listLocalChapters(novelId)` - 查询本地章节列表
- `cancelLocalGeneration(runId)` - 取消生成
- `startLocalGeneration(input)` - 启动本地生成
- `subscribeLocalGeneration(runId, onEvent)` - 订阅生成事件

**特点**：
- 集中管理 Tauri 命令调用
- 统一错误处理与日志
- 类型安全

---

### 3. UI 组件：CreationModeSelector

**文件**：`client/src/components/localCreation/CreationModeSelector.tsx`

**使用方式**：

```typescript
import { CreationModeSelector } from "@/components/localCreation";

export function MyWorkspace({ novelId }: { novelId: string }) {
  return (
    <div className="flex items-center justify-between">
      <h1>创作工作台</h1>
      <CreationModeSelector novelId={novelId} />
    </div>
  );
}
```

**外观**：
```
┌──────────────────────────┐
│ 🖥️ 本地创作 ☁️ 云创作   │
│ [  selected  ] [ ghost ] │
└──────────────────────────┘
```

---

### 4. UI 组件：LocalCreationKeyManager

**文件**：`client/src/components/localCreation/LocalCreationKeyManager.tsx`

**使用方式**：

```typescript
import { LocalCreationKeyManager } from "@/components/localCreation";

export function LocalCreationSettings() {
  return (
    <div className="max-w-md">
      <h2>系统密钥设置</h2>
      <LocalCreationKeyManager />
    </div>
  );
}
```

**功能**：
- API Key 输入与保存
- 测试连接按钮
- 密钥状态显示
- 端点与模型信息展示

---

## 集成清单

### 步骤 1：在创作工作台添加模式选择器

**修改文件**：`client/src/pages/novels/components/workspaceShell.tsx` 或类似创作入口

```typescript
import { CreationModeSelector } from "@/components/localCreation";

export function CreationWorkspace({ novelId }: { novelId: string }) {
  return (
    <div>
      <header className="flex items-center justify-between">
        <h1>创作工作台</h1>
        <CreationModeSelector novelId={novelId} />
      </header>

      {/* 其他工作台内容 */}
    </div>
  );
}
```

### 步骤 2：在设置页面添加密钥管理

**修改文件**：`client/src/pages/settings/` 中的相关设置页面

```typescript
import { LocalCreationKeyManager } from "@/components/localCreation";

export function LocalCreationSettings() {
  return (
    <section className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold mb-4">本地创作设置</h2>
        <div className="max-w-md">
          <LocalCreationKeyManager />
        </div>
      </div>
    </section>
  );
}
```

### 步骤 3：在生成路由中使用模式

**修改文件**：`client/src/services/novel/generator.ts` 或创作生成逻辑

```typescript
import { useLocalCreationMode } from "@/hooks/useLocalCreationMode";
import { startLocalGeneration } from "@/api/localCreation";

export function useChapterGeneration(novelId: string) {
  const { isLocalMode } = useLocalCreationMode(novelId);
  
  const generate = async (params: GenerationParams) => {
    if (isLocalMode) {
      // 本地创作流程
      try {
        const outcome = await startLocalGeneration({
          commandId: generateId(),
          novelId,
          targetChapterId: params.chapterId,
          provider: params.provider,
          model: params.model,
          promptVersions: params.promptVersions || {},
          maxAutomaticRetries: 3,
          endpoint: params.endpoint,
          allowLocalhostHttp: false,
        });
        
        // 订阅事件
        await subscribeLocalGeneration(outcome.runId, (event) => {
          // 处理事件
        });
        
        return outcome;
      } catch (error) {
        toast.error(`本地生成失败: ${error.message}`);
        throw error;
      }
    } else {
      // 云创作流程（既有代码）
      return nodeApi.createChapter(params);
    }
  };
  
  return { generate };
}
```

### 步骤 4：生成取消集成

**修改生成进度组件**：

```typescript
import { useLocalCreationMode } from "@/hooks/useLocalCreationMode";
import { cancelLocalGeneration } from "@/api/localCreation";

export function TextGenerationProgress({ novelId, runId }: Props) {
  const { isLocalMode } = useLocalCreationMode(novelId);
  
  const handleCancel = async () => {
    try {
      if (isLocalMode) {
        await cancelLocalGeneration(runId);
        toast.success("本地生成已中止");
      } else {
        await nodeApi.cancelGeneration(runId);
      }
    } catch (error) {
      toast.error("取消生成失败");
    }
  };
  
  return (
    <div>
      <div className="flex items-center gap-2">
        <span>正在生成（{isLocalMode ? "本地" : "云"}）...</span>
        <Button onClick={handleCancel} variant="outline" size="sm">
          取消
        </Button>
      </div>
    </div>
  );
}
```

---

## TypeScript 类型检查

所有代码已通过 TypeScript 检查：

```bash
cd /Users/liuzhiwei/Desktop/workspace/git-code/AI-Novel-Writing-Assistant
pnpm --filter client typecheck
```

✅ **通过无错误**

---

## 下一步

1. **验证集成**：在实际页面中测试模式选择和密钥保存
2. **错误处理**：完善网络错误和边界情况处理
3. **UI 磨光**：根据设计稿调整样式和布局
4. **端对端测试**：完整流程测试（选择模式 → 保存密钥 → 生成章节）

---

## 现有 desktopTransport.ts 扩展

已添加以下导出到 `client/src/platform/desktopTransport.ts`：

```typescript
// 本地创作特定命令

export interface ModelSetupView {
  endpoint: string;
  model: string;
  allowLocalhostHttp: boolean;
  hasApiKey: boolean;
}

export function listChapters(novelId: string): Promise<ChapterSnapshot[]>
export function getModelSetup(): Promise<ModelSetupView | null>
export function saveModelSetup(input: {...}): Promise<ModelSetupView>
export function cancelGeneration(runId: string): Promise<void>
export function openRun(input: StartGeneration): Promise<CommandOutcome>
```

这些函数直接映射到 Rust Tauri 命令层。

