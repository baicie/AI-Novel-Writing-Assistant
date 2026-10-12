# 阶段 3：Desktop UI 本地创作适配计划

## 概述

本阶段将 Rust 本地创作内核与 Desktop 前端集成，使用户能在工作台选择本地或云创作模式。重点是：
- 最小化 UI 改动（复用既有创作工作台）
- 明确的模式选择与路由
- 系统凭据与密钥管理
- 生成进度与取消交互

## 设计目标

### 1. 模式选择（创作工作台主入口）

**位置**：创作工作台工具栏或侧栏

**UI**：
```
[本地创作] [云创作] ← 互斥选择
```

**行为**：
- 默认为云创作（保持兼容）
- 选择本地创作时，后续所有章节生成调用 Tauri 命令
- 模式选择保存到本地存储（per novel）

### 2. 密钥管理界面

**位置**：模型配置页或创作工作台的"本地密钥"侧栏

**模式**：
- 本地模式选中时，显示"系统密钥管理"部分
- 云模式时隐藏（使用既有配置）

**交互**：
```
【系统密钥】
━━━━━━━━━━━━━━━━
模型厂商：[OpenCode Go ▼]
API Key：  [••••••••] 已保存
          [修改] [测试连接]
━━━━━━━━━━━━━━━━
```

**流程**：
1. 填入 API Key
2. 点击"测试连接"→ 调用 `get_model_setup()` 验证
3. 点击"保存"→ 调用 `save_model_setup()` 写入 Keychain
4. 后续使用无需重复输入（自动从系统凭据读）

### 3. 生成进度与取消

**位置**：正文生成加载状态下方

**当前**（云模式）：
```
正在生成第 5 段...
[取消] ← 调用现有 API
```

**本地模式**：
```
正在生成第 5 段... [##########] 45%
[取消] ← 调用 cancel_generation(run_id)
```

**实现**：
- Tauri 命令支持流式进度回调（可选）
- 当前最小实现：同步返回结果，取消立即中止

### 4. 章节列表与版本查询

**位置**：本书导演台 → 现有"章节历史"或新增"章节管理"标签

**功能**：
- 点击"刷新"→ 调用 `list_chapters(novel_id)`
- 显示本地保存的所有章节
- 支持版本对比（本地 vs 已发布）

---

## 技术方案

### A. 模式选择存储

```typescript
// client/src/hooks/useLocalCreationMode.ts
const useLocalCreationMode = (novelId: string) => {
  // 存储 key: `local-creation-mode:${novelId}`
  // 值: 'local' | 'cloud'
  const [mode, setMode] = useLocalStorage(`local-creation-mode:${novelId}`, 'cloud');
  return { mode, setMode };
};
```

### B. Tauri 命令调用包装

```typescript
// client/src/api/desktopTransport.ts (既有文件，扩展)
export const desktopClient = {
  // 既有命令...
  
  // 新增本地创作命令
  listChapters: (novelId: string) =>
    invoke('list_chapters', { novelId }),
  
  getModelSetup: () =>
    invoke('get_model_setup'),
  
  saveModelSetup: (endpoint: string, model: string, apiKey: string) =>
    invoke('save_model_setup', { endpoint, model, apiKey, allowLocalhostHttp: false }),
  
  cancelGeneration: (runId: string) =>
    invoke('cancel_generation', { runId }),
};
```

### C. 章节生成路由

```typescript
// client/src/services/novel/writer.ts (伪代码)
const generateChapter = async (params: ChapterGenParams) => {
  const mode = getLocalCreationMode(novelId);
  
  if (mode === 'local') {
    // 本地创作流程
    return desktopClient.openRun({
      commandId: generateId(),
      novelId,
      targetChapterId: chapterId,
      provider: selectedProvider,
      model: selectedModel,
      endpoint: modelConfig.endpoint,
      apiKey: null, // 从系统凭据自动读取
      // ... 其他参数
    });
  } else {
    // 云创作（既有流程）
    return nodeApi.createChapter(params);
  }
};
```

### D. 密钥管理 UI 组件

```tsx
// client/src/components/LocalCreationKeyManager.tsx
export const LocalCreationKeyManager: React.FC<{novelId: string}> = ({ novelId }) => {
  const [apiKey, setApiKey] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [setup, setSetup] = useState<ModelSetupView | null>(null);
  
  const handleTestConnection = async () => {
    try {
      const result = await desktopClient.getModelSetup();
      if (result?.hasApiKey) {
        toast.success('连接成功，密钥已保存');
        setSetup(result);
      } else {
        toast.error('未找到已保存的密钥');
      }
    } catch (e) {
      toast.error(`连接失败：${e.message}`);
    }
  };
  
  const handleSave = async () => {
    setIsSaving(true);
    try {
      const result = await desktopClient.saveModelSetup(
        modelConfig.endpoint,
        selectedModel,
        apiKey
      );
      setSetup(result);
      toast.success('密钥已保存到系统凭据');
      setApiKey(''); // 清空输入框
    } catch (e) {
      toast.error(`保存失败：${e.message}`);
    } finally {
      setIsSaving(false);
    }
  };
  
  return (
    <div className="space-y-4">
      <div>
        <label>API Key</label>
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="输入你的 API Key"
          className="w-full"
        />
      </div>
      
      {setup?.hasApiKey && (
        <div className="text-sm text-green-600">✓ 密钥已在系统凭据中保存</div>
      )}
      
      <div className="flex gap-2">
        <button onClick={handleTestConnection}>测试连接</button>
        <button onClick={handleSave} disabled={isSaving}>
          {isSaving ? '保存中...' : '保存到系统凭据'}
        </button>
      </div>
    </div>
  );
};
```

