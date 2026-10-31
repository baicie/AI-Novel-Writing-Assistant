// F：只读协议（anthropic-messages）可以保存与发现，但不允许绑定到生成任务。
// 这里刻意绕过 UI，直接调用 Tauri IPC，验证后端仍然拒绝。
describe("只读协议的绑定时后端拒绝", () => {
  const invoke = (command, args) =>
    browser.execute((cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload), command, args);

  it("IPC 直连也拿不到生成绑定", async () => {
    await browser.pause(2500);

    const result = await browser.execute(async () => {
      const call = window.__TAURI_INTERNALS__.invoke;
      const profiles = await call("list_provider_profiles");
      const existing = (profiles || []).find((item) => item.id === "probe-readonly");
      await call("upsert_provider_profile", {
        request: {
          providerId: "probe-readonly",
          displayName: "只读协议网关",
          protocol: "anthropic-messages",
          baseUrl: "https://api.anthropic.com",
          allowLocalhostHttp: false,
          sortOrder: 1,
          isBuiltin: false,
          credential: { kind: "replace", apiKey: "sk-readonly" },
          expectedRevision: existing ? existing.revision : null,
        },
      });
      const models = await call("list_provider_models", { providerId: "probe-readonly" });
      const hasModel = (models || []).some((item) => item.modelId === "claude-readonly");
      if (!hasModel) {
        await call("upsert_provider_model", {
          request: {
            providerId: "probe-readonly",
            modelId: "claude-readonly",
            expectedRevision: null,
          },
        });
      }
      let rejected = null;
      try {
        await call("upsert_model_binding", {
          request: {
            taskType: "writer",
            providerId: "probe-readonly",
            modelId: "claude-readonly",
            expectedRevision: null,
          },
        });
      } catch (error) {
        rejected = JSON.stringify(error);
      }
      const bindings = await call("list_model_bindings");
      return { rejected, bindings };
    });

    console.log("只读协议绑定结果:", result.rejected);
    if (result.rejected === null) {
      throw new Error("只读协议不应被允许绑定到 writer");
    }
    if (!String(result.rejected).includes("NOT_SUPPORTED")) {
      throw new Error("拒绝原因应当是 NOT_SUPPORTED，实际 " + result.rejected);
    }
    const bound = (result.bindings || []).some(
      (item) => item.providerId === "probe-readonly" && item.taskType === "writer"
    );
    if (bound) {
      throw new Error("被拒绝的绑定不应落库：" + JSON.stringify(result.bindings));
    }
  });
});
