const test = require("node:test");
const assert = require("node:assert/strict");
const { z } = require("zod");
const {
  PROVIDERS,
  SUPPORTED_PROVIDERS,
  buildProviderSessionHeaders,
  isProviderModelCallable,
} = require("../dist/llm/providers.js");
const {
  getJsonCapability,
  getModelParameterCompatibility,
  resolveModelTemperature,
} = require("../dist/llm/capabilities.js");
const {
  buildOpenAICompatibleDefaultHeaders,
  createLLMFromResolvedOptions,
  resolveLLMClientOptions,
  setProviderSecretCache,
} = require("../dist/llm/factory.js");
const { createAnthropicLLM } = require("../dist/llm/anthropicClient.js");
const { refreshProviderModels } = require("../dist/llm/modelCatalog.js");
const { HumanMessage } = require("@langchain/core/messages");

test("custom provider auth mode replaces the SDK bearer header when requested", () => {
  assert.equal(buildOpenAICompatibleDefaultHeaders("bearer", "secret"), undefined);
  assert.deepEqual(buildOpenAICompatibleDefaultHeaders("x-api-key", "secret"), {
    Authorization: null,
    "x-api-key": "secret",
  });
  assert.deepEqual(buildOpenAICompatibleDefaultHeaders("none", "secret"), {
    Authorization: null,
  });
});
const {
  classifyStructuredOutputFailure,
  resolveStructuredOutputProfile,
  selectStructuredOutputStrategy,
} = require("../dist/llm/structuredOutput.js");

test("supported providers include kimi, minimax, glm, qwen, gemini and ollama", () => {
  for (const provider of ["kimi", "minimax", "glm", "qwen", "gemini", "ollama"]) {
    assert.ok(SUPPORTED_PROVIDERS.includes(provider), `${provider} should be available`);
  }
});

test("new provider defaults are present in their model fallback lists", () => {
  for (const provider of ["kimi", "minimax", "glm", "qwen", "gemini", "ollama"]) {
    assert.ok(
      PROVIDERS[provider].models.includes(PROVIDERS[provider].defaultModel),
      `${provider} default model should exist in fallback models`,
    );
  }
});

test("opencode go is a built-in gateway whose fallback models are all callable", () => {
  const config = PROVIDERS["opencode-go"];
  assert.ok(SUPPORTED_PROVIDERS.includes("opencode-go"));
  assert.equal(config.baseURL, "https://opencode.ai/zen/go/v1");
  assert.equal(config.envKey, "OPENCODE_GO_API_KEY");
  assert.equal(config.models[0], config.defaultModel);
  assert.ok(config.models.every((model) => isProviderModelCallable("opencode-go", model)));

  assert.equal(isProviderModelCallable("opencode-go", "grok-4.7"), false);
  assert.equal(isProviderModelCallable("opencode-go", " Grok-4.6 "), false);
  assert.equal(isProviderModelCallable("opencode-go", "minimax-m3"), true);
  assert.equal(isProviderModelCallable("grok", "grok-4"), true);
  assert.equal(isProviderModelCallable("custom_gateway", "grok-4.7"), true);
});

test("opencode go model catalog hides models served only over the Responses API", async () => {
  const originalFetch = global.fetch;
  global.fetch = async () => new Response(JSON.stringify({
    object: "list",
    data: [{ id: "grok-4.7" }, { id: "deepseek-v4.1-flash" }, { id: "grok-4.6" }, { id: "minimax-m3" }],
  }), { status: 200, headers: { "content-type": "application/json" } });
  try {
    assert.deepEqual(await refreshProviderModels("opencode-go", "test-key"), ["deepseek-v4.1-flash", "minimax-m3"]);
  } finally {
    global.fetch = originalFetch;
  }
});

