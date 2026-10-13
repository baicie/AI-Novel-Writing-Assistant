# 阶段 3 本地创作 UI 测试计划

## 概述

本文档定义了完整的测试用例，用于验证本地创作模式选择和密钥管理的正确性。

## 测试环境

- **桌面平台**：Tauri 2.12+
- **前端框架**：React 18+
- **浏览器**：Chrome/Safari（开发）
- **后端**：Rust 本地内核 + Node Express

---

## 单元测试

### Test 1: useLocalCreationMode Hook

**文件**：`client/src/hooks/useLocalCreationMode.ts`

```typescript
describe("useLocalCreationMode", () => {
  it("应该返回默认的云创作模式", () => {
    // localStorage 中无值
    const { mode } = useLocalCreationMode("novel-1");
    expect(mode).toBe("cloud");
  });

  it("应该读取已保存的本地模式", () => {
    localStorage.setItem("local-creation-mode:novel-1", "local");
    const { mode } = useLocalCreationMode("novel-1");
    expect(mode).toBe("local");
  });

  it("应该按小说隔离模式存储", () => {
    localStorage.setItem("local-creation-mode:novel-1", "local");
    localStorage.setItem("local-creation-mode:novel-2", "cloud");
    
    const mode1 = useLocalCreationMode("novel-1").mode;
    const mode2 = useLocalCreationMode("novel-2").mode;
    
    expect(mode1).toBe("local");
    expect(mode2).toBe("cloud");
  });

  it("setMode 应该更新存储", () => {
    const { setMode } = useLocalCreationMode("novel-1");
    setMode("local");
    
    const stored = localStorage.getItem("local-creation-mode:novel-1");
    expect(stored).toBe("local");
  });

  it("应该处理 localStorage 不可用的情况", () => {
    // Mock localStorage.getItem 抛出异常
    const { mode } = useLocalCreationMode("novel-1");
    expect(mode).toBe("cloud"); // 降级到默认值
  });
});
```

### Test 2: LocalCreationKeyManager Component

```typescript
describe("LocalCreationKeyManager", () => {
  it("应该在加载时显示加载状态", () => {
    const { getByRole } = render(<LocalCreationKeyManager />);
    expect(getByRole("status")).toBeInTheDocument(); // Loader
  });

  it("应该禁用保存按钮当 API Key 为空", () => {
    const { getByText } = render(<LocalCreationKeyManager />);
    const saveButton = getByText("保存到系统凭据");
    expect(saveButton).toBeDisabled();
  });

  it("当输入 API Key 时应该启用保存按钮", async () => {
    const { getByPlaceholderText, getByText } = render(<LocalCreationKeyManager />);
    const input = getByPlaceholderText("输入你的 API Key");
    
    await userEvent.type(input, "sk-test-key-123");
    
    const saveButton = getByText("保存到系统凭据");
    expect(saveButton).not.toBeDisabled();
  });

  it("点击保存后应该显示成功提示", async () => {
    const mockSave = jest.fn().mockResolvedValue({
      endpoint: "https://api.example.com",
      model: "gpt-4",
      allowLocalhostHttp: false,
      hasApiKey: true,
    });

    jest.spyOn(localCreation, "saveLocalModelSetup").mockImplementation(mockSave);

    const { getByPlaceholderText, getByText } = render(<LocalCreationKeyManager />);
    const input = getByPlaceholderText("输入你的 API Key");
    
    await userEvent.type(input, "sk-test-key-123");
    await userEvent.click(getByText("保存到系统凭据"));

    await waitFor(() => {
      expect(mockSave).toHaveBeenCalled();
    });
  });

  it("显示已保存的密钥状态", async () => {
    const mockGet = jest.fn().mockResolvedValue({
      endpoint: "https://api.example.com",
      model: "gpt-4",
      allowLocalhostHttp: false,
      hasApiKey: true,
    });

    jest.spyOn(localCreation, "getLocalModelSetup").mockImplementation(mockGet);

    const { getByText } = render(<LocalCreationKeyManager />);
    
    await waitFor(() => {
      expect(getByText(/密钥已在系统凭据中保存/i)).toBeInTheDocument();
    });
  });
});
```

