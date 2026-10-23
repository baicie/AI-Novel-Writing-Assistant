import fs from "node:fs";
import path from "node:path";

async function tauriInvoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Confirm After Edit restart", () => {
  it("persists the author-edited candidate and official revision across app restart", async () => {
    const state = JSON.parse(fs.readFileSync(path.join(process.env.BIZ_NOVEL_E2E_DATA_DIR, "confirm-edit-state.json"), "utf8"));
    const chapter = await tauriInvoke("get_chapter", { novelId: state.novelId, chapterId: state.chapterId });
    expect(chapter.content).toBe(state.edited);
    const detail = await tauriInvoke("get_run_detail", { novelId: state.novelId, runId: state.runId });
    expect(detail.artifacts.some(artifact => artifact.artifactType === "chapter_approval" && artifact.status === "user_edited")).toBe(true);
    expect(detail.latestCandidate.content).toBe(state.edited);
  });
});
