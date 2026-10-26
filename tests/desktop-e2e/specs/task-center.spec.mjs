async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native task center", () => {
  it("projects a batch task and pauses it through the real task center", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 任务中心", description: "" });
    for (let index = 1; index <= 2; index += 1) {
      await invoke("create_chapter", { novelId: novel.id, title: `第${index}章`, sortOrder: index });
    }
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await (await $("[data-testid=batch-workspace]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=batch-create]")).click();
    await browser.waitUntil(async () => (await invoke("list_batches", { novelId: novel.id })).length === 1, { timeout: 20_000 });

    await browser.execute(() => { window.location.hash = "#/tasks"; });
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("连续创作"), { timeout: 20_000 });
    const body = await browser.execute(() => document.body.innerText);
    expect(body).toContain("连续创作");
    expect(body).toContain("任务");

    const pause = await $("[data-testid=task-pause]");
    await pause.waitForDisplayed({ timeout: 20_000 });
    await pause.click();
    await browser.waitUntil(async () => (await invoke("list_batches", { novelId: novel.id }))[0].status === "paused", { timeout: 20_000 });
    const project = await invoke("list_native_tasks", { novelId: novel.id });
    expect(project.some((task) => task.taskType === "batch")).toBe(true);
  });
});
