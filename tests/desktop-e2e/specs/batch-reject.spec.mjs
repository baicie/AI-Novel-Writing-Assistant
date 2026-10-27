async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native batch reject safety", () => {
  it("stops progression after rejecting the current candidate", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 批量拒绝", description: "" });
    for (let index = 1; index <= 3; index += 1) {
      await invoke("create_chapter", { novelId: novel.id, title: `第${index}章`, sortOrder: index });
    }
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await (await $("[data-testid=batch-workspace]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=batch-create]")).click();
    await browser.waitUntil(async () => {
      const batch = (await invoke("list_batches", { novelId: novel.id }))[0];
      return batch?.items?.[0]?.runId;
    }, { timeout: 30_000 });
    const reject = await $("[data-testid=director-reject-button]");
    await reject.waitForDisplayed({ timeout: 30_000 });
    await reject.click();
    await browser.pause(2_000);
    const batch = (await invoke("list_batches", { novelId: novel.id }))[0];
    expect(["waiting_approval", "failed", "cancelled"]).toContain(batch.status);
    expect(batch.items.filter((item) => item.runId).length).toBe(1);
    expect(batch.items[1].runId).toBeNull();
    expect(batch.items[2].runId).toBeNull();
    const chapter1 = await invoke("get_chapter", { novelId: novel.id, chapterId: batch.items[0].chapterId });
    expect(chapter1.officialVersion).toBe(0);
  });
});
