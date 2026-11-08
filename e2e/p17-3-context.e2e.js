// P17.3：Director 上下文来源追踪（真实 Tauri IPC / SQLite / 本地 Mock LLM）。
//
// 验证：已确认事实进入运行上下文、Context Trace 记录来源、数据库后续变化
// 不修改历史 Run 的冻结上下文；上下文预览读取当前事实，但不把候选当成已确认事实。
import http from "node:http";

const invoke = (command, args) =>
  browser.execute(
    (cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload),
    command,
    args,
  );

const NOVEL_TITLE = "P17.3 上下文追踪";

const story = () =>
  JSON.stringify({
    expansion: {
      expanded_premise: "P17.3 上下文",
      protagonist_core: "林动",
      conflict_engine: "寻找真相",
      conflict_layers: { external: "山门", internal: "犹疑" },
      mystery_box: "旧案",
      emotional_line: "信任",
      setpiece_seeds: ["夜巡"],
      tone_reference: "冷峻",
    },
    decomposition: {
      selling_point: "连续性",
      core_conflict: "真相与秩序",
      main_hook: "旧案",
      progression_loop: "查案",
      growth_path: "立住",
      major_payoffs: ["翻案"],
      ending_flavor: "克制",
    },
    keyEvents: [{ id: "ch1-event-1", chapterOrder: 1, type: "required_event", description: "林动按计划入山", participants: ["林动"], enforcement: "hard" }],
    constraints: ["不能替作者改设定"],
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
    chapters: [{ chapterOrder: 1, title: "第一章", volumeTitle: "山门卷", summary: "入山", characterNames: ["林动"], obligations: [{ id: "ch1-req-1", type: "required_event", description: "林动按计划入山", enforcement: "hard", participants: ["林动"], sourceEventId: "ch1-event-1" }] }],
  }),
  "他推开门，屋里没有人。",
  JSON.stringify({ issues: [] }),
];

