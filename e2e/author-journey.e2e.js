// WIP（未注册到 wdio.conf.js）：已验证「真实应用 → 本地 Mock 收到 6 次请求 → Run 停在等待确认」；
// 尚未打通「作者确认 → 正式正文落库」这一段，因此不纳入门禁。
// A：新用户闭环 —— 建来源 → 加模型 → 绑定 → 建书/章 → 发起生成 → 真实 Mock 收到请求 → 确认 → 正文落库。
// 模型请求全部打到本进程内的本地 Mock Server，不访问任何付费服务。
import http from "node:http";

const story = () =>
  JSON.stringify({
    expansion: {
      expanded_premise: "E2E 闭环",
      protagonist_core: "主角",
      conflict_engine: "冲突",
      conflict_layers: { external: "外", internal: "内" },
      mystery_box: "未知",
      emotional_line: "情绪",
      setpiece_seeds: ["场面"],
      tone_reference: "气质",
    },
    decomposition: {
      selling_point: "卖点",
      core_conflict: "冲突",
      main_hook: "钩子",
      progression_loop: "循环",
      growth_path: "成长",
      major_payoffs: ["回报"],
      ending_flavor: "结局",
    },
    constraints: ["规则"],
    issues: [],
  });

const script = [
  story(),
  JSON.stringify({
    members: [
      { name: "林动", role: "主角", castRole: "protagonist", storyFunction: "承载" },
      { name: "苏晚", role: "同伴", castRole: "ally", storyFunction: "协助" },
      { name: "陆沉", role: "对手", castRole: "antagonist", storyFunction: "施压" },
    ],
    relations: [
      { sourceName: "林动", targetName: "苏晚", surfaceRelation: "同门" },
      { sourceName: "林动", targetName: "陆沉", surfaceRelation: "敌对" },
    ],
  }),
  JSON.stringify({
    volumes: [{ volumeOrder: 1, title: "山门卷", roleLabel: "开局", coreReward: "入门", planningMode: "hard" }],
  }),
  JSON.stringify({
    chapters: [{ chapterOrder: 1, title: "第一章", volumeTitle: "山门卷", summary: "入山", characterNames: ["林动"] }],
  }),
  "E2E 生成出来的正文：少年入山，风雪扑面。",
];

describe("新用户闭环（本地 Mock 生成）", () => {
  let server;
  let port;
  let calls = 0;

  before(async () => {
    server = http.createServer((req, res) => {
      calls += 1;
      let body = "";
      req.on("data", (chunk) => (body += chunk));
      req.on("end", () => {
        const content = script[Math.min(calls - 1, script.length - 1)];
        const event = JSON.stringify({ choices: [{ delta: { content } }] });
        res.writeHead(200, { "content-type": "text/event-stream" });
        res.end("data: " + event + "\n\ndata: [DONE]\n\n");
      });
    });
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    port = server.address().port;
  });

  after(async () => {
    if (server) {
      await new Promise((resolve) => server.close(resolve));
    }
  });

  it("真实 UI 数据链路能把生成请求打到本地 Mock，并在确认前不落正式正文", async () => {
    await browser.pause(2500);
    const result = await browser.execute(async (endpointBase) => {
      const call = window.__TAURI_INTERNALS__.invoke;
      const base = endpointBase;

      // 1. 来源（本地 mock，允许明文 http）
      const profiles = await call("list_provider_profiles");
      const existing = (profiles || []).find((item) => item.id === "e2e-local");
      await call("upsert_provider_profile", {
        request: {
          providerId: "e2e-local",
          displayName: "本地 Mock",
          protocol: "openai-compatible",
          baseUrl: base,
          allowLocalhostHttp: true,
          sortOrder: 2,
          isBuiltin: false,
          credential: { kind: "replace", apiKey: "sk-e2e-local" },
          expectedRevision: existing ? existing.revision : null,
        },
      });

      // 2. 模型
      const models = await call("list_provider_models", { providerId: "e2e-local" });
      if (!(models || []).some((item) => item.modelId === "local-model")) {
        await call("upsert_provider_model", {
          request: { providerId: "e2e-local", modelId: "local-model", expectedRevision: null },
        });
      }

      // 3. 四个环节都绑定到本地 mock
      for (const taskType of ["planner", "writer", "review", "repair"]) {
        await call("upsert_model_binding", {
          request: {
            taskType,
            providerId: "e2e-local",
            modelId: "local-model",
            expectedRevision: null,
          },
        });
      }

      // 4. 书与章节
      const novel = await call("create_novel", { title: "E2E 闭环书", description: "" });
      const chapter = await call("create_chapter", {
        novelId: novel.id,
        title: "第一章",
        sortOrder: 1,
      });

      // 5. 发起生成
      const opened = await call("open_run", {
        request: {
          commandId: "e2e-open-" + Date.now(),
          novelId: novel.id,
          targetChapterId: chapter.id,
          provider: "e2e-local",
          model: "local-model",
          promptVersions: { chapter_draft: "chapter-draft-v1" },
          tokenBudget: 4096,
          contextTokenBudget: null,
          maxAutomaticRetries: 0,
          repairBudget: 1,
          endpoint: base + "/chat/completions",
          allowLocalhostHttp: true,
        },
      });
      return { novelId: novel.id, chapterId: chapter.id, runId: opened.run_id ?? opened.runId };
    }, "http://127.0.0.1:" + port + "/v1");

    const { novelId, chapterId, runId } = result;
    console.log("发起生成:", JSON.stringify(result));

    // 6. 等待后台 worker 推进（必要时批准），直到成功或停在可恢复点
    let status = "unknown";
    for (let i = 0; i < 40; i += 1) {
      await browser.pause(800);
      const detail = await browser.execute(
        (nid, rid) => window.__TAURI_INTERNALS__.invoke("get_run_detail", { novelId: nid, runId: rid }),
        novelId,
        runId
      );
      status = detail.status;
      if (status === "waiting_approval") {
        await browser.execute(
          (rid, idx) =>
            window.__TAURI_INTERNALS__.invoke("approve_run", {
              commandId: "e2e-approve-" + idx,
              runId: rid,
            }),
          runId,
          i
        );
      }
      if (status === "succeeded" || status === "failed") {
        break;
      }
    }
    console.log("最终状态:", status, "远端请求数:", calls);
    // 生成必须真的打到本地 Mock：这是「AI 功能真的跑了」的硬证据。
    if (calls < 5) {
      throw new Error("生成链路请求数不足，实际：" + calls);
    }
    // Run 必须停在可恢复点或成功，不能是失败/丢状态。
    if (!["succeeded", "waiting_approval", "paused"].includes(status)) {
      throw new Error("Run 状态异常：" + status);
    }

    // 章节正文在作者确认前保持为空，这是产品语义（先预览后落笔），不是缺陷。
    const chapter = await browser.execute(
      (nid, cid) => window.__TAURI_INTERNALS__.invoke("get_chapter", { novelId: nid, chapterId: cid }),
      novelId,
      chapterId
    );
    const content = chapter?.content ?? "";
    console.log("确认前正文长度:", String(content).length);
    if (String(content).length !== 0) {
      throw new Error("未确认前不应直接写入正式正文：" + JSON.stringify(chapter));
    }
  });
});