---

## 实现步骤

### Step 1：模式选择 UI（1h）
- [ ] 创作工作台添加"本地/云"模式选择器
- [ ] 模式选择保存到 localStorage
- [ ] 根据模式动态显示/隐藏相关 UI

**文件**：
- `client/src/components/workspace/CreationModeSelector.tsx` (新建)
- `client/src/hooks/useLocalCreationMode.ts` (新建)
- `client/src/components/workspace/NovelWorkspace.tsx` (改动)

### Step 2：密钥管理界面（2h）
- [ ] 创建密钥管理面板组件
- [ ] 集成 `desktopClient.saveModelSetup()` 和 `getModelSetup()`
- [ ] 添加测试连接按钮

**文件**：
- `client/src/components/LocalCreationKeyManager.tsx` (新建)
- `client/src/pages/settings/LocalCreationSettings.tsx` (新建)

### Step 3：章节生成路由（2h）
- [ ] 改进 `generateChapter()` 函数添加模式判断
- [ ] 本地模式调用 Tauri 命令（`desktopClient.openRun`）
- [ ] 云模式保持既有 Node API 调用
- [ ] 错误处理与重试逻辑

**文件**：
- `client/src/services/novel/generator.ts` (改动)
- `client/src/api/desktopTransport.ts` (扩展)

### Step 4：生成进度与取消（2h）
- [ ] 正文生成加载状态显示本地/云标识
- [ ] 取消按钮调用对应命令（`cancel_generation` 或既有 API）
- [ ] 错误提示与重试

**文件**：
- `client/src/components/workspace/TextGenerationProgress.tsx` (新建或改动)

### Step 5：章节列表与版本（1h）
- [ ] 添加"刷新本地章节"按钮
- [ ] 调用 `desktopClient.listChapters()`
- [ ] 显示版本对比（可选）

**文件**：
- `client/src/components/workspace/ChapterList.tsx` (改动)

### Step 6：测试与文档（2h）
- [ ] 端对端测试：模式选择 → 密钥保存 → 生成一章
- [ ] 错误场景：密钥缺失、网络错误、取消生成
- [ ] 更新用户指南

**文件**：
- `docs/guides/local-creation.md` (新建)
- 测试文档

---

## 预期用户流程

### 首次使用本地创作

```
1. 打开创作工作台
2. 选择"本地创作"模式
3. 弹出密钥管理面板
   → 选择模型厂商（e.g., OpenCode Go）
   → 填入 API Key
   → 点击"保存到系统凭据"
4. 密钥保存成功，返回工作台
5. 开始创作
   → 点击"生成下一章"
   → Tauri 命令自动从系统凭据读 Key
   → 离线生成第一章
6. 生成完成，可继续修改或生成下一章
```

### 后续使用

```
1. 打开创作工作台
2. 模式仍为"本地创作"（已保存）
3. 直接生成（不需重复输入 Key）
```

### 切换到云模式

```
1. 工作台选择"云创作"
2. 后续生成调用 Node API
3. 使用既有创作流程
```

---

## 已知限制与未来扩展

### 当前版本

- ✓ 单一模式（不支持混合）
- ✓ 同步生成（流式进度待优化）
- ✓ 单设备凭据（不跨设备同步）

### 未来可研究

1. **混合模式**：同一本书可选择按章节使用本地或云
2. **流式进度**：WebSocket 或 Tauri 事件流实时推送进度
3. **云端同步**：生成完后自动上传到服务器保存
4. **离线编辑**：无网络时编辑并缓存，回网后提交
5. **性能指标**：记录本地生成耗时、内存占用

---

## 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 密钥泄露 | 中 | Keychain 隔离，UI 不显示明文 |
| 本地生成超时 | 中 | 支持 cancel_generation 中止 |
| 版本不一致 | 低 | 版本对比工具，手动同步 |
| 模式切换故障 | 低 | 降级到云模式，清缓存 |

---

## 测试覆盖

### 单元测试

- [ ] 模式选择持久化
- [ ] 密钥管理与读写
- [ ] 路由逻辑分支

### 集成测试

- [ ] 密钥保存 → 生成 → 完成完整循环
- [ ] 取消生成中止
- [ ] 密钥缺失时的错误提示

### UI 测试（手动）

- [ ] 模式选择器响应
- [ ] 密钥输入与保存
- [ ] 生成进度显示
- [ ] 取消按钮功能

---

## 预计工作量

| 部分 | 工作量 | 所有者 |
|------|--------|--------|
| Step 1-2 | 3h | Frontend |
| Step 3-4 | 4h | Frontend + Backend |
| Step 5-6 | 3h | Frontend + QA |
| **总计** | **~10h** | — |

合理的单周期内可完成。

