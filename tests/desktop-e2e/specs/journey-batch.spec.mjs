import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Shared fixture journey: continuous creation", () => {
  it("writes ten chapters through the existing gates without duplicating commits", async function () {
    this.timeout(600_000);
    const fixture = JSON.parse(fs.readFileSync(`${dataDir}/journey-fixture.json`, "utf8"));
    const model = await invoke("get_model_setup");
    expect(model.hasApiKey).toBe(true);

    let batches = await invoke("list_batches", { novelId: fixture.novelId });
    if (!batches.length) {
      await invoke("create_batch", { request: {
        commandId: `journey-batch-${Date.now()}`,
        novelId: fixture.novelId,
        chapterIds: fixture.chapterIds,
        provider: "openai-compatible",
        model: model.model,
        promptVersions: { chapter_draft: "chapter-draft-v2" },
        tokenBudget: null,
        contextTokenBudget: null,
        maxAutomaticRetries: 0,
        repairBudget: 1,
        endpoint: model.endpoint,
        allowLocalhostHttp: model.allowLocalhostHttp,
      } });
      batches = await invoke("list_batches", { novelId: fixture.novelId });
    }
    const batchId = batches[0].id;

    const deadline = Date.now() + 480_000;
    let autoResumes = 0;
    let lastError = "";
    let lastState = "";
    while (Date.now() < deadline) {
      try {
        const batch = await invoke("get_batch", { batchId });
        if (batch.status === "completed" || batch.status === "cancelled" || batch.status === "failed") break;
        const item = batch.items[batch.cursor] ?? batch.items.find((entry) => entry.status !== "succeeded");
        lastState = `batch=${batch.status} cursor=${batch.cursor} item=${item?.status ?? "none"} run=${item?.runId ?? "none"}`;
        if (item?.runId) {
          const detail = await invoke("get_run_detail", { novelId: fixture.novelId, runId: item.runId });
          lastState += ` run=${detail.status} signal=${detail.pauseSignal ?? "-"} actions=${(detail.legalActions ?? []).join(",")}`;
          if (detail.pauseSignal === "safety" && detail.pauseKind === "safety") {
          for (const debt of detail.qualityDebt ?? []) {
            if (String(debt.runId) !== item.runId) continue;
            if (["accepted", "resolved"].includes(String(debt.status))) continue;
            await invoke("accept_quality_debt", {
              runId: item.runId,
              debtId: String(debt.id),
              expectedVersion: Number(debt.decisionVersion ?? 0),
              reason: "旅程：作者确认保留该质量问题。",
            });
          }
          if (detail.legalActions.includes("resume_quality_debt")) {
            await invoke("resume_run_with_quality_debt", {
              commandId: `journey-resume-${item.runId}-${++autoResumes}`,
              runId: item.runId,
              reason: "旅程：逐条确认后继续这一章。",
            });
          }
          } else if (detail.legalActions.includes("confirm")) {
            await invoke("approve_run", {
              commandId: `journey-approve-${item.runId}-${detail.controlVersion}`,
              runId: item.runId,
              expectedVersion: detail.controlVersion,
            });
          }
        }
      } catch (cause) {
        lastError = cause instanceof Error ? cause.message : String(cause);
        console.log("[journey] retrying after:", lastError, "|", lastState);
      }
      await browser.pause(500);
    }

    const finalBatch = await invoke("get_batch", { batchId });
    if (finalBatch.status !== "completed") {
      console.log("[journey] stuck:", finalBatch.status, "cursor", finalBatch.cursor, "|", lastState, "| lastError:", lastError);
    }
    expect(finalBatch.status).toBe("completed");
    expect(finalBatch.items.every((item) => item.status === "succeeded")).toBe(true);

    let commits = 0;
    for (const item of finalBatch.items) {
      const chapter = await invoke("get_chapter", { novelId: fixture.novelId, chapterId: item.chapterId });
      expect(chapter.officialVersion).toBeGreaterThanOrEqual(1);
      expect(chapter.content.length).toBeGreaterThan(20);
      if (item.runId) {
        const detail = await invoke("get_run_detail", { novelId: fixture.novelId, runId: item.runId });
        const chapterCommits = detail.artifacts.filter((artifact) => artifact.artifactType === "chapter_commit");
        expect(chapterCommits.length).toBe(1);
        commits += chapterCommits.length;
      }
    }
    expect(commits).toBe(10);

    fs.writeFileSync(`${dataDir}/journey-batch.json`, JSON.stringify({
      batchId,
      runIds: finalBatch.items.map((item) => item.runId),
    }, null, 2));
  });
});
