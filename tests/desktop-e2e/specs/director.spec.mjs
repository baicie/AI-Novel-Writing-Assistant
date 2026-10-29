import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;

async function tauriInvoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

async function openDirector(novelId) {
  await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novelId);
  await browser.refresh();
  await (await $("[data-tauri-director-page]")).waitForDisplayed({ timeout: 20_000 });
  await browser.waitUntil(async () => (await $("[data-testid=director-start-button]")).isExisting(), {
    timeout: 20_000,
    timeoutMsg: "Director start control missing after navigation",
  });
}

async function createFixture(title) {
  const novel = await tauriInvoke("create_novel", { title, description: "" });
  const chapter = await tauriInvoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
  return { novelId: novel.id, chapterId: chapter.id };
}

async function readRunId() {
  await browser.waitUntil(async () => {
    const meta = await $("[data-testid=director-run-meta]");
    return (await meta.isExisting()) && (await meta.getText()).includes("Run ");
  }, { timeout: 30_000 });
  const text = await (await $("[data-testid=director-run-meta]")).getText();
  const runId = text.match(/Run ([^ ]+)/)?.[1] ?? "";
  expect(runId.length).toBeGreaterThan(0);
  return runId;
}

async function confirmOneGate(novelId, runId, expectedText) {
  await browser.waitUntil(async () => (await $("[data-testid=director-approve-button]")).isExisting(), {
    timeout: 45_000,
    timeoutMsg: `Gate did not appear (${expectedText})`,
  });
  const body = await browser.execute(() => document.body.innerText);
  if (expectedText) expect(body).toContain(expectedText);
  const before = await tauriInvoke("get_run_detail", { novelId, runId });
  await (await $("[data-testid=director-approve-button]")).click();
  await browser.waitUntil(async () => {
    const current = await tauriInvoke("get_run_detail", { novelId, runId });
    return current.controlVersion > before.controlVersion;
  }, { timeout: 30_000, timeoutMsg: "Gate approve did not advance control version" });
}

