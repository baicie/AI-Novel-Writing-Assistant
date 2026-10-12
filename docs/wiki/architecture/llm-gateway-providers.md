# 多模型网关厂商接入边界

## 背景

OpenCode Go 这类订阅制网关用一个 API Key 转发 DeepSeek、Kimi、GLM、Qwen、MiniMax 等多家模型。对新手来说，它的价值是配置一次就能在多家模型之间切换，不必分别申请和维护多个厂商账号。

网关不等于上游官方接口：

- 各模型的原生协议不同，有 Chat Completions、Anthropic Messages 和 OpenAI Responses 三种。
- 网关对客户端有流量身份要求。
- 上游厂商的 `response_format`、思考开关等扩展参数是否透传，没有核验。

如果把网关当成某个官方厂商的别名接入，会出现三类问题：

- 模型能选但调不通。
- 结构化输出被不支持的参数打断。
- 请求被网关当成通用 SDK 流量。

这些问题会落到自动导演、章节生产和 Creative Hub 的每一次 AI 调用上。

## 决策

- 网关作为独立的内置厂商接入，例如 `opencode-go`。不复用任何上游厂商 id，也不按模型名套用上游官方端点的能力档案。
- 项目只通过 Chat Completions 调用网关。OpenCode Go 会把 Chat Completions 请求转换为模型的原生格式，所以 MiniMax、Qwen、Claude 这类原生走 Messages 的模型也走同一条通道。只支持 Responses 的模型（当前是 Grok）在模型目录中隐藏。这样新手看到的每个候选模型都能直接调用，模型路由也不需要为同一个厂商分裂出多种协议。
- 网关要求的客户端身份（应用自己的 User-Agent、稳定的会话 ID）属于厂商连接合同，统一在模型客户端的传输层附加。自动导演、章节生产、Creative Hub、连接测试等入口都自动继承，任何业务服务都不应自己拼这些请求头。

## 当前规则

- 厂商配置 `responsesOnlyModelPrefixes` 声明只支持 Responses 的模型前缀。远程模型目录拉取后按它过滤；静态候选清单里的每个模型也必须可调用，`server/tests/llmProviders.test.js` 守护这一点。
- 协议支持以网关实测为准，不要按官方文档的“原生端点”列推断。OpenCode Go 在鉴权前先校验模型与格式，所以可以不带 Key 探测：
  - 请求 `/chat/completions` 返回 `Model ... is not supported for format oa-compat`，说明这个模型不能走 Chat Completions。
  - 返回 `Missing API key`，说明格式可用。

  新增或调整前缀之前，先用这种方式逐个核验。
- 厂商配置 `sessionHeader` 声明会话头名称，OpenCode Go 是 `x-opencode-session`。会话 ID 以作品为单位：
  - 有 `promptMeta.novelId` 时，取它的 SHA-256 前 32 位，不直接外发内部 id。
  - 没有作品上下文时，使用进程级随机 ID。

  同一作品的规划、正文、审校请求落在同一会话里，方便网关做路由和提示词缓存。
- User-Agent 使用不带版本号的产品标识 `BizNovelStudio (+仓库地址)`。服务端拿不到桌面应用版本，不写死一个会过期的版本号。
- 身份请求头必须通过客户端的自定义 `fetch` 在请求发出前写入。原因是 LangChain `ChatOpenAI` 处理 `configuration.defaultHeaders` 时有两个行为：
  - 它把请求头转成小写键，再无条件写入自己的 `langchainjs-openai/...` User-Agent。
  - 它丢弃值为 `null` 的请求头。

  所以 `defaultHeaders` 既不能覆盖 User-Agent，也不能删除 SDK 默认发送的 `Authorization`。Anthropic 协议客户端用的是项目自己的 `fetch`，可以直接合并请求头，鉴权头始终最后写入。
- 结构化输出：
  - 网关一律走 `prompt_json`，`jsonCapabilities` 声明不支持原生 JSON。
  - 结构化档案落到默认分支（Qwen 模型落到兼容 Qwen 分支），单次结构化输出上限 8192。
  - 不要因为模型名是 deepseek、glm 或 kimi，就套用官方端点的 `json_object` 或 `json_schema`。
- 思考开关：DeepSeek、GLM 的 `thinking` 参数只发给官方端点或自定义厂商；网关内置厂商不发送，思考行为由网关和上游默认值决定。按模型身份收敛的参数约束仍然生效，例如 Kimi K3 的 temperature 固定为 `1.0`。
- 缓存：网关地址不在显式缓存白名单内，请求不加 `cache_control`。缓存收益依赖网关按会话 ID 做的自动缓存。
- OpenCode Go 官方定位是编程智能体流量，会监控影响其他用户的异常流量，每个模型的月度额度也不同。默认模型选择额度高、价格低的 `deepseek-v4.1-flash`，以降低长篇连续生产中途触顶的概率。

## 示例

推荐做法：

- 网关新增一个只支持 Responses 的模型族时，先用不带 Key 的探测确认格式支持，再更新 `responsesOnlyModelPrefixes` 和对应测试。
- 网关要求新的身份请求头时，扩展厂商配置，由 `buildProviderSessionHeaders` 生成，再交给传输层统一附加。
- 排查请求头问题时，用模拟的 `fetch` 截获最终请求，以实际发出的请求头为准。

不推荐或禁止做法：

- 为了“匹配原生端点”，把 OpenCode Go 的 MiniMax 或 Qwen 模型路由改成 Anthropic 协议。网关已经做了格式转换，额外的协议分支只会让模型路由和连接测试分叉。
- 在 `defaultHeaders` 里写 User-Agent，或用 `Authorization: null` 去掉鉴权头，然后认为已经生效。
- 在业务服务里为某个网关模型临时拼请求头、换 baseURL 或加重试。

## 失败模式

- 连接测试报 `not supported for format oa-compat`：这个模型只支持 Responses。确认它是否属于新出现的模型族，然后补充 `responsesOnlyModelPrefixes`；不要在调用侧换协议或加重试。
- 网关拒绝请求或提示异常流量：先用模拟的 `fetch` 检查实际的 User-Agent 和会话头，确认没有回落成 `langchainjs-openai`，再检查网关控制台的用量额度。
- 结构化任务频繁触发 JSON 修复：检查 `structuredProfile.family` 是否为 `default` 或兼容 Qwen 分支，确认网关厂商没有被误判为官方端点的能力档案。
- 自定义厂商选了 `x-api-key` 或无需鉴权，网关仍报鉴权冲突：检查最终请求是否还带着 `Authorization`。`defaultHeaders` 中值为 `null` 的头会被 LangChain 丢弃，去掉鉴权头必须在传输层完成。

## 相关模块

- `shared/types/llm.ts`
- `server/src/llm/providers.ts`
- `server/src/llm/factory.ts`
- `server/src/llm/anthropicClient.ts`
- `server/src/llm/modelCatalog.ts`
- `server/src/llm/capabilities.ts`
- `server/src/llm/structuredOutput.ts`
- `server/src/llm/reasoning.ts`
- `server/tests/llmProviders.test.js`

## 来源文档

- [当前模型选择与厂商默认模型边界](./model-selection.md)
- [厂商缓存请求适配](../../../server/src/platform/llm/cache/README.md)
- [OpenCode Go 官方文档](https://opencode.ai/docs/go/)
