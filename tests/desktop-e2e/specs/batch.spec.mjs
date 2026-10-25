async function invoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Native batch writing", () => {
  it("creates a durable batch for three chapters", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 批量创作", description: "" });
    const chapters = [];
    for (let index = 1; index <= 3; index += 1) {
      chapters.push(await invoke("create_chapter", { novelId: novel.id, title: `第${index}章`, sortOrder: index }));
    }
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await (await $("[data-testid=batch-workspace]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=batch-create]")).click();
    await browser.waitUntil(async () => (await invoke("list_batches", { novelId: novel.id })).length === 1, { timeout: 20_000 });
    const batch = (await invoke("list_batches", { novelId: novel.id }))[0];
    expect(batch.items.length).toBe(3);
    expect(batch.items.every((item) => item.chapterId)).toBe(true);
  });

  it("runs three chapters through existing approval gates and commits three revisions", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 批量全链路", description: "" });
    const chapters = [];
    for (let index = 1; index <= 3; index += 1) {
      chapters.push(await invoke("create_chapter", { novelId: novel.id, title: `第${index}章`, sortOrder: index }));
    }
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await (await $("[data-testid=batch-workspace]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=batch-create]")).click();
    await browser.waitUntil(async () => (await invoke("list_batches", { novelId: novel.id })).length === 1, { timeout: 20_000 });
    let batch = (await invoke("list_batches", { novelId: novel.id }))[0];
    const deadline = Date.now() + 120_000;
    while (Date.now() < deadline && batch.status !== "completed" && batch.status !== "failed") {
      batch = (await invoke("list_batches", { novelId: novel.id }))[0];
      const item = batch.items[batch.cursor] ?? batch.items.find((entry) => entry.status !== "succeeded");
      if (item?.runId) {
        const detail = await invoke("get_run_detail", { novelId: novel.id, runId: item.runId });
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
    expect(batch.items.map((item) => item.runId)).toHaveLength(3);
    expect(new Set(batch.items.map((item) => item.runId)).size).toBe(3);
    for (const chapter of chapters) {
      const detail = await invoke("get_chapter", { novelId: novel.id, chapterId: chapter.id });
      expect(detail.officialVersion).toBeGreaterThanOrEqual(1);
      expect(detail.content.length).toBeGreaterThan(0);
    }
    const full = await invoke("export_novel_selection", { novelId: novel.id, selection: "entire_book", format: "markdown" });
    expect(full).toContain("第1章");
    expect(full).toContain("第3章");
  });

  it("cancels a running batch through the existing run command", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 批量取消", description: "" });
    for (let index = 1; index <= 3; index += 1) {
      await invoke("create_chapter", { novelId: novel.id, title: `第${index}章`, sortOrder: index });
    }
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await (await $("[data-testid=batch-workspace]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=batch-create]")).click();
    await browser.waitUntil(async () => {
      const rows = await invoke("list_batches", { novelId: novel.id });
      return rows.length === 1 && rows[0].items.some((item) => item.runId);
    }, { timeout: 20_000, timeoutMsg: "batch did not create its first run" });
    await (await $("[data-testid=batch-cancel]")).click();
    await browser.waitUntil(async () => (await invoke("list_batches", { novelId: novel.id }))[0].status === "cancelled", { timeout: 20_000 });
    const before = (await invoke("list_batches", { novelId: novel.id }))[0];
    await new Promise((resolve) => setTimeout(resolve, 4000));
    const after = (await invoke("list_batches", { novelId: novel.id }))[0];
    expect(after.status).toBe("cancelled");
    expect(after.items.filter((item) => item.runId).length).toBe(before.items.filter((item) => item.runId).length);
  });
});