describe("Biz Novel Studio Native Director", () => {
  it("renders an independent Tauri Director page and start control", async () => {
    const fixture = await createFixture("WDIO 页面书");
    await openDirector(fixture.novelId);
    const start = await $("[data-testid=director-start-button]");
    await start.waitForDisplayed({ timeout: 20_000 });
    expect(await start.isEnabled()).toBe(true);
    expect(fs.existsSync(`${dataDir}/novel.sqlite`)).toBe(true);
  });

  it("completes the full Director flow using real UI buttons and survives refresh", async () => {
    const fixture = await createFixture("WDIO 全流程书");
    await openDirector(fixture.novelId);
    const start = await $("[data-testid=director-start-button]");
    await start.waitForDisplayed({ timeout: 20_000 });
    await start.click();
    const runId = await readRunId();

    await confirmOneGate(fixture.novelId, runId, "林动");
    await confirmOneGate(fixture.novelId, runId, "山门卷");
    await confirmOneGate(fixture.novelId, runId, "E2E 候选正文");

    await browser.waitUntil(async () => {
      const current = await tauriInvoke("get_run_detail", { novelId: fixture.novelId, runId });
      return current.status === "succeeded";
    }, { timeout: 45_000, timeoutMsg: "Director run did not succeed" });

    const chapter = await tauriInvoke("get_chapter", { novelId: fixture.novelId, chapterId: fixture.chapterId });
    expect(chapter.content).toContain("E2E 候选正文");
    expect(chapter.officialVersion).toBeGreaterThanOrEqual(1);
    fs.writeFileSync(`${dataDir}/e2e-state.json`, JSON.stringify({ ...fixture, runId }));

    await browser.refresh();
    await (await $("[data-tauri-director-page]")).waitForDisplayed({ timeout: 20_000 });
    await browser.waitUntil(async () => {
      const status = await $("[data-testid=director-status]");
      return (await status.isExisting()) && (await status.getText()).includes("已完成");
    }, { timeout: 20_000, timeoutMsg: "Completed Run was not restored after refresh" });
    expect(await browser.execute(() => document.body.innerText)).toContain("运行历史");
    const reopened = await tauriInvoke("get_chapter", { novelId: fixture.novelId, chapterId: fixture.chapterId });
    expect(reopened.content).toBe(chapter.content);
  });
});

  it("rejects and regenerates a gated candidate while retaining history", async () => {
    const fixture = await createFixture("WDIO 拒绝重生成书");
    await openDirector(fixture.novelId);
    await (await $("[data-testid=director-start-button]")).click();
    const runId = await readRunId();
    await (await $("[data-testid=director-reject-button]")).waitForDisplayed({ timeout: 45_000 });
    const before = await tauriInvoke("get_run_detail", { novelId: fixture.novelId, runId });
    await (await $("[data-testid=director-reject-button]")).click();
    await browser.waitUntil(async () => {
      const current = await tauriInvoke("get_run_detail", { novelId: fixture.novelId, runId });
      return current.controlVersion > before.controlVersion;
    }, { timeout: 30_000 });
    await (await $("[data-testid=director-regenerate-button]")).waitForDisplayed({ timeout: 45_000 });
    const second = await tauriInvoke("get_run_detail", { novelId: fixture.novelId, runId });
    await (await $("[data-testid=director-regenerate-button]")).click();
    await browser.waitUntil(async () => {
      const current = await tauriInvoke("get_run_detail", { novelId: fixture.novelId, runId });
      return current.controlVersion > second.controlVersion;
    }, { timeout: 30_000 });
    const detail = await tauriInvoke("get_run_detail", { novelId: fixture.novelId, runId });
    const castArtifacts = detail.artifacts.filter(artifact => artifact.artifactType === "character_cast");
    expect(castArtifacts.length).toBeGreaterThanOrEqual(2);
    expect(castArtifacts.some(artifact => artifact.status === "stale")).toBe(true);
    await tauriInvoke("cancel_run", { commandId: `wdio-decision-cancel-${Date.now()}`, runId });
  });

  it("edits a candidate, confirms after edit, commits it, and preserves the original candidate", async () => {
    const fixture = await createFixture("WDIO 候选编辑书");
    await openDirector(fixture.novelId);
    await (await $("[data-testid=director-start-button]")).click();
    const runId = await readRunId();
    await confirmOneGate(fixture.novelId, runId, "林动");
    await confirmOneGate(fixture.novelId, runId, "山门卷");
    await (await $("[data-testid=director-approve-button]")).waitForDisplayed({ timeout: 45_000 });
    await (await $("[data-testid=director-edit-candidate-button]")).click();
    const input = await $("[data-testid=director-candidate-edit-input]");
    await input.waitForDisplayed({ timeout: 20_000 });
    const edited = "WDIO 作者修改后的候选正文：雨停之后才入山。";
    await input.clearValue();
    await input.addValue(edited);
    await (await $("[data-testid=director-confirm-candidate-edit]")).click();
    await browser.waitUntil(async () => {
      const current = await tauriInvoke("get_run_detail", { novelId: fixture.novelId, runId });
      return current.status === "succeeded";
    }, { timeout: 45_000, timeoutMsg: "author-edited candidate did not commit" });
    const chapter = await tauriInvoke("get_chapter", { novelId: fixture.novelId, chapterId: fixture.chapterId });
    expect(chapter.content).toBe(edited);
    const detail = await tauriInvoke("get_run_detail", { novelId: fixture.novelId, runId });
    const approval = detail.artifacts.filter(artifact => artifact.artifactType === "chapter_approval");
    expect(approval.some(artifact => artifact.status === "user_edited")).toBe(true);
    expect(detail.latestCandidate.content).toBe(edited);
    expect(chapter.officialVersion).toBeGreaterThanOrEqual(1);
    fs.writeFileSync(`${dataDir}/confirm-edit-state.json`, JSON.stringify({ ...fixture, runId, edited }));
  });
