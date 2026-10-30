import test from "node:test";
import assert from "node:assert/strict";

import {
  BINDING_TASK_TYPES,
  applyTemplate,
  canBindForGeneration,
  describeProviderError,
  draftFromProvider,
  emptyDraft,
  isDirty,
  mergeCandidates,
  toggleSelection,
  validateDraft,
} from "../src/pages/settings/providerRegistryModel.ts";

const provider = {
  id: "gw",
  displayName: "网关",
  protocol: "openai-compatible",
  baseUrl: "https://gw.example.com/v1",
  allowLocalhostHttp: false,
  isBuiltin: false,
  sortOrder: 0,
  revision: 3,
  credentialPresent: true,
};

test("任务种类与工作流定义一致", () => {
  const values = BINDING_TASK_TYPES.map((item) => item.value);
  assert.deepEqual(values, [
    "planner",
    "writer",
    "review",
    "light_review",
    "critical_review",
    "repair",
    "replan",
    "state_resolution",
    "summary",
    "fact_extraction",
    "chat",
  ]);
});

test("编辑已有配置默认保留密钥且带回版本号", () => {
  const draft = draftFromProvider(provider);
  assert.equal(draft.credentialKind, "keep");
  assert.equal(draft.expectedRevision, 3);
  assert.equal(draft.apiKey, "");
  assert.equal(isDirty(draft, draft), false);
});

test("模板只填地址协议，不动用户密钥", () => {
  const draft = { ...emptyDraft(), apiKey: "sk-user", providerId: "" };
  const next = applyTemplate(
    {
      id: "ollama",
      displayName: "Ollama（本地）",
      protocol: "ollama",
      baseUrl: "http://127.0.0.1:11434",
      requiresApiKey: false,
      local: true,
      models: [],
    },
    draft,
  );
  assert.equal(next.protocol, "ollama");
  assert.equal(next.baseUrl, "http://127.0.0.1:11434");
  assert.equal(next.apiKey, "sk-user");
  assert.equal(next.allowLocalhostHttp, true, "本地模板要自动允许本地地址");
  assert.equal(next.providerId, "ollama");
});

test("校验：标识、地址、密钥三态互斥", () => {
  const base = { ...emptyDraft(), providerId: "gw", baseUrl: "https://gw/v1", apiKey: "sk-1" };
  assert.equal(validateDraft(base, false).ok, true);
  assert.equal(validateDraft({ ...base, providerId: "GW" }, false).ok, false);
  assert.equal(validateDraft({ ...base, baseUrl: "gw/v1" }, false).ok, false);
  assert.equal(validateDraft({ ...base, apiKey: "" }, false).ok, false, "替换必须带密钥");
  assert.equal(
    validateDraft({ ...base, credentialKind: "keep" }, true).ok,
    false,
    "keep 不能带密钥",
  );
  assert.equal(
    validateDraft({ ...base, credentialKind: "keep", apiKey: "" }, false).ok,
    false,
    "没有已保存密钥时不能选 keep",
  );
  assert.equal(validateDraft({ ...base, credentialKind: "clear", apiKey: "" }, true).ok, true);
});

test("候选合并：去重、已保存默认勾选", () => {
  const candidates = [
    { providerId: "gw", modelId: "a", displayName: "A", capabilities: {}, source: "discovered", overridden: [] },
    { providerId: "gw", modelId: "a", displayName: "A", capabilities: {}, source: "discovered", overridden: [] },
    { providerId: "gw", modelId: "b", displayName: null, capabilities: {}, source: "discovered", overridden: [] },
  ];
  const saved = [
    { providerId: "gw", modelId: "a", displayName: "A", capabilities: {}, source: "user", overridden: [] },
  ];
  const rows = mergeCandidates(candidates, saved, []);
  assert.equal(rows.length, 2);
  assert.equal(rows[0].selected, true, "已保存的默认勾选");
  assert.equal(rows[0].alreadySaved, true);
  assert.equal(rows[1].displayName, "b");
});

test("勾选切换是幂等的两边操作", () => {
  assert.deepEqual(toggleSelection([], "a"), ["a"]);
  assert.deepEqual(toggleSelection(["a"], "a"), []);
  assert.deepEqual(toggleSelection(["a"], "b"), ["a", "b"]);
});

test("错误码翻译成用户动作", () => {
  assert.match(describeProviderError({ code: "CONFLICT", message: "x", retryable: true }), /刷新/);
  assert.match(describeProviderError({ code: "IN_USE", message: "x", retryable: false }), /任务绑定/);
  assert.match(describeProviderError({ code: "CREDENTIAL_MISSING", message: "", retryable: false }), /API Key/);
});

test("只有具备生成适配器的协议可绑定创作任务", () => {
  assert.equal(canBindForGeneration("openai-compatible"), true);
  assert.equal(canBindForGeneration("anthropic-messages"), false);
  assert.equal(canBindForGeneration("ollama"), false);
});