describe("Director 上下文来源追踪（桌面原生）", function () {
  this.timeout(180_000);
  let server;
  let port;
  let memoryServer;
  let memoryPort;
  let calls = 0;
  let novelId = "";
  let chapterId = "";
  let runId = "";
  const heroId = "p173-context-hero";
  let frozenTrace = null;
  let frozenPrompt = "";

  before(async function () {
    this.timeout(180_000);
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
    const base = "http://127.0.0.1:" + port + "/v1";

    memoryServer = http.createServer((req, res) => {
      let body = "";
      req.on("data", (chunk) => (body += chunk));
      req.on("end", () => {
        const content = JSON.stringify({
          facts: [
            {
              factType: "character_state",
              truthfulness: "narrator_fact",
              entityName: "林动",
              value: "状态：警觉",
              evidenceQuote: "他推开门",
              confidence: 0.8,
            },
          ],
        });
        const event = JSON.stringify({ choices: [{ delta: { content } }] });
        res.writeHead(200, { "content-type": "text/event-stream" });
        res.end("data: " + event + "\n\ndata: [DONE]\n\n");
      });
    });
    await new Promise((resolve) => memoryServer.listen(0, "127.0.0.1", resolve));
    memoryPort = memoryServer.address().port;

    await browser.pause(2500);
    const seeded = await browser.execute(async (title, hero, endpointBase) => {
      const call = window.__TAURI_INTERNALS__.invoke;
      const profiles = (await call("list_provider_profiles")) || [];
      const existing = profiles.find((item) => item.id === "p173-context-local");
      await call("upsert_provider_profile", {
        request: {
          providerId: "p173-context-local",
          displayName: "P17.3 上下文 Mock",
          protocol: "openai-compatible",
          baseUrl: endpointBase,
          allowLocalhostHttp: true,
          confirmPlaintextHostChange: true,
          sortOrder: 3,
          isBuiltin: false,
          credential: { kind: "replace", apiKey: "sk-p173-context" },
          expectedRevision: existing ? existing.revision : null,
        },
      });
      const models = (await call("list_provider_models", { providerId: "p173-context-local" })) || [];
      if (!models.some((item) => item.modelId === "context-model")) {
        await call("upsert_provider_model", {
          request: { providerId: "p173-context-local", modelId: "context-model", expectedRevision: null },
        });
      }
      for (const taskType of ["planner", "writer", "review", "repair"]) {
        await call("upsert_model_binding", {
          request: { taskType, providerId: "p173-context-local", modelId: "context-model", expectedRevision: null },
        });
      }

      const novels = (await call("list_novels")) || [];
      const existingNovel = novels.find((item) => item.title === title);
      const novel = existingNovel || (await call("create_novel", { title, description: "" }));
      const chapters = (await call("list_chapters", { novelId: novel.id })) || [];
      const existingChapter = chapters.find((item) => item.sortOrder === 1);
      const chapter = existingChapter
        ? existingChapter
        : await call("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
      await call("save_chapter", {
        novelId: novel.id,
        chapterId: chapter.id,
        expectedVersion: chapter.officialVersion ?? 0,
        title: "第一章",
        content: "他推开门，屋里没有人。\n\n林动还活着。",
      });
      const characters = (await call("list_characters", { novelId: novel.id })) || [];
      if (!characters.some((item) => item.id === hero)) {
        await call("create_character", { request: { id: hero, novelId: novel.id, name: "林动", role: "主角" } });
      }
      return { novelId: novel.id, chapterId: chapter.id };
    }, NOVEL_TITLE, heroId, base);
    novelId = seeded.novelId;
    chapterId = seeded.chapterId;

    await invoke("confirm_structured_fact", {
      fact: {
        id: "p173-context-alive",
        novelId,
        entityId: heroId,
        factType: "life_state",
        valueJson: '{"state":"alive"}',
        sourceChapterId: chapterId,
        sourceRevision: 1,
        evidenceQuote: "林动还活着",
        effectiveChapterOrder: 1,
        nowMs: Date.now(),
      },
    });
    await invoke("create_plot_thread", {
      request: {
        plotId: "p173-context-plot",
        novelId,
        title: "旧案",
        body: "林动在查旧案",
        chapterId,
        sourceRevision: 1,
        evidence: "他推开门",
        commandId: "p173-context-plot-create",
      },
    });
    await invoke("upsert_timeline_event", {
      request: {
        id: "p173-context-event",
        novelId,
        chapterOrder: 1,
        body: "旧案线索出现",
        sourceChapterId: chapterId,
        sourceRevision: "1",
        status: "confirmed",
      },
    });

    const opened = await invoke("open_run", {
      request: {
        commandId: "p173-context-open-" + Date.now(),
        novelId,
        targetChapterId: chapterId,
        provider: "p173-context-local",
        model: "context-model",
        promptVersions: { chapter_draft: "chapter-draft-v1" },
        tokenBudget: 4096,
        contextTokenBudget: 24000,
        maxAutomaticRetries: 0,
        repairBudget: 1,
        endpoint: base + "/chat/completions",
        allowLocalhostHttp: true,
        confirmPlaintextHostChange: true,
      },
    });
    runId = opened.run_id ?? opened.runId;

    // 推进两个规划门点，直到 context 步骤执行并保存 trace。
    const deadline = Date.now() + 150_000;
    let lastStatus = "";
    while (Date.now() < deadline) {
      const detail = await invoke("get_run_detail", { novelId, runId });
      const trace = await invoke("get_run_context_trace", { novelId, runId }).catch(() => null);
      if (trace) {
        frozenTrace = trace;
        const artifact = (detail.artifacts || []).find((item) => item.artifactType === "context");
        frozenPrompt = artifact?.payloadJson ?? "";
        break;
      }
      const stamp = `${detail.status}:${(detail.legalActions || []).join(",")}:${detail.pauseKind || ""}:${detail.pauseSignal || ""}`;
      if (stamp !== lastStatus) {
        console.log(
          "P17.3 run:",
          stamp,
          "pause=",
          detail.pauseReason || "",
          "failure=",
          detail.failureReason || "",
          "cursor=",
          detail.cursorStepId || "",
        );
        lastStatus = stamp;
      }
      if (detail.status === "failed" || detail.status === "succeeded") break;
      const actions = detail.legalActions ?? [];
      if (detail.status === "paused" && !actions.includes("resume") && !actions.includes("approve")) {
        break;
      }
      // 规划门点用 approve_run 推进；候选正文门点（gate.id=approval）必须走候选 CAS，不能在这里确认。
      const gateId = detail.gate?.id ?? "";
      if (
        detail.status === "waiting_approval" &&
        gateId !== "approval" &&
        (actions.includes("approve") || actions.includes("confirm"))
      ) {
        await invoke("approve_run", {
          commandId: "p173-context-approve-" + Date.now(),
          runId,
        }).catch(() => undefined);
      }
      await browser.pause(500);
    }
    if (!frozenTrace) {
      const detail = await invoke("get_run_detail", { novelId, runId }).catch(() => null);
      console.log("P17.3 no context trace:", JSON.stringify(detail));
      throw new Error("Run 没有执行到 context 步骤，无法验证上下文来源追踪。");
    }
  });

  after(async () => {
    if (server) {
      await new Promise((resolve) => server.close(resolve));
    }
    if (memoryServer) {
      await new Promise((resolve) => memoryServer.close(resolve));
    }
  });

  it("已确认事实进入运行上下文，trace 记录来源", async () => {
    expect(frozenTrace).not.toBeNull();
    const structured = frozenTrace.sources.find((source) => source.sourceType === "structured_story_facts");
    expect(structured).toBeTruthy();
    expect(structured.selected).toBe(true);
    expect(structured.contentHash).toBeTruthy();
    const storyFacts = frozenTrace.sources.find((source) => source.sourceType === "story_facts");
    expect(storyFacts).toBeTruthy();
    expect(storyFacts.selected).toBe(true);
    expect(frozenPrompt).toContain("alive");
    expect(frozenPrompt).toContain("林动");
    expect(frozenPrompt).toContain("旧案");
    expect(frozenPrompt).toContain("旧案线索");
  });

  it("数据库后续变化不修改历史 Run 的冻结上下文", async () => {
    await invoke("confirm_structured_fact", {
      fact: {
        id: "p173-context-location",
        novelId,
        entityId: heroId,
        factType: "location",
        valueJson: '{"location":"北境哨塔"}',
        sourceChapterId: chapterId,
        sourceRevision: 1,
        evidenceQuote: "林动还活着",
        effectiveChapterOrder: 1,
        nowMs: Date.now(),
      },
    });
    const again = await invoke("get_run_context_trace", { novelId, runId });
    expect(again.contentHash).toBe(frozenTrace.contentHash);
    expect(again.sources).toEqual(frozenTrace.sources);

    const detail = await invoke("get_run_detail", { novelId, runId });
    const artifact = (detail.artifacts || []).find((item) => item.artifactType === "context");
    expect(artifact.payloadJson).toBe(frozenPrompt);
    expect(artifact.payloadJson).not.toContain("北境哨塔");
  });

  it("上下文预览读取当前事实，不把候选当成已确认事实", async () => {
    const before = await invoke("preview_chapter_context", {
      novelId,
      chapterId,
      inputBudget: 24000,
      outputReserve: 8000,
    });
    expect(before.trace.contentHash).not.toBe(frozenTrace.contentHash);
    expect(before.trace.sources.some((source) => source.sourceType === "structured_story_facts" && source.selected)).toBe(true);

    // 抽取一个未确认候选：预览的已确认事实集合不能被候选改变。
    await invoke("extract_memory", {
      request: {
        commandId: "p173-context-memory-" + Date.now(),
        novelId,
        chapterId,
        provider: "openai-compatible",
        model: "context-model",
        endpoint: "http://127.0.0.1:" + memoryPort + "/v1",
        allowLocalhostHttp: true,
        tokenBudget: 800,
      },
    });
    const after = await invoke("preview_chapter_context", {
      novelId,
      chapterId,
      inputBudget: 24000,
      outputReserve: 8000,
    });
    expect(after.trace.contentHash).toBe(before.trace.contentHash);
  });

  it("章节工作台能展示上下文来源与预算", async () => {
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      chapterId,
    );
    await browser.pause(2500);
    const tab = await $('[data-testid="panel-tab-context"]');
    await tab.waitForExist({ timeout: 10000 });
    await tab.click();
    const ready = await $('[data-testid="context-ready"]');
    await ready.waitForExist({ timeout: 15000 });
    expect(await ready.getText()).toContain("可以生成");
    const sources = await $$('[data-testid="context-source"]');
    expect(sources.length).toBeGreaterThan(0);
    const preview = await $('[data-testid="context-preview"]');
    expect(await preview.getText()).toContain("上下文版本");
    expect(await preview.getText()).toContain("预算");
  });
});
