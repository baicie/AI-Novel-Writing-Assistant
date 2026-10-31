import test from "node:test";
import assert from "node:assert/strict";

import {
  DESKTOP_MODEL_PRESETS,
  presetById,
  presetForEndpoint,
  requiresLocalhostPermission,
  validateModelForm,
} from "../src/pages/settings/desktopModelPresets.ts";

test("厂商预设包含常见选择与自定义兜底", () => {
  const ids = DESKTOP_MODEL_PRESETS.map((item) => item.id);
  assert.deepEqual(ids, ["deepseek", "openai", "moonshot", "custom"]);
  assert.equal(presetById("deepseek")?.model, "deepseek-chat");
  assert.equal(presetById("nope"), undefined);
});

test("已保存的地址能反推厂商，匹配不到归到自定义", () => {
  assert.equal(presetForEndpoint("https://api.deepseek.com/v1").id, "deepseek");
  assert.equal(presetForEndpoint("https://api.openai.com/v1/").id, "openai");
  assert.equal(presetForEndpoint("https://api.moonshot.cn/v1").id, "moonshot");
  assert.equal(presetForEndpoint("http://127.0.0.1:8080/v1").id, "custom");
  assert.equal(presetForEndpoint("").id, "custom");
});

test("明文地址需要显式允许（含自建网关）", () => {
  assert.equal(requiresLocalhostPermission("http://localhost:1234/v1"), true);
  assert.equal(requiresLocalhostPermission("http://127.0.0.1:8080/v1"), true);
  assert.equal(requiresLocalhostPermission("https://api.deepseek.com/v1"), false);
  assert.equal(requiresLocalhostPermission("http://example.com/v1"), true, "公网明文网关也要显式允许");
});

test("表单校验：地址、模型、密钥都必填", () => {
  const base = { endpoint: "https://api.deepseek.com/v1", model: "deepseek-chat", apiKey: "sk-1", hasSavedKey: false };
  assert.equal(validateModelForm(base), null);
  assert.equal(validateModelForm({ ...base, endpoint: "" }), "请填写接口地址");
  assert.equal(validateModelForm({ ...base, endpoint: "api.deepseek.com" }), "接口地址需要以 http:// 或 https:// 开头");
  assert.equal(validateModelForm({ ...base, model: "  " }), "请填写模型名称");
  assert.equal(validateModelForm({ ...base, apiKey: "" }), "请填写 API Key");
  assert.equal(validateModelForm({ ...base, apiKey: "", hasSavedKey: true }), null);
});
