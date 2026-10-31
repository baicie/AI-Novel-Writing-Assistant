// 验证：导航栏选择器 + 推理强度落库（真实 app）。
describe("导航栏模型选择与推理强度", () => {
  const invoke = (command, args) =>
    browser.execute((cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload), command, args);

  it("种子数据后选择器可用、强度可保存", async () => {
    await browser.pause(2500);

    const seeded = await browser.execute(async () => {
      const invoke = window.__TAURI_INTERNALS__.invoke;
      // 同一份应用数据会被多次运行复用，种子必须幂等：先读当前状态，再按 revision 更新。
      const profiles = await invoke("list_provider_profiles");
      const existing = (profiles || []).find((item) => item.id === "probe");
      await invoke("upsert_provider_profile", {
        request: {
          providerId: "probe",
          displayName: "探针网关",
          protocol: "openai-compatible",
          baseUrl: "https://api.example.com/v1",
          allowLocalhostHttp: false,
          sortOrder: 0,
          isBuiltin: false,
          credential: { kind: "replace", apiKey: "sk-probe" },
          expectedRevision: existing ? existing.revision : null,
        },
      });
      const models = await invoke("list_provider_models", { providerId: "probe" });
      const hasModel = (models || []).some((item) => item.modelId === "probe-model");
      if (!hasModel) {
        await invoke("upsert_provider_model", {
          request: {
            providerId: "probe",
            modelId: "probe-model",
            reasoningEfforts: ["low", "medium", "high", "max"],
            expectedRevision: null,
          },
        });
      }
      return "seeded";
    });
    console.log("种子:", seeded);

    // 切到设置页再回来，模拟用户在设置里加完模型回首页
    await browser.execute(() => { location.hash = "#/settings"; });
    await browser.pause(1200);
    await browser.execute(() => { location.hash = "#/"; });
    await browser.pause(1800);

    const structure = await browser.execute(() => {
      const labels = Array.from(document.querySelectorAll("select")).map((el) =>
        el.getAttribute("aria-label"),
      );
      const effort = document.querySelector('select[aria-label="推理强度"]');
      return {
        selects: labels.filter(Boolean),
        effortOptions: effort
          ? Array.from(effort.querySelectorAll("option")).map((o) => o.textContent)
          : null,
      };
    });
    console.log("selects:", JSON.stringify(structure.selects));
    console.log("强度选项:", JSON.stringify(structure.effortOptions));

    await browser.execute(() => {
      const pick = (label, value) => {
        const el = document.querySelector('select[aria-label="' + label + '"]');
        if (!el) return false;
        const setter = Object.getOwnPropertyDescriptor(window.HTMLSelectElement.prototype, "value").set;
        setter.call(el, value);
        el.dispatchEvent(new Event("change", { bubbles: true }));
        return true;
      };
      pick("模型来源", "probe");
    });
    await browser.pause(1500);
    const modelOk = await browser.execute(() => {
      const el = document.querySelector('select[aria-label="模型"]');
      if (!el) return false;
      const setter = Object.getOwnPropertyDescriptor(window.HTMLSelectElement.prototype, "value").set;
      setter.call(el, "probe-model");
      el.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    });
    console.log("选择模型:", modelOk);
    await browser.pause(1500);

    const effortSet = await browser.execute(() => {
      const el = document.querySelector('select[aria-label="推理强度"]');
      if (!el) return false;
      const setter = Object.getOwnPropertyDescriptor(window.HTMLSelectElement.prototype, "value").set;
      setter.call(el, "high");
      el.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    });
    console.log("选择强度:", effortSet);
    await browser.pause(1800);

    const bindings = await invoke("list_model_bindings");
    console.log("绑定:", JSON.stringify(bindings));
    const writer = (bindings || []).find((item) => item.taskType === "writer");
    expect(writer?.providerId).toBe("probe");
    expect(writer?.modelId).toBe("probe-model");
    expect(writer?.reasoningEffort).toBe("high");
  });
});
