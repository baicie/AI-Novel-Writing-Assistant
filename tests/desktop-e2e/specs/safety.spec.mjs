async function tauriInvoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Native Director safety pause", () => {
  it("stops at Safety when review finds issues and hides ordinary resume", async () => {
    const novel = await tauriInvoke("create_novel", { title: "WDIO 安全问题书", description: "" });
    const chapter = await tauriInvoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    const start = await $("[data-testid=director-start-button]");
    await start.waitForDisplayed({ timeout: 20_000 });
    await start.click();
    // Confirm all real gates until the run reaches the review/repair boundary.
    for (let index = 0; index < 4; index += 1) {
      try {
        const approve = await $("[data-testid=director-approve-button]");
        await approve.waitForDisplayed({ timeout: 30_000 });
        await approve.click();
      } catch { break; }
    }
    await browser.waitUntil(async () => {
      const text = await (await $("[data-testid=director-status]")).getText();
      return text.includes("已暂停") || text.includes("paused");
    }, { timeout: 45_000, timeoutMsg: "Safety pause did not appear" });
    const runs = await tauriInvoke("list_runs", { novelId: novel.id });
    const run = runs.find(item => item.chapterId === chapter.id);
    const detail = await tauriInvoke("get_run_detail", { novelId: novel.id, runId: run.id });
    expect(detail.pauseKind).toBe("safety");
    expect(detail.artifacts.some(a => a.artifactType === "repair_notes")).toBe(false);
    expect(detail.artifacts.some(a => a.artifactType === "chapter_commit")).toBe(false);
    expect(await (await $("[data-testid=director-resume-button]")).isExisting()).toBe(false);
    const saved = await tauriInvoke("get_chapter", { novelId: novel.id, chapterId: chapter.id });
    expect(saved.content).toBe("");
  });
});