test("opencode go requests identify the app and keep one opaque session per novel", () => {
  const novelA = buildProviderSessionHeaders("opencode-go", "novel-a");
  assert.match(novelA["User-Agent"], /^BizNovelStudio\b/);
  assert.match(novelA["x-opencode-session"], /^[0-9a-f]{32}$/);
  assert.deepEqual(buildProviderSessionHeaders("opencode-go", "novel-a"), novelA);
  assert.notEqual(buildProviderSessionHeaders("opencode-go", "novel-b")["x-opencode-session"], novelA["x-opencode-session"]);

  const unscoped = buildProviderSessionHeaders("opencode-go");
  assert.ok(unscoped["x-opencode-session"]);
  assert.equal(buildProviderSessionHeaders("opencode-go", "  ")["x-opencode-session"], unscoped["x-opencode-session"]);
  assert.notEqual(unscoped["x-opencode-session"], novelA["x-opencode-session"]);

  assert.equal(buildProviderSessionHeaders("deepseek", "novel-a"), undefined);
  assert.equal(buildProviderSessionHeaders("custom_gateway", "novel-a"), undefined);
});

test("opencode go clients carry the session headers on both request protocols", async () => {
  const expected = buildProviderSessionHeaders("opencode-go", "novel-a");
  const resolved = {
    provider: "opencode-go",
    providerName: "OpenCode Go",
    model: "glm-5.3",
    temperature: 0.7,
    apiKey: "test-key",
    baseURL: "https://opencode.ai/zen/go/v1",
    authMode: "bearer",
    concurrencyLimit: 0,
    requestIntervalMs: 0,
    reasoningEnabled: true,
    reasoningEffort: null,
    includeRawResponse: false,
    requestProtocol: "openai_compatible",
    executionMode: "plain",
    structuredProfile: null,
    structuredStrategy: null,
    reasoningForcedOff: false,
    promptMeta: { novelId: "novel-a" },
  };
  const chat = createLLMFromResolvedOptions(resolved);
  const deepseek = createLLMFromResolvedOptions({ ...resolved, provider: "deepseek", baseURL: "https://api.deepseek.com/v1" });
  assert.equal(deepseek.clientConfig.fetch, undefined);

  const originalFetch = global.fetch;
  let chatHeaders;
  let anthropicHeaders;
  global.fetch = async (_url, init) => {
    if (init.headers instanceof Headers) {
      chatHeaders = init.headers;
      return new Response("{}", { status: 200 });
    }
    anthropicHeaders = init.headers;
    return new Response(JSON.stringify({ content: [{ type: "text", text: "ok" }] }), { status: 200 });
  };
  try {
    await chat.clientConfig.fetch("https://opencode.ai/zen/go/v1/chat/completions", {
      method: "POST",
      headers: { "user-agent": "langchainjs-openai/1.0.0", authorization: "Bearer test-key" },
    });
    await createAnthropicLLM({
      apiKey: "test-key",
      model: "minimax-m3",
      baseURL: resolved.baseURL,
      temperature: 0.7,
      headers: expected,
    }).invoke([new HumanMessage("hi")]);
  } finally {
    global.fetch = originalFetch;
  }
  assert.equal(chatHeaders.get("user-agent"), expected["User-Agent"]);
  assert.equal(chatHeaders.get("x-opencode-session"), expected["x-opencode-session"]);
  assert.equal(chatHeaders.get("authorization"), "Bearer test-key");
  assert.equal(anthropicHeaders["User-Agent"], expected["User-Agent"]);
  assert.equal(anthropicHeaders["x-opencode-session"], expected["x-opencode-session"]);
  assert.equal(anthropicHeaders["x-api-key"], "test-key");
});

