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
  "E2E 候选正文：少年入山。",
  // 审校第一轮必须返回真实问题，才会触发 bounded repair。
  JSON.stringify({
    issues: [{ type: "timeline", severity: "high", evidence: "时间线冲突", suggestion: "修正时间" }],
  }),
  "E2E 修复后的正文：少年入山，风雪扑面。",
  JSON.stringify({ issues: [] }),
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
          confirmPlaintextHostChange: true,
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
          confirmPlaintextHostChange: true,
        },
      });
      return { novelId: novel.id, chapterId: chapter.id, runId: opened.run_id ?? opened.runId };
    }, "http://127.0.0.1:" + port + "/v1");

    const { novelId, chapterId, runId } = result;
    console.log("发起生成:", JSON.stringify(result));

    // 6. 状态驱动推进：规划门点用 approve；候选稿门点（gate.id = approval）用编辑后确认。
    const stamp = Date.now();
    let status = "unknown";
    let confirmed = false;
    const deadline = Date.now() + 90_000;
    while (Date.now() < deadline) {
      const detail = await browser.execute(
        (nid, rid) => window.__TAURI_INTERNALS__.invoke("get_run_detail", { novelId: nid, runId: rid }),
        novelId,
        runId
      );
      status = detail.status;
      if (status === "succeeded" || status === "failed") {
        break;
      }
      const actions = detail.legalActions ?? [];
      const candidate = detail.latestCandidate ?? null;
      const gateId = detail.gate?.id ?? "";
      const canConfirm =
        status === "waiting_approval" &&
        gateId === "approval" &&
        (actions.includes("confirm") || actions.includes("confirm_after_edit")) &&
        candidate &&
        candidate.id &&
        candidate.contentSha256 &&
        typeof candidate.content === "string" &&
        candidate.content.length > 0;
      if (canConfirm) {
        const outcome = await browser.execute(
          async (rid, cand, cmd, version) => {
            try {
              await window.__TAURI_INTERNALS__.invoke("confirm_candidate_after_edit_run", {
                commandId: cmd,
                runId: rid,
                expectedVersion: version,
                baseCandidateId: cand.id,
                baseContentSha256: cand.contentSha256,
                content: cand.content,
              });
              return "ok";
            } catch (error) {
              return JSON.stringify(error);
            }
          },
          runId,
          candidate,
          "e2e-confirm-" + stamp,
          detail.controlVersion ?? null
        );
        console.log("确认候选稿:", outcome);
        confirmed = true;
        await browser.pause(600);
        continue;
      }
      if (status === "waiting_approval" && (actions.includes("approve") || actions.includes("confirm"))) {
        const outcome = await browser.execute(
          async (rid, cmd) => {
            try {
              await window.__TAURI_INTERNALS__.invoke("approve_run", { commandId: cmd, runId: rid });
              return "ok";
            } catch (error) {
              return JSON.stringify(error);
            }
          },
          runId,
          "e2e-approve-" + stamp + "-" + Date.now()
        );
        console.log("推进门点", gateId, JSON.stringify(actions), outcome);
      }
      await browser.pause(500);
    }
    console.log("是否已确认候选:", confirmed);
    console.log("最终状态:", status, "远端请求数:", calls);
    // 生成必须真的打到本地 Mock：这是「AI 功能真的跑了」的硬证据。
    if (calls < 2) {
      throw new Error("生成链路没有真正发出请求，实际：" + calls);
    }
    // Run 必须停在可恢复点或成功，不能是失败/丢状态。
    if (!["succeeded", "waiting_approval", "paused"].includes(status)) {
      throw new Error("Run 状态异常：" + status);
    }

    const chapter = await browser.execute(
      (nid, cid) => window.__TAURI_INTERNALS__.invoke("get_chapter", { novelId: nid, chapterId: cid }),
      novelId,
      chapterId
    );
    const content = chapter?.content ?? "";
    console.log("最终正文长度:", String(content).length, "状态:", status);
    if (status !== "succeeded") {
      throw new Error("确认后 Run 未成功结束，状态：" + status + " 请求数：" + calls);
    }
    if (String(content).length === 0) {
      throw new Error("确认后正文仍为空");
    }
  });
});
