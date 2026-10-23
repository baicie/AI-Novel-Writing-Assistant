import fs from "node:fs";
import path from "node:path";
const state = JSON.parse(fs.readFileSync(path.join(process.env.BIZ_NOVEL_E2E_DATA_DIR, "candidate-state.json"), "utf8"));
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }
describe("Candidate edit journal restart", () => {
  it("restores the unconfirmed edit in a new app process and commits it", async () => {
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, state.novelId);
    await browser.refresh();
    await (await $("[data-testid=director-edit-candidate-button]")).waitForDisplayed({ timeout: 30_000 });
    await (await $("[data-testid=director-edit-candidate-button]")).click();
    const recovery = await $("[data-testid=director-candidate-draft-recovery]");
    await recovery.waitForDisplayed({ timeout: 20_000 });
    const choice = await $("[data-testid=director-candidate-draft-choice]");
    await choice.waitForDisplayed({ timeout: 10_000 });
    await choice.click();
    const input = await $("[data-testid=director-candidate-edit-input]");
    await input.waitForDisplayed({ timeout: 20_000 });
    await browser.waitUntil(async () => (await input.getValue()).includes("候选草稿跨进程恢复正文"), { timeout: 20_000, timeoutMsg: "candidate journal was not restored" });
    await (await $("[data-testid=director-confirm-candidate-edit]")).click();
    await browser.waitUntil(async () => (await invoke("get_run_detail", { novelId: state.novelId, runId: state.runId })).status === "succeeded", { timeout: 45_000 });
    const chapter = await invoke("get_chapter", { novelId: state.novelId, chapterId: state.chapterId });
    expect(chapter.content).toBe("候选草稿跨进程恢复正文");
    const detail = await invoke("get_run_detail", { novelId: state.novelId, runId: state.runId });
    expect(detail.artifacts.some(artifact => artifact.artifactType === "chapter_approval" && artifact.status === "user_edited")).toBe(true);
  });
});
