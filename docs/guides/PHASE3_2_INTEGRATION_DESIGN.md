# 阶段 3.2 精确集成设计方案

## 📋 架构分析结果

### 1. **小说编辑页面结构**
- **主入口**：`client/src/pages/novels/NovelEdit.tsx`（主逻辑容器）
- **视图组件**：`client/src/pages/novels/components/NovelEditView.tsx`（界面呈现）
- **标题栏**：使用 `<StepHero>` 组件（在 `workspaceShell.tsx` 定义）
- **位置特征**：
  - 标题栏在 NovelEditView 行 197-240
  - 标题栏右侧有 actions 区域（现有：简易模式、AI 自动导演、导出按钮）
  - 标题栏左侧 eyebrow 显示进度信息

### 2. **章节生成逻辑位置**
- **体积规划生成**：`client/src/pages/novels/hooks/useNovelVolumePlanning.generation.ts`
- **章节编辑组件**：`client/src/pages/novels/components/chapterEditor/ChapterEditorShell.tsx`
- **发现**：当前还没有统一的生成 API hook，生成逻辑分散在各个组件中
- **改动策略**：在 chapterEditor 中添加模式判断逻辑

### 3. **设置页面结构**
- **主页**：`client/src/pages/settings/SettingsPage.tsx`
- **结构**：Tab 式（通过 useState 选择）或 section 式（竖直排列）
- **现有内容**：模型厂商设置、API Key 管理等
- **位置特征**：可在底部或单独创建 section

---

## 🎯 阶段 3.2 分步实现方案

### **Step 2.1：在 NovelEditView 标题栏添加模式选择器**

**修改文件**：`client/src/pages/novels/components/NovelEditView.tsx`

**改动点**（行 197-240 的 StepHero 区域）：

```typescript
// 1. 导入模式选择器
import { CreationModeSelector } from "@/components/localCreation";

// 2. 在 StepHero 的 actions 中添加（第一个）
<StepHero
  // ... 其他 props ...
  actions={(
    <>
      {/* 【新增】本地创作模式选择器 */}
      <CreationModeSelector novelId={id} />

      {/* 既有的按钮... */}
      {onSwitchToSimpleMode ? (
        <Button type="button" variant="outline" ...>
          ...
        </Button>
      ) : null}
      {/* ... */}
    </>
  )}
/>
```

**验证清单**：
- [ ] 模式选择器显示在标题栏右侧
- [ ] 本地/云两个按钮可点击
- [ ] 刷新后模式保持

---

### **Step 2.2：章节编辑中集成模式路由**

**修改文件**：`client/src/pages/novels/components/chapterEditor/ChapterEditorShell.tsx`

**第一部分 - 导入和 Hook 设置**（顶部）：

```typescript
// 新增导入
import { useLocalCreationMode } from "@/hooks/useLocalCreationMode";
import { startLocalGeneration, cancelLocalGeneration } from "@/api/localCreation";

// 在组件中添加
export function ChapterEditorShell(props: ChapterEditorShellProps) {
  const { novelId, chapterId } = props;  // 假设这两个字段存在
  
  // 【新增】模式选择 hook
  const { isLocalMode } = useLocalCreationMode(novelId);
  
  // ... 既有代码 ...
}
```

**第二部分 - 生成方法改造**：

在现有的生成相关 mutation 中，根据 mode 选择 API：

```typescript
// 假设现有有一个 generateChapterMutation，改造它：

// 【查找现有的生成逻辑，通常是 useMutation 的形式】
// 将其改造为：

const generateChapterMutation = useMutation({
  mutationFn: async (params: GenerationParams) => {
    if (isLocalMode) {
      // 【本地模式】
      try {
        const outcome = await startLocalGeneration({
          commandId: generateId(),
          novelId,
          targetChapterId: chapterId,
          provider: params.provider || "opencode-go",
          model: params.model || "gpt-4",
          promptVersions: params.promptVersions || {},
          maxAutomaticRetries: 3,
          endpoint: params.endpoint || "https://api.default.com",
          allowLocalhostHttp: false,
        });
        return { 
          type: "local" as const, 
          runId: outcome.runId,
          outcome 
        };
      } catch (error) {
        toast.error(`本地生成失败: ${error instanceof Error ? error.message : "未知错误"}`);
        throw error;
      }
    } else {
      // 【云模式 - 既有逻辑】
      return {
        type: "cloud" as const,
        result: await existingCloudGenerationApi(params)
      };
    }
  },
  onSuccess: (data) => {
    if (data.type === "local") {
      toast.success("本地生成已启动");
      // 订阅事件
      subscribeLocalGeneration(data.runId, (event) => {
        // 处理进度事件
      });
    } else {
      // 既有云模式处理
    }
  },
  onError: (error) => {
    toast.error(error instanceof Error ? error.message : "生成失败");
  },
});
```

**验证清单**：
- [ ] 选择本地模式后点击生成调用 Tauri 命令
- [ ] 选择云模式后点击生成调用既有 API
- [ ] 错误提示明确区分模式

---

### **Step 2.3：生成进度显示改造**

**修改文件**：`client/src/pages/novels/components/chapterEditor/ChapterEditorShell.tsx`（或进度显示组件）

**改动**：在进度显示部分添加模式标签