test("opencode go uses prompt json and leaves vendor thinking switches to the gateway", async () => {
  assert.deepEqual(
    getJsonCapability("opencode-go", "deepseek-v4.1-flash", "https://opencode.ai/zen/go/v1"),
    { supportsJsonObject: false, supportsJsonSchema: false },
  );
  setProviderSecretCache("opencode-go", { key: "test-key", reasoningEnabled: true });
  try {
    const deepseekPro = await resolveLLMClientOptions("opencode-go", {
      model: "deepseek-v4-pro",
      executionMode: "structured",
      structuredStrategy: "prompt_json",
      maxTokens: 20000,
    });
    assert.equal(deepseekPro.baseURL, "https://opencode.ai/zen/go/v1");
    assert.equal(deepseekPro.requestProtocol, "openai_compatible");
    assert.equal(deepseekPro.structuredProfile?.family, "default");
    assert.equal(deepseekPro.structuredProfile?.preferredStructuredStrategy, "prompt_json");
    assert.equal(deepseekPro.maxTokens, 8192);
    assert.equal(deepseekPro.modelKwargs, undefined);

    const kimiK3 = await resolveLLMClientOptions("opencode-go", { model: "kimi-k3", temperature: 0.4 });
    assert.equal(kimiK3.temperature, 1);
  } finally {
    setProviderSecretCache("opencode-go", null);
  }
});

test("kimi thinking models do not enable forced json mode", () => {
  const stableCapability = getJsonCapability("kimi", "moonshot-v1-32k");
  assert.equal(stableCapability.supportsJsonObject, true);

  const thinkingCapability = getJsonCapability("kimi", "kimi-k2-thinking-turbo");
  assert.equal(thinkingCapability.supportsJsonObject, false);
});

test("kimi k2 and k3 models force temperature 1 while moonshot models keep requested temperature", () => {
  assert.deepEqual(
    getModelParameterCompatibility("kimi", "kimi-k2-turbo-preview"),
    { fixedTemperature: 1 },
  );
  assert.equal(resolveModelTemperature("kimi", "kimi-k2-turbo-preview", 0.4), 1);
  assert.deepEqual(
    getModelParameterCompatibility("kimi", "kimi-k3"),
    { fixedTemperature: 1 },
  );
  assert.equal(resolveModelTemperature("kimi", "kimi-k3", 0.5), 1);
  assert.equal(resolveModelTemperature("kimi", "moonshot-v1-32k", 0.4), 0.4);
  assert.equal(resolveModelTemperature("deepseek", "deepseek-chat", undefined), 0.7);
});

test("ollama does not advertise forced json mode", () => {
  const capability = getJsonCapability("ollama", "llama3.2");
  assert.equal(capability.supportsJsonObject, false);
  assert.equal(capability.supportsJsonSchema, false);
});

test("minimax clamps temperature into supported range", () => {
  assert.deepEqual(
    getModelParameterCompatibility("minimax", "MiniMax-M2.7"),
    { minimumTemperature: 0.01, maximumTemperature: 1 },
  );
  assert.equal(resolveModelTemperature("minimax", "MiniMax-M2.7", 0), 0.01);
  assert.equal(resolveModelTemperature("minimax", "MiniMax-M2.7", 1.5), 1);
  assert.equal(resolveModelTemperature("minimax", "MiniMax-M2.7", 0.4), 0.4);
});

