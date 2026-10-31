import test from "node:test";
import assert from "node:assert/strict";

import { createProviderClient } from "../src/platform/providerTransport.ts";

function recorder() {
  const calls = [];
  const client = createProviderClient(async (command, args) => {
    calls.push({ command, args });
    return undefined;
  });
  return { client, calls };
}

test("每个方法都打到约定的命令名", async () => {
  const { client, calls } = recorder();
  await client.listTemplates();
  await client.listProfiles();
  await client.getProfile("gw");
  await client.listModels("gw");
  await client.deleteProfile("gw");
  await client.listBindings();
  assert.deepEqual(
    calls.map((call) => call.command),
    [
      "list_provider_templates",
      "list_provider_profiles",
      "get_provider_profile",
      "list_provider_models",
      "delete_provider_profile",
      "list_model_bindings",
    ],
  );
});

test("upsertProfile 透传 camelCase 请求体", async () => {
  const { client, calls } = recorder();
  await client.upsertProfile({
    providerId: "gw",
    protocol: "openai-compatible",
    baseUrl: "https://gw.example.com/v1",
    credential: { kind: "replace", apiKey: "sk-1" },
    expectedRevision: 3,
  });
  assert.equal(calls[0].command, "upsert_provider_profile");
  const request = (calls[0].args).request;
  assert.equal(request.providerId, "gw");
  assert.equal(request.expectedRevision, 3);
  assert.deepEqual(request.credential, { kind: "replace", apiKey: "sk-1" });
  assert.ok(!("provider_id" in request), "不能混用 snake_case");
});

test("keep / clear 不带 apiKey", async () => {
  const { client, calls } = recorder();
  await client.upsertProfile({
    providerId: "gw",
    protocol: "openai-compatible",
    baseUrl: "https://gw.example.com/v1",
    credential: { kind: "keep" },
  });
  await client.upsertProfile({
    providerId: "gw",
    protocol: "openai-compatible",
    baseUrl: "https://gw.example.com/v1",
    credential: { kind: "clear" },
  });
  const first = (calls[0].args).request;
  const second = (calls[1].args).request;
  assert.deepEqual(Object.keys(first.credential), ["kind"]);
  assert.deepEqual(Object.keys(second.credential), ["kind"]);
});

test("discover 支持未保存草稿，apply 必带 previewToken", async () => {
  const { client, calls } = recorder();
  await client.discover({
    protocol: "ollama",
    baseUrl: "http://127.0.0.1:11434",
    allowLocalhostHttp: true,
  });
  await client.applyDiscovery({
    providerId: "gw",
    previewToken: "pv-1",
    selectedModelIds: ["a", "b"],
    expectedRevision: 2,
  });
  assert.equal(calls[0].command, "discover_provider_models");
  const draft = (calls[0].args).request;
  assert.ok(!("providerId" in draft) || draft.providerId == null, "草稿可以没有 providerId");
  const apply = (calls[1].args).request;
  assert.equal(apply.previewToken, "pv-1");
  assert.deepEqual(apply.selectedModelIds, ["a", "b"]);
});

test("deleteModel 与 upsertBinding 的请求形状稳定", async () => {
  const { client, calls } = recorder();
  await client.deleteModel("gw", "a", 5);
  await client.upsertBinding({
    taskType: "chapter",
    providerId: "gw",
    modelId: "a",
    reasoningEffort: "high",
  });
  assert.deepEqual(calls[0].args, { providerId: "gw", modelId: "a", expectedRevision: 5 });
  const binding = (calls[1].args).request;
  assert.equal(binding.taskType, "chapter");
  assert.equal(binding.reasoningEffort, "high");
});