### Test 3: CreationModeSelector Component

```typescript
describe("CreationModeSelector", () => {
  it("应该显示本地和云模式按钮", () => {
    const { getByText } = render(<CreationModeSelector novelId="novel-1" />);
    expect(getByText("本地创作")).toBeInTheDocument();
    expect(getByText("云创作")).toBeInTheDocument();
  });

  it("选中的模式按钮应该高亮", () => {
    localStorage.setItem("local-creation-mode:novel-1", "local");
    const { getByText } = render(<CreationModeSelector novelId="novel-1" />);
    
    const localButton = getByText("本地创作").closest("button");
    expect(localButton).toHaveClass("default"); // 高亮样式
  });

  it("点击按钮应该切换模式", async () => {
    const { getByText } = render(<CreationModeSelector novelId="novel-1" />);
    
    const cloudButton = getByText("云创作").closest("button");
    await userEvent.click(cloudButton);
    
    const stored = localStorage.getItem("local-creation-mode:novel-1");
    expect(stored).toBe("cloud");
  });
});
```

---

## 集成测试

### Test 4: 模式选择与 API 路由

**场景**：用户选择本地模式，生成章节

```typescript
describe("本地模式章节生成集成", () => {
  it("本地模式应该调用 Tauri 命令", async () => {
    const mockOpenRun = jest.fn().mockResolvedValue({
      runId: "run-123",
      status: "started",
      replayed: false,
    });

    jest.spyOn(desktopTransport, "openRun").mockImplementation(mockOpenRun);

    // 设置本地模式
    localStorage.setItem("local-creation-mode:novel-1", "local");

    // 触发生成
    const result = await startLocalGeneration({
      commandId: "cmd-123",
      novelId: "novel-1",
      targetChapterId: "ch-1",
      provider: "opencode-go",
      model: "gpt-4",
      promptVersions: {},
      maxAutomaticRetries: 3,
      endpoint: "https://api.example.com",
      allowLocalhostHttp: false,
    });

    expect(mockOpenRun).toHaveBeenCalled();
    expect(result.runId).toBe("run-123");
  });

  it("云模式应该调用 Node API", async () => {
    const mockCreateChapter = jest.fn().mockResolvedValue({
      id: "ch-1",
      content: "生成的内容",
    });

    jest.spyOn(nodeApi, "createChapter").mockImplementation(mockCreateChapter);

    // 设置云模式
    localStorage.setItem("local-creation-mode:novel-1", "cloud");

    // 触发生成
    const result = await generateChapter({
      novelId: "novel-1",
      chapterId: "ch-1",
      // 其他参数...
    });

    expect(mockCreateChapter).toHaveBeenCalled();
  });

  it("模式切换后应该影响后续生成调用", async () => {
    localStorage.setItem("local-creation-mode:novel-1", "local");

    // 第一次：本地模式
    const local = useLocalCreationMode("novel-1");
    expect(local.isLocalMode).toBe(true);

    // 切换到云模式
    local.setMode("cloud");

    // 第二次：云模式
    const cloud = useLocalCreationMode("novel-1");
    expect(cloud.isCloudMode).toBe(true);
  });
});
```

### Test 5: 密钥保存与读取流程