test("structured output profiles distinguish official, ModelScope Qwen and unknown custom endpoints", () => {
  const schema = z.object({ value: z.string() });

  const openaiProfile = resolveStructuredOutputProfile({
    provider: "openai",
    model: "gpt-5-mini",
    baseURL: "https://api.openai.com/v1",
    executionMode: "structured",
  });
  assert.equal(openaiProfile.family, "openai");
  assert.equal(openaiProfile.nativeJsonSchema, true);
  assert.equal(selectStructuredOutputStrategy(openaiProfile, schema), "json_schema");

  const glmProfile = resolveStructuredOutputProfile({
    provider: "glm",
    model: "glm-4.5-air",
    baseURL: "https://open.bigmodel.cn/api/paas/v4",
    executionMode: "structured",
  });
  assert.equal(glmProfile.family, "glm");
  assert.equal(glmProfile.nativeJsonObject, true);
  assert.equal(glmProfile.requiresNonThinkingForStructured, true);
  assert.equal(glmProfile.supportsReasoningToggle, true);
  assert.equal(selectStructuredOutputStrategy(glmProfile, schema), "json_object");

  const glmBehindProxyProfile = resolveStructuredOutputProfile({
    provider: "openai",
    model: "glm-5",
    baseURL: "https://aiproxy.example.com/v1",
    executionMode: "structured",
  });
  assert.equal(glmBehindProxyProfile.family, "default");
  assert.equal(glmBehindProxyProfile.nativeJsonSchema, false);
  assert.equal(glmBehindProxyProfile.nativeJsonObject, false);
  assert.equal(selectStructuredOutputStrategy(glmBehindProxyProfile, schema), "prompt_json");

  const kimiBehindProxyProfile = resolveStructuredOutputProfile({
    provider: "openai",
    model: "kimi-k2.5",
    baseURL: "https://aiproxy.example.com/v1",
    executionMode: "structured",
  });
  assert.equal(kimiBehindProxyProfile.family, "default");
  assert.equal(kimiBehindProxyProfile.nativeJsonSchema, false);
  assert.equal(kimiBehindProxyProfile.nativeJsonObject, false);
  assert.equal(selectStructuredOutputStrategy(kimiBehindProxyProfile, schema), "prompt_json");

  const minimaxBehindProxyProfile = resolveStructuredOutputProfile({
    provider: "openai",
    model: "MiniMax-M2.5",
    baseURL: "https://aiproxy.example.com/v1",
    executionMode: "structured",
  });
  assert.equal(minimaxBehindProxyProfile.family, "default");
  assert.equal(minimaxBehindProxyProfile.nativeJsonSchema, false);
  assert.equal(selectStructuredOutputStrategy(minimaxBehindProxyProfile, schema), "prompt_json");

  const qwenBehindProxyProfile = resolveStructuredOutputProfile({
    provider: "openai",
    model: "qwen3.6-plus",
    baseURL: "https://aiproxy.example.com/v1",
    executionMode: "structured",
  });
  assert.equal(qwenBehindProxyProfile.family, "custom_openai_compatible_qwen");
  assert.equal(qwenBehindProxyProfile.nativeJsonSchema, false);
  assert.equal(selectStructuredOutputStrategy(qwenBehindProxyProfile, schema), "prompt_json");

  const deepseekBehindProxyProfile = resolveStructuredOutputProfile({
    provider: "openai",
    model: "deepseek-chat",
    baseURL: "https://aiproxy.example.com/v1",
    executionMode: "structured",
  });
  assert.equal(deepseekBehindProxyProfile.family, "default");
  assert.equal(deepseekBehindProxyProfile.nativeJsonSchema, false);
  assert.equal(deepseekBehindProxyProfile.nativeJsonObject, false);
  assert.equal(selectStructuredOutputStrategy(deepseekBehindProxyProfile, schema), "prompt_json");

  const deepseekFlashProfile = resolveStructuredOutputProfile({
    provider: "deepseek",
    model: "deepseek-flash",
    baseURL: "https://api.deepseek.com/v1",
    executionMode: "structured",
  });
  assert.equal(deepseekFlashProfile.requiresNonThinkingForStructured, true);
  assert.equal(deepseekFlashProfile.supportsReasoningToggle, true);

  const deepseekLegacyFlashProfile = resolveStructuredOutputProfile({
    provider: "deepseek",
    model: "deepseek-v4-flash",
    baseURL: "https://api.deepseek.com/v1",
    executionMode: "structured",
  });
  assert.equal(deepseekLegacyFlashProfile.requiresNonThinkingForStructured, true);
  assert.equal(deepseekLegacyFlashProfile.supportsReasoningToggle, true);

  const kimiProfile = resolveStructuredOutputProfile({
    provider: "kimi",
    model: "kimi-k2.5",
    baseURL: "https://api.moonshot.cn/v1",
    executionMode: "structured",
  });
  assert.equal(kimiProfile.family, "kimi");
  assert.equal(kimiProfile.nativeJsonObject, true);
  assert.equal(selectStructuredOutputStrategy(kimiProfile, schema), "json_object");

  const kimiThinkingProfile = resolveStructuredOutputProfile({
    provider: "kimi",
    model: "kimi-k2-thinking-turbo",
    baseURL: "https://api.moonshot.cn/v1",
    executionMode: "structured",
  });
  assert.equal(kimiThinkingProfile.family, "kimi");
  assert.equal(kimiThinkingProfile.nativeJsonObject, false);
  assert.equal(selectStructuredOutputStrategy(kimiThinkingProfile, schema), "prompt_json");

  const modelscopeProfile = resolveStructuredOutputProfile({
    provider: "custom_modelscope",
    model: "Qwen/Qwen3.5-397B-A17B",
    baseURL: "https://api-inference.modelscope.cn/v1",
    executionMode: "structured",
  });
  assert.equal(modelscopeProfile.family, "modelscope_qwen");
  assert.equal(modelscopeProfile.requiresNonThinkingForStructured, true);
  assert.equal(modelscopeProfile.supportsReasoningToggle, true);
  assert.equal(selectStructuredOutputStrategy(modelscopeProfile, schema), "prompt_json");
  assert.deepEqual(
    getJsonCapability("custom_modelscope", "Qwen/Qwen3.5-397B-A17B", "https://api-inference.modelscope.cn/v1"),
    { supportsJsonSchema: false, supportsJsonObject: false },
  );

  const qwenMixedProfile = resolveStructuredOutputProfile({
    provider: "qwen",
    model: "qwen3.6-plus",
    baseURL: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    executionMode: "structured",
  });
  assert.equal(qwenMixedProfile.family, "dashscope_qwen");
  assert.equal(qwenMixedProfile.nativeJsonObject, true);
  assert.equal(qwenMixedProfile.requiresNonThinkingForStructured, true);
  assert.equal(qwenMixedProfile.supportsReasoningToggle, true);
  assert.equal(qwenMixedProfile.omitMaxTokensForNativeStructured, true);
  assert.equal(selectStructuredOutputStrategy(qwenMixedProfile, schema), "json_object");

  const qwenThinkingProfile = resolveStructuredOutputProfile({
    provider: "qwen",
    model: "qwen3-235b-a22b-thinking-2507",
    baseURL: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    executionMode: "structured",
  });
  assert.equal(qwenThinkingProfile.family, "dashscope_qwen");
  assert.equal(qwenThinkingProfile.nativeJsonObject, false);
  assert.equal(qwenThinkingProfile.requiresNonThinkingForStructured, false);
  assert.equal(qwenThinkingProfile.supportsReasoningToggle, false);
  assert.equal(qwenThinkingProfile.omitMaxTokensForNativeStructured, false);
  assert.equal(selectStructuredOutputStrategy(qwenThinkingProfile, schema), "prompt_json");

  const customProfile = resolveStructuredOutputProfile({
    provider: "custom_gateway",
    model: "gpt-4o-mini",
    baseURL: "https://llm.example.com/v1",
    executionMode: "structured",
  });
  assert.equal(customProfile.family, "custom_openai_compatible");
  assert.equal(customProfile.nativeJsonObject, false);
  assert.equal(customProfile.preferredStructuredStrategy, "prompt_json");

  const customDeepSeekProfile = resolveStructuredOutputProfile({
    provider: "custom_ooioo",
    model: "deepseek-v4-flash-0731-fast",
    baseURL: "https://ooioo.work/v1",
    executionMode: "structured",
  });
  assert.equal(customDeepSeekProfile.family, "custom_openai_compatible");
  assert.equal(customDeepSeekProfile.nativeJsonObject, false);
  assert.equal(selectStructuredOutputStrategy(customDeepSeekProfile, schema), "prompt_json");

  const directDeepSeekThroughCustomProvider = resolveStructuredOutputProfile({
    provider: "custom_ooioo",
    model: "deepseek-v4-flash",
    baseURL: "https://api.deepseek.com/v1",
    executionMode: "structured",
  });
  assert.equal(directDeepSeekThroughCustomProvider.family, "deepseek");
  assert.equal(selectStructuredOutputStrategy(directDeepSeekThroughCustomProvider, schema), "json_object");
});

