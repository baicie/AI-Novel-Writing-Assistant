import fs from "node:fs";
import path from "node:path";

async function tauriInvoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Chapter editor SIGKILL recovery", () => {
  it("restores the pre-SIGKILL draft in a new process", async () => {
    const state = JSON.parse(fs.readFileSync(path.join(process.env.BIZ_NOVEL_E2E_DATA_DIR, "editor-crash-state.json"), "utf8"));
    await browser.execute(([n, c]) => { window.location.hash = `#/novels/${n}/chapters/${c}`; }, [state.novelId, state.chapterId]);
    await (await $("[data-testid=chapter-editor-surface]")).waitForDisplayed({ timeout: 20_000 });
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("SIGKILL 前未保存的正文"), { timeout: 20_000, timeoutMsg: "draft missing after SIGKILL" });
    const detail = await tauriInvoke("get_chapter", { novelId: state.novelId, chapterId: state.chapterId });
    expect(detail.content).toBe("");
    const save = await $("[data-testid=chapter-save-button]");
    await save.waitForEnabled({ timeout: 20_000 });
    await save.click();
    await browser.waitUntil(async () => (await tauriInvoke("get_chapter", { novelId: state.novelId, chapterId: state.chapterId })).content.includes("SIGKILL 前未保存的正文"), { timeout: 20_000 });
  });
});
