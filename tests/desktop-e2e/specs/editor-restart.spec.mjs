import fs from "node:fs";
import path from "node:path";

async function tauriInvoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Chapter editor durable draft across app processes", () => {
  it("restores an unsaved draft in a new app process and allows official save", async () => {
    const state = JSON.parse(fs.readFileSync(path.join(process.env.BIZ_NOVEL_E2E_DATA_DIR, "editor-state.json"), "utf8"));
    const { novelId } = state;
    const chapterId = state.chapterIds[0];
    await browser.execute(([n, c]) => { window.location.hash = `#/novels/${n}/chapters/${c}`; }, [novelId, chapterId]);
    await (await $("[data-testid=chapter-editor-surface]")).waitForDisplayed({ timeout: 20_000 });
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("测试草稿刷新"), { timeout: 20_000, timeoutMsg: "durable draft missing after app restart" });
    await (await $("[data-testid=chapter-draft-banner]")).waitForDisplayed({ timeout: 10_000 });
    const save = await $("[data-testid=chapter-save-button]");
    await save.waitForDisplayed({ timeout: 10_000 });
    if (await save.isEnabled()) await save.click();
    await browser.waitUntil(async () => {
      const chapter = await tauriInvoke("get_chapter", { novelId, chapterId });
      return chapter.content.includes("测试草稿刷新") && chapter.officialVersion >= 1;
    }, { timeout: 20_000, timeoutMsg: "official save did not persist restored draft" });
    const chapter = await tauriInvoke("get_chapter", { novelId, chapterId });
    expect(chapter.content).toContain("测试草稿刷新");
    expect(chapter.officialVersion).toBeGreaterThanOrEqual(1);
  });
});
