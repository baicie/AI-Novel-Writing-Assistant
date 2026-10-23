async function tauriInvoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Native Director UI cancellation", () => {
  it("cancels a running generation from the UI and does not commit text", async () => {
    const novel = await tauriInvoke("create_novel", { title: "WDIO 取消书", description: "" });
    const chapter = await tauriInvoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    const start = await $("[data-testid=director-start-button]");
    await start.waitForDisplayed({ timeout: 20_000 });
    await start.click();
    const cancel = await $("[data-testid=director-cancel-button]");
    await cancel.waitForDisplayed({ timeout: 20_000 });
    await cancel.click();
    await browser.waitUntil(async () => {
      const text = await (await $("[data-testid=director-status]")).getText();
      return text.includes("已取消") || text.includes("cancelled");
    }, { timeout: 30_000, timeoutMsg: "UI cancel did not reach cancelled" });
    const runs = await tauriInvoke("list_runs", { novelId: novel.id });
    const run = runs.find(item => item.chapterId === chapter.id);
    expect(run?.state).toBe("cancelled");
    const detail = await tauriInvoke("get_run_detail", { novelId: novel.id, runId: run.id });
    expect(detail.status).toBe("cancelled");
    const saved = await tauriInvoke("get_chapter", { novelId: novel.id, chapterId: chapter.id });
    expect(saved.content).toBe("");
  });
});
