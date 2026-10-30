# 模型来源、任务路由与推理强度

## 背景

桌面版把「厂商账号」换成了本地的模型来源注册表：一处来源 = 地址 + 协议 + 密钥引用，
模型条目挂在这条来源下，任务绑定把「创作环节 → 来源 + 模型 + 推理强度」定下来。
网页版那套按厂商枚举的接口在桌面端不存在，界面必须走注册表，否则就会落到「未接入」的兜底。

## 当前规则

1. **单一事实来源**：`provider_credential`（来源）→ `provider_model`（模型）→ `model_binding`（环节绑定）。
   界面与运行时都只读这三张表，不再各自维护一份默认配置。
2. **密钥只存引用**：绑定与来源里保存的是密钥槽名字，真正的密钥进系统钥匙库；
   钥匙库写入后必须回读校验，读不回来时退回应用数据目录的 0600 文件，读取路径两处都查。
3. **首屏配置要回填注册表**：`save_model_setup` 除了写旧的默认配置，还会把来源、模型、`writer` 绑定登记进注册表，
   否则用户「配置完成」后界面仍显示未配置。
4. **推理强度是绑定属性**：`model_binding.reasoning_effort` 决定该环节用哪档强度；
   进入一次运行后冻结进 `RunContract.reasoning_effort`，中途不再变化。
5. **发送规则**：OpenAI 兼容协议把 `reasoning_effort` 放进请求体；未选择时不带该字段。
   Ollama 协议没有对应字段，显式忽略，不伪造参数。
6. **地址许可**：默认只接受 https；用户填 `http://` 时视为显式允许明文连接（自建网关常见），
   元数据与链路本地地址始终拒绝。模型列表 404 时会再试一次 `/v1/models`，覆盖只暴露 `/v1` 的网关。

## 失败模式与排查

| 现象 | 常见原因 | 排查路径 |
| --- | --- | --- |
| 界面提示「暂未接入」 | 某个界面调用了尚未映射的网页接口 | 桌面适配层会记录 `NOT_MIGRATED METHOD PATH` 到应用日志与 `window.__missingRoutes` |
| 添加模型后仍显示未配置 | 密钥没写进去，或注册表没回填 | `get_model_setup` 与 `list_provider_profiles` 看 `hasApiKey` / `credentialPresent` |
| 获取模型列表失败 | 地址缺 `/v1`、密钥错、明文 http 未被允许 | 错误文案现在带错误码（`AUTH_FAILED` / `NOT_SUPPORTED` / `NETWORK_ERROR`） |
| 推理强度不生效 | 协议不是 OpenAI 兼容，或没选档位 | 看绑定是否带 `reasoningEffort`，再看请求体是否带 `reasoning_effort` |

## 相关模块

- 界面：`client/src/components/layout/DesktopModelSelector.tsx`、`client/src/pages/settings/ProviderRegistrySection.tsx`
- 适配：`client/src/platform/desktopApiAdapter.ts`、`client/src/platform/desktopModelRules.ts`
- 服务：`crates/novel-application/src/provider_service.rs`
- 运行时：`crates/novel-application/src/worker.rs`、`src-tauri/src/commands/generation.rs`
- 供应商实现：`crates/novel-llm/src/provider.rs`

## 已知取舍

- 钥匙库不可靠时，密钥以 ChaCha20-Poly1305 加密落到应用数据目录（文件 0600，主密钥优先放钥匙库、否则同目录 0600）；
  历史明文文件在首次读取时自动加密迁移，篡改或跨槽解密会直接失败而不会误用。
- 明文 HTTP 的授权绑定到主机：地址换到别的主机必须显式再确认一次，避免一次授权被复用到任意目标。
- 推理强度档位是自由文本，不做白名单校验；Ollama 协议没有该参数，界面会提示「该协议不支持强度」并仍按绑定保存。