test("resolveLLMClientOptions applies structured reasoning and token guardrails", async () => {
  setProviderSecretCache("custom_modelscope", {
    key: "test-key",
    model: "Qwen/Qwen3.5-397B-A17B",
    baseURL: "https://api-inference.modelscope.cn/v1",
    displayName: "ModelScope Qwen",
    reasoningEnabled: true,
  });
  setProviderSecretCache("qwen", {
    key: "test-key",
    reasoningEnabled: true,
  });
  setProviderSecretCache("openai", {
    key: "test-key",
    reasoningEnabled: true,
  });
  setProviderSecretCache("deepseek", {
    key: "test-key",
    reasoningEnabled: true,
  });
  setProviderSecretCache("glm", {
    key: "test-key",
    reasoningEnabled: true,
  });
  setProviderSecretCache("kimi", {
    key: "test-key",
  });
  setProviderSecretCache("custom_kimik3", {
    key: "test-key",
    model: "kimi-k3",
    baseURL: "https://gateway.example.com/v1",
  });

  try {
    const modelscope = await resolveLLMClientOptions("custom_modelscope", {
      executionMode: "structured",
      structuredStrategy: "prompt_json",
      maxTokens: 20000,
    });
    assert.equal(modelscope.structuredProfile?.family, "modelscope_qwen");
    assert.equal(modelscope.reasoningEnabled, false);
    assert.equal(modelscope.reasoningForcedOff, true);
    assert.equal(modelscope.modelKwargs?.enable_thinking, false);
    assert.equal(modelscope.maxTokens, 8192);
    assert.equal(modelscope.requestProtocol, "openai_compatible");

    const qwen = await resolveLLMClientOptions("qwen", {
      apiKey: "test-key",
      model: "qwen3.5-397b-a17b",
      baseURL: "https://dashscope.aliyuncs.com/compatible-mode/v1",
      executionMode: "structured",
      structuredStrategy: "json_object",
      maxTokens: 20000,
    });
    assert.equal(qwen.structuredProfile?.family, "dashscope_qwen");
    assert.equal(qwen.reasoningEnabled, false);
    assert.equal(qwen.reasoningForcedOff, true);
    assert.equal(qwen.modelKwargs?.enable_thinking, false);
    assert.equal(qwen.maxTokens, undefined);
    assert.equal(qwen.requestProtocol, "openai_compatible");

    const glm = await resolveLLMClientOptions("glm", {
      model: "glm-4.6v",
      executionMode: "structured",
      structuredStrategy: "json_object",
    });
    assert.equal(glm.structuredProfile?.family, "glm");
    assert.equal(glm.reasoningEnabled, false);
    assert.equal(glm.reasoningForcedOff, true);
    assert.deepEqual(glm.modelKwargs?.thinking, { type: "disabled" });

    const kimiK3 = await resolveLLMClientOptions("kimi", {
      model: "kimi-k3",
      temperature: 0.5,
    });
    assert.equal(kimiK3.temperature, 1);

    const proxiedKimiK3 = await resolveLLMClientOptions("custom_kimik3", {
      temperature: 0.1,
    });
    assert.equal(proxiedKimiK3.temperature, 1);

    const qwenThinking = await resolveLLMClientOptions("qwen", {
      apiKey: "test-key",
      model: "qwen3-235b-a22b-thinking-2507",
      baseURL: "https://dashscope.aliyuncs.com/compatible-mode/v1",
      executionMode: "structured",
      structuredStrategy: "prompt_json",
      maxTokens: 20000,
    });
    assert.equal(qwenThinking.structuredProfile?.family, "dashscope_qwen");
    assert.equal(qwenThinking.reasoningEnabled, true);
    assert.equal(qwenThinking.reasoningForcedOff, false);
    assert.equal(qwenThinking.modelKwargs?.enable_thinking, undefined);
    assert.equal(qwenThinking.maxTokens, 8192);
    assert.equal(qwenThinking.requestProtocol, "openai_compatible");

    const deepseekFlash = await resolveLLMClientOptions("deepseek", {
      model: "deepseek-flash",
      executionMode: "structured",
      structuredStrategy: "json_object",
      maxTokens: 5000,
    });
    assert.equal(deepseekFlash.structuredProfile?.family, "deepseek");
    assert.equal(deepseekFlash.reasoningEnabled, false);
    assert.equal(deepseekFlash.reasoningForcedOff, true);
    assert.deepEqual(deepseekFlash.modelKwargs?.thinking, { type: "disabled" });
    assert.equal(deepseekFlash.modelKwargs?.reasoning_effort, undefined);
    assert.equal(deepseekFlash.reasoningEffort, null);
    assert.equal(deepseekFlash.modelKwargs?.enable_thinking, undefined);

    const deepseekMax = await resolveLLMClientOptions("deepseek", {
      model: "deepseek-v4-pro",
      reasoningEffort: "max",
    });
    assert.deepEqual(deepseekMax.modelKwargs?.thinking, { type: "enabled" });
    assert.equal(deepseekMax.modelKwargs?.reasoning_effort, "max");
    assert.equal(deepseekMax.reasoningEffort, "max");

    const anthropicProtocol = await resolveLLMClientOptions("openai", {
      apiKey: "test-key",
      model: "claude-sonnet-4-5",
      baseURL: "https://aiproxy.example.com/v1",
      requestProtocol: "anthropic",
      executionMode: "structured",
      structuredStrategy: "prompt_json",
    });
    assert.equal(anthropicProtocol.requestProtocol, "anthropic");
    assert.equal(anthropicProtocol.structuredProfile?.family, "anthropic");
  } finally {
    setProviderSecretCache("custom_modelscope", null);
    setProviderSecretCache("qwen", null);
    setProviderSecretCache("openai", null);
    setProviderSecretCache("deepseek", null);
    setProviderSecretCache("glm", null);
    setProviderSecretCache("kimi", null);
    setProviderSecretCache("custom_kimik3", null);
  }
});

