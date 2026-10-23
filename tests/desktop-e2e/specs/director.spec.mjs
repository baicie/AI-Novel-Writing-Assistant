import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;

async function tauriInvoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Biz Novel Studio Native Director", () => {
  let novelId = "";
  let chapterId = "";
  let runId = "";
  it("renders the Tauri Director page and a real start control", async () => {
    const novel = await tauriInvoke("create_novel", { title: "WDIO 导演书", description: "" });
    novelId = novel.id;
    const chapter = await tauriInvoke("create_chapter", { novelId, title: "第一章", sortOrder: 1 });
    chapterId = chapter.id;
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novelId);
    const page = await $("[data-tauri-director-page]");
    await page.waitForDisplayed({ timeout: 20_000 });
    const start = await $("[data-testid=director-start-button]");
    await start.waitForDisplayed({ timeout: 20_000 });
    expect(await start.isEnabled()).toBe(true);
    expect(fs.existsSync(`${dataDir}/novel.sqlite`)).toBe(true);
  });

  it("starts a Rust Run through the actual start button", async () => {
    const start = await $("[data-testid=director-start-button]");
    await start.click();
    await browser.waitUntil(async () => {
      const status = await $("[data-testid=director-status]");
      const text = await status.getText();
      return !text.includes("queued") && !text.includes("排队");
    }, { timeout: 30_000, timeoutMsg: "Director Run did not leave queued state" });
    const meta = await $("[data-testid=director-run-meta]");
    const metaText = await meta.getText();
    expect(metaText).toContain("Run ");
    runId = metaText.match(/Run ([^ ]+)/)?.[1] ?? "";
    expect(runId.length).toBeGreaterThan(0);
  });

  it("approves the real Character Cast gate through the UI", async () => {
    await browser.waitUntil(async () => (await $("[data-testid=director-approve-button]")).isExisting(), { timeout: 30_000, timeoutMsg: "Character gate did not appear" });
    const body = await browser.execute(() => document.body.innerText);
    expect(body).toContain("林动");
    expect(body).toContain("苏晚");
    expect(body).toContain("陆沉");
    const before = await tauriInvoke("get_run_detail", { novelId, runId });
    await (await $("[data-testid=director-approve-button]")).click();
    await browser.waitUntil(async () => {
      const current = await tauriInvoke("get_run_detail", { novelId, runId });
      return current.controlVersion > before.controlVersion;
    }, { timeout: 30_000, timeoutMsg: "Character gate approve did not advance control version" });
  });

  it("approves the real Volume Strategy gate through the UI", async () => {
    await browser.waitUntil(async () => (await $("[data-testid=director-approve-button]")).isExisting(), { timeout: 30_000, timeoutMsg: "Volume gate did not appear" });
    const body = await browser.execute(() => document.body.innerText);
    expect(body).toContain("山门卷");
    const before = await tauriInvoke("get_run_detail", { novelId, runId });
    await (await $("[data-testid=director-approve-button]")).click();
    await browser.waitUntil(async () => {
      const current = await tauriInvoke("get_run_detail", { novelId, runId });
      return current.controlVersion > before.controlVersion;
    }, { timeout: 30_000, timeoutMsg: "Volume approve did not advance control version" });
  });

  it("approves the chapter and commits official text", async () => {
    await browser.waitUntil(async () => (await $("[data-testid=director-approve-button]")).isExisting(), { timeout: 45_000, timeoutMsg: "Chapter gate did not appear" });
    const body = await browser.execute(() => document.body.innerText);
    expect(body).toContain("E2E 候选正文");
    const before = await tauriInvoke("get_run_detail", { novelId, runId });
    await (await $("[data-testid=director-approve-button]")).click();
    await browser.waitUntil(async () => {
      const current = await tauriInvoke("get_run_detail", { novelId, runId });
      return current.status === "succeeded" && current.controlVersion > before.controlVersion;
    }, { timeout: 45_000, timeoutMsg: "Chapter approve did not reach succeeded" });
    const chapter = await tauriInvoke("get_chapter", { novelId, chapterId });
    expect(chapter.content).toContain("E2E 候选正文");
    expect(chapter.officialVersion).toBeGreaterThanOrEqual(1);
  });

  it("reloads the UI and restores saved state from SQLite", async () => {
    if (dataDir) {
      fs.writeFileSync(`${dataDir}/e2e-state.json`, JSON.stringify({ novelId, chapterId, runId }));
    }
    await browser.refresh();
    await browser.waitUntil(async () => (await $("[data-tauri-director-page]")).isExisting(), { timeout: 20_000, timeoutMsg: "Director page did not reload" });
    const body = await browser.execute(() => document.body.innerText);
    expect(body).toContain("运行历史");
    const chapter = await tauriInvoke("get_chapter", { novelId, chapterId });
    expect(chapter.content).toContain("E2E 候选正文");
  });
});
