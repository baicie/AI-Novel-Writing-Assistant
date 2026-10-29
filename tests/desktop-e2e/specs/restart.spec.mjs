import fs from "node:fs";
import path from "node:path";

async function tauriInvoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Native Director restart persistence", () => {
  it("restores official text, run history and artifacts after a new app process", async () => {
    const statePath = path.join(process.env.BIZ_NOVEL_E2E_DATA_DIR, "e2e-state.json");
    const state = JSON.parse(fs.readFileSync(statePath, "utf8"));
    const chapter = await tauriInvoke("get_chapter", { novelId: state.novelId, chapterId: state.chapterId });
    expect(chapter.content).toContain("E2E 候选正文");
    expect(chapter.officialVersion).toBeGreaterThanOrEqual(1);
    const runs = await tauriInvoke("list_runs", { novelId: state.novelId });
    expect(runs.some(run => run.id === state.runId && run.state === "succeeded")).toBe(true);
    const detail = await tauriInvoke("get_run_detail", { novelId: state.novelId, runId: state.runId });
    expect(detail.status).toBe("succeeded");
    expect(detail.artifacts.some(a => a.artifactType === "story_macro")).toBe(true);
    expect(detail.artifacts.some(a => a.artifactType === "chapter_commit")).toBe(true);
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, state.novelId);
    await browser.waitUntil(async () => (await $("[data-testid=director-status]")).isExisting(), { timeout: 20_000 });
    expect(await (await $("[data-testid=director-status]")).getText()).toContain("已完成");
    expect(await browser.execute(() => document.body.innerText)).toContain("运行历史");
  });
});