```typescript
// 在显示生成进度的地方添加

{generationInProgress && (
  <div className="flex items-center gap-3 rounded-lg bg-blue-50 dark:bg-blue-950 px-3 py-2">
    <Loader2 className="h-4 w-4 animate-spin" />
    <span className="text-sm">
      正在生成…
      <Badge variant="secondary" className="ml-2">
        {isLocalMode ? "🖥️ 本地" : "☁️ 云"}
      </Badge>
    </span>
    <Button 
      variant="ghost" 
      size="sm"
      onClick={() => {
        if (isLocalMode) {
          cancelLocalGeneration(currentRunId);
        } else {
          existingCancelApi(currentRunId);
        }
      }}
    >
      取消
    </Button>
  </div>
)}
```

**验证清单**：
- [ ] 生成时显示本地/云标签
- [ ] 取消按钮在本地模式调用 cancelLocalGeneration

---

### **Step 2.4：在设置页添加本地创作密钥管理**

**修改文件**：`client/src/pages/settings/SettingsPage.tsx`

**改动**（在 JSX 返回部分末尾添加新 Section）：

```typescript
import { LocalCreationKeyManager } from "@/components/localCreation";

// 在 SettingsPage 组件的 return JSX 末尾添加

<section className="space-y-4 border-t pt-6">
  <div>
    <h2 className="text-lg font-semibold flex items-center gap-2 mb-2">
      <span>🖥️</span>
      本地创作 - 系统密钥设置
    </h2>
    <p className="text-sm text-muted-foreground mb-4">
      在此配置 API Key。本地生成时会自动从系统凭据读取，无需重复输入。
    </p>
  </div>

  <div className="max-w-md">
    <LocalCreationKeyManager />
  </div>

  <div className="text-xs text-muted-foreground space-y-1 mt-4">
    <p>💡 关于本地创作：</p>
    <ul className="ml-4 space-y-1 list-disc">
      <li>密钥存储在系统凭据中（macOS Keychain）</li>
      <li>生成完全离线运行，不需要网络连接</li>
      <li>支持在中断后继续创作</li>
    </ul>
  </div>
</section>
```

**验证清单**：
- [ ] 密钥管理组件在设置页显示
- [ ] 可输入和保存 API Key
- [ ] 测试连接按钮有效

---

### **Step 2.5：模式切换的用户提示**

**修改文件**：`client/src/components/localCreation/CreationModeSelector.tsx`

**现有代码已有基础结构，需要增强提示**：

```typescript
import { useLocalCreationMode } from "@/hooks/useLocalCreationMode";
import { toast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";

export function CreationModeSelector({ novelId }: { novelId: string }) {
  const { mode, setMode } = useLocalCreationMode(novelId);

  const handleModeChange = (newMode: CreationMode) => {
    if (newMode === mode) return;

    // 【提示信息】
    if (newMode === "local" && mode === "cloud") {
      toast.info("💡 已切换到本地创作。提示：首次使用需在设置中配置 API Key");
    } else if (newMode === "cloud" && mode === "local") {
      toast.info("✅ 已切换到云创作模式");
    }

    setMode(newMode);
  };

  return (
    <div className="flex items-center gap-2 rounded-lg bg-muted/50 p-1">
      <Button
        variant={mode === "local" ? "default" : "ghost"}
        size="sm"
        onClick={() => handleModeChange("local")}
        title="使用本地 Rust 内核，需配置 API Key（在设置中）"
        className="flex items-center gap-2"
      >
        <Cpu className="h-4 w-4" />
        <span className="hidden sm:inline">本地创作</span>
      </Button>

      <Button
        variant={mode === "cloud" ? "default" : "ghost"}
        size="sm"
        onClick={() => handleModeChange("cloud")}
        title="使用云端服务生成"
        className="flex items-center gap-2"
      >
        <Cloud className="h-4 w-4" />
        <span className="hidden sm:inline">云创作</span>
      </Button>
    </div>
  );
}
```

---

## 📊 实现时间分解

| Step | 任务 | 工作量 | 技术难度 |
|------|------|--------|---------|
| 2.1 | 标题栏添加选择器 | 15 min | 极低 |
| 2.2 | 生成逻辑路由 | 45 min | 中等 |
| 2.3 | 进度显示改造 | 20 min | 低 |
| 2.4 | 设置页添加管理器 | 15 min | 低 |
| 2.5 | 提示增强 | 10 min | 极低 |
| **验证** | 端对端测试 + Bug 修复 | 1h | — |
| **总计** | — | **~2.5h** | — |

---

## ✅ 实现检查清单

### 代码改动检查
- [ ] NovelEditView.tsx 添加模式选择器（导入 + JSX）
- [ ] ChapterEditorShell.tsx 添加模式路由（导入 + Hook + mutation 改造）
- [ ] 生成取消逻辑集成
- [ ] SettingsPage.tsx 添加密钥管理（导入 + JSX）
- [ ] CreationModeSelector.tsx 增强提示

### 编译检查
- [ ] `pnpm --filter client typecheck` 零错误
- [ ] `pnpm --filter client build` 通过

### 功能验证
- [ ] 模式选择器显示和切换
- [ ] 本地模式调用 Tauri 命令
- [ ] 云模式调用既有 API
- [ ] 进度显示准确
- [ ] 取消按钮有效
- [ ] 密钥管理界面可用
- [ ] 切换时有提示

### 持久化验证
- [ ] 刷新页面模式保持
- [ ] localStorage 正确存储
- [ ] 系统凭据正确保存

---

## 🎯 立即行动方案

**我建议**：

1. ✅ 我负责**精确实现上述 5 个 Step**
2. ✅ 逐个修改文件，保持代码清晰
3. ✅ 每个 Step 完成后验证 TypeScript 编译
4. ✅ 最后统一验证并提交

**时间估计**：约 2.5-3 小时完成所有集成

**风险**：极低（都是 UI 集成，不涉及核心业务逻辑）

---

**你确认我开始实施吗？**

