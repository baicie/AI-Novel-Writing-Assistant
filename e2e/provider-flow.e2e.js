// 原生业务链路冒烟：真实桌面进程里通过 IPC 走完
// 建来源 → 取模型（本地 mock 服务）→ 确认候选 → 绑定 → 解析路由。
// 全程不联网、不调用付费模型。
import http from "node:http";

const invoke = (command, args) =>
  browser.execute(
    (cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload),
    command,
    args,
  );

describe("模型来源业务链路", () => {
  let server;
  let baseUrl;

  before(async () => {
    server = http.createServer((req, res) => {
      if (req.method === "GET" && req.url.endsWith("/models")) {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ data: [{ id: "e2e-model-a" }, { id: "e2e-model-b" }] }));
        return;
      }
      res.writeHead(404).end();
    });
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    baseUrl = `http://127.0.0.1:${server.address().port}/v1`;

    // 每次运行前清掉上次残留的来源，保证用例可重复。
    await invoke("delete_provider_profile", { providerId: "e2e-gw", force: true }).catch(() => {});
  });

  after(() => server?.close());

  it("保存来源后能取回，并带回版本号", async () => {
    const created = await invoke("upsert_provider_profile", {
      request: {
        providerId: "e2e-gw",
        displayName: "E2E 网关",
        protocol: "openai-compatible",
        baseUrl,
        allowLocalhostHttp: true,
        credential: { kind: "replace", apiKey: "sk-e2e" },
        expectedRevision: null,
      },
    });
    expect(created.id).toBe("e2e-gw");
    // 系统钥匙库在无头 E2E 会话里可能拒绝读取（macOS Keychain 限制），
    // 这里只断言字段存在且为布尔，凭据本身的安全性由单元测试与序列化断言覆盖。
    expect(typeof created.credentialPresent).toBe("boolean");
    expect(created.revision).toBe(1);

    const listed = await invoke("list_provider_profiles");
    const found = listed.find((item) => item.id === "e2e-gw");
    expect(found).toBeTruthy();
    expect(JSON.stringify(found)).not.toContain("sk-e2e");
  });

  it("草稿发现只返回候选，确认后才落库", async () => {
    const preview = await invoke("discover_provider_models", {
      request: {
        providerId: "e2e-gw",
        protocol: "openai-compatible",
        baseUrl,
        apiKey: "sk-e2e",
        allowLocalhostHttp: true,
      },
    });
    expect(preview.models.length).toBe(2);
    expect(preview.previewToken).toBeTruthy();

    const before = await invoke("list_provider_models", { providerId: "e2e-gw" });
    expect(before.length).toBe(0);

    const applied = await invoke("apply_provider_discovery", {
      request: {
        providerId: "e2e-gw",
        previewToken: preview.previewToken,
        selectedModelIds: ["e2e-model-a"],
        expectedRevision: 1,
      },
    });
    expect(applied.added).toBe(1);

    const after = await invoke("list_provider_models", { providerId: "e2e-gw" });
    expect(after.map((model) => model.modelId)).toEqual(["e2e-model-a"]);
  });

  it("绑定后能解析出路由，且不含密钥", async () => {
    await invoke("upsert_model_binding", {
      request: {
        taskType: "writer",
        providerId: "e2e-gw",
        modelId: "e2e-model-a",
        reasoningEffort: null,
        expectedRevision: null,
      },
    });
    const route = await invoke("get_effective_model_route", { taskType: "writer" });
    expect(route.providerId).toBe("e2e-gw");
    expect(route.modelId).toBe("e2e-model-a");
    expect(JSON.stringify(route)).not.toContain("sk-e2e");

    const status = await invoke("check_run_route", {
      providerId: "e2e-gw",
      modelId: "e2e-model-a",
    });
    expect(status.kind).toBe("resolved");
  });
});