test("structured failure classification separates native-json, thinking and schema problems", () => {
  assert.equal(
    classifyStructuredOutputFailure({ error: new Error("response_format json_schema is not supported") }),
    "unsupported_native_json",
  );
  assert.equal(
    classifyStructuredOutputFailure({ rawContent: "<think>draft</think>{\"value\":\"ok\"}" }),
    "thinking_pollution",
  );
  assert.equal(
    classifyStructuredOutputFailure({ error: new Error("Unexpected end of JSON input") }),
    "incomplete_json",
  );
  assert.equal(
    classifyStructuredOutputFailure({ error: new Error("Expected ',' or '}' after property value") }),
    "malformed_json",
  );
  assert.equal(
    classifyStructuredOutputFailure({ error: new Error("schema validation failed") }),
    "schema_mismatch",
  );
  assert.equal(
    classifyStructuredOutputFailure({
      error: new Error("Unexpected token '<', \"<!doctype\" is not valid JSON"),
      rawContent: "<!DOCTYPE html><html><head><title>429 Too Many Requests</title></head><body>rate limit</body></html>",
    }),
    "transport_error",
  );
  assert.equal(
    classifyStructuredOutputFailure({
      error: new Error("schema validation failed"),
      rawContent: "{\"snippet\":\"<html>rendered fragment</html>\",\"value\":\"ok\"}",
    }),
    "schema_mismatch",
  );
  assert.equal(
    classifyStructuredOutputFailure({
      error: new Error("Expected ',' or '}' after property value"),
      rawContent: "{\"snippet\":\"<html>rendered fragment</html>\"",
    }),
    "malformed_json",
  );
});
