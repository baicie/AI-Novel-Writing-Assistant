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

describe("Native batch reject then regenerate", () => {
  it("regenerates after reject and advances only after approval", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 拒绝再生成", description: "" });
    for (let index = 1; index <= 3; index += 1) {
      await invoke("create_chapter", { novelId: novel.id, title: `第${index}章`, sortOrder: index });
    }
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await (await $("[data-testid=batch-workspace]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=batch-create]")).click();
    await browser.waitUntil(async () => (await invoke("list_batches", { novelId: novel.id }))[0]?.items?.[0]?.runId, { timeout: 30_000 });
    const reject = await $("[data-testid=director-reject-button]");
    await reject.waitForDisplayed({ timeout: 30_000 });
    await reject.click();
    await browser.pause(1_000);
    const regenerate = await $("[data-testid=director-regenerate-button]");
    await regenerate.waitForDisplayed({ timeout: 15_000 });
    await regenerate.click();
    const deadline = Date.now() + 60_000;
    let advanced = false;
    while (Date.now() < deadline) {
      const batch = (await invoke("list_batches", { novelId: novel.id }))[0];
      if ((batch.cursor ?? 0) >= 1) { advanced = true; break; }
      const item = batch.items[0];
      if (item?.runId) {
        const detail = await invoke("get_run_detail", { novelId: novel.id, runId: item.runId });
        if (Array.isArray(detail.legalActions) && detail.legalActions.includes("confirm")) {
          const approve = await $("[data-testid=director-approve-button]");
          if (await approve.isDisplayed()) await approve.click();
        }
      }
      await browser.pause(700);
    }
    expect(advanced).toBe(true);
    await browser.waitUntil(async () => {
      const current = (await invoke("list_batches", { novelId: novel.id }))[0];
      return Boolean(current?.items?.[1]?.runId);
    }, { timeout: 30_000, timeoutMsg: "chapter 2 run did not start" });
    const batch = (await invoke("list_batches", { novelId: novel.id }))[0];
    expect(batch.items[0].status).toBe("succeeded");
    expect(batch.items[1].runId).not.toBeNull();
    const chapter1 = await invoke("get_chapter", { novelId: novel.id, chapterId: batch.items[0].chapterId });
    expect(chapter1.officialVersion).toBeGreaterThanOrEqual(1);
  });
});