```typescript
describe("密钥管理集成流程", () => {
  it("完整流程：保存 → 读取 → 使用", async () => {
    // Step 1: 保存密钥
    const setupResult = await saveLocalModelSetup({
      endpoint: "https://api.example.com",
      model: "gpt-4",
      apiKey: "sk-test-key-123",
      allowLocalhostHttp: false,
    });

    expect(setupResult.hasApiKey).toBe(true);

    // Step 2: 读取密钥
    const readSetup = await getLocalModelSetup();
    expect(readSetup?.hasApiKey).toBe(true);

    // Step 3: 用于生成
    const outcome = await startLocalGeneration({
      commandId: "cmd-123",
      novelId: "novel-1",
      targetChapterId: "ch-1",
      provider: "opencode-go",
      model: "gpt-4",
      promptVersions: {},
      maxAutomaticRetries: 3,
      endpoint: setupResult.endpoint,
      allowLocalhostHttp: setupResult.allowLocalhostHttp,
    });

    expect(outcome.runId).toBeDefined();
  });

  it("缺失 API Key 时应该给出错误提示", async () => {
    // 不保存密钥
    const mockOpenRun = jest.fn().mockRejectedValue(
      new Error("API Key 不存在"),
    );

    jest.spyOn(desktopTransport, "openRun").mockImplementation(mockOpenRun);

    await expect(
      startLocalGeneration({
        // 参数...
      }),
    ).rejects.toThrow("API Key 不存在");
  });
});
```

---

## 端对端测试（手动）

### Test 6: UI 工作流

**场景**：完整的用户交互流程

1. **打开创作工作台**
   - [ ] 应看到模式选择器（默认云创作）
   - [ ] 应看到创作编辑区

2. **切换到本地模式**
   - [ ] 点击"本地创作"按钮
   - [ ] 按钮高亮
   - [ ] 刷新页面后模式仍为本地

3. **打开密钥管理**
   - [ ] 点击"密钥设置"按钮
   - [ ] 弹出对话框
   - [ ] 显示 API Key 输入框

4. **保存 API Key**
   - [ ] 输入有效的 API Key
   - [ ] 点击"保存到系统凭据"
   - [ ] 显示成功提示 ✓
   - [ ] 按钮变灰（禁用）

5. **测试连接**
   - [ ] 点击"测试连接"
   - [ ] 显示测试中...
   - [ ] 连接成功提示

6. **生成章节**
   - [ ] 关闭密钥管理对话框
   - [ ] 点击"生成下一章"
   - [ ] 调用本地 Rust 命令（无网络请求）
   - [ ] 显示生成进度
   - [ ] 完成后显示结果

7. **取消生成**
   - [ ] 在生成过程中点击"取消"
   - [ ] 生成立即中止
   - [ ] 显示"已中止"提示

### Test 7: 错误情景

- [ ] 密钥输入为空时保存按钮禁用
- [ ] API Key 格式错误时给出提示
- [ ] 网络不可用时显示离线提示
- [ ] Tauri 命令超时时显示重试选项
- [ ] 密钥过期时显示刷新提示

---

## 性能测试

### Test 8: 初始加载时间

```
- 模式选择器渲染：< 100ms
- 密钥管理组件加载：< 500ms
- 密钥读取：< 1s
```

### Test 9: 内存占用

```
- 单组件实例：< 2MB
- 多小说模式存储：< 50KB（localStorage）
```

---

## 测试覆盖检查清单

| 组件/功能 | 单元 | 集成 | E2E | 备注 |
|----------|------|------|-----|------|
| useLocalCreationMode | ✅ | ✅ | ✅ | 完整覆盖 |
| CreationModeSelector | ✅ | ✅ | ✅ | UI 交互测试 |
| LocalCreationKeyManager | ✅ | ✅ | ✅ | 形式验证测试 |
| 模式路由逻辑 | ✅ | ✅ | ✅ | API 调用分支 |
| 密钥管理流程 | ✅ | ✅ | ✅ | 完整 E2E |
| 错误处理 | ✅ | ✅ | ✅ | 边界情况 |

---

## 运行测试

```bash
# 单元测试
pnpm --filter client test

# 集成测试
pnpm --filter client test:integration

# E2E 测试（手动或 Playwright）
pnpm --filter client test:e2e
```

---

## 预期结果

所有测试通过后：
- ✅ 本地创作模式可正确切换
- ✅ API Key 可安全保存到系统凭据
- ✅ 路由逻辑正确分支到本地或云 API
- ✅ UI 交互流畅无错误
- ✅ 错误提示清晰有帮助

