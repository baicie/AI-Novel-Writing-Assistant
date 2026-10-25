import fs from "node:fs";
import path from "node:path";

async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native batch restart recovery", () => {
  it("resumes the persisted batch and completes three chapters in a new process", async () => {
    const state = JSON.parse(fs.readFileSync(path.join(process.env.BIZ_NOVEL_E2E_DATA_DIR, "batch-restart-state.json"), "utf8"));
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, state.novelId);
    await (await $("[data-testid=batch-workspace]")).waitForDisplayed({ timeout: 20_000 });
    let batch = (await invoke("list_batches", { novelId: state.novelId }))[0];
    expect(batch.id).toBe(state.batchId);
    expect(batch.items).toHaveLength(3);
    const firstBoundRun = batch.items.find((item) => item.runId)?.runId;
    expect(firstBoundRun).toBeTruthy();

    const deadline = Date.now() + 120_000;
    while (Date.now() < deadline && batch.status !== "completed" && batch.status !== "failed") {
      batch = (await invoke("list_batches", { novelId: state.novelId }))[0];
      const item = batch.items[batch.cursor] ?? batch.items.find((entry) => entry.status !== "succeeded");
      if (item?.runId) {
        const detail = await invoke("get_run_detail", { novelId: state.novelId, runId: item.runId });
        if (Array.isArray(detail.legalActions) && detail.legalActions.includes("confirm")) {
          const approve = await $("[data-testid=director-approve-button]");
          await approve.waitForDisplayed({ timeout: 30_000 });
          await approve.click();
        } else if (Array.isArray(detail.legalActions) && detail.legalActions.includes("resume") && detail.latestCandidate) {
          const resume = await $("[data-testid=director-resume-button]");
          await resume.waitForDisplayed({ timeout: 30_000 });
          await resume.click();
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 700));
    }
    expect(batch.status).toBe("completed");
    expect(new Set(batch.items.map((item) => item.runId)).size).toBe(3);
    for (const chapterId of state.chapterIds) {
      const chapter = await invoke("get_chapter", { novelId: state.novelId, chapterId });
      expect(chapter.officialVersion).toBeGreaterThanOrEqual(1);
      expect(chapter.content.length).toBeGreaterThan(0);
    }
  });
});
