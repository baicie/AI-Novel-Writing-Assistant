async function invoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Native batch bounded repair", () => {
  it("repairs once, stops at the quality ceiling, and continues only after each reminder is handled", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 批量修复", description: "" });
    for (let index = 1; index <= 3; index += 1) {
      await invoke("create_chapter", { novelId: novel.id, title: `第${index}章`, sortOrder: index });
    }
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await (await $("[data-testid=batch-workspace]")).waitForDisplayed({ timeout: 20_000 });
    const budget = await $("[data-testid=batch-repair-budget]");
    expect(await budget.getValue()).toBe("1");
    await (await $("[data-testid=batch-create]")).click();

    let runId = "";
    let reachedQualityCeiling = false;
    const deadline = Date.now() + 150_000;
    while (Date.now() < deadline) {
      const batch = (await invoke("list_batches", { novelId: novel.id }))[0];
      runId = batch?.items?.[0]?.runId ?? "";
      if (runId) {
        const detail = await invoke("get_run_detail", { novelId: novel.id, runId });
        if (detail.status === "paused" && detail.pauseSignal === "safety") {
          reachedQualityCeiling = true;
          break;
        }
        const gateTypes = detail.gate?.artifactTypes ?? [];
        if (Array.isArray(detail.legalActions) && detail.legalActions.includes("confirm") && !gateTypes.includes("chapter_approval")) {
          const approve = await $("[data-testid=director-approve-button]");
          if (await approve.isExisting()) await approve.click();
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 700));
    }
    expect(reachedQualityCeiling).toBe(true);

    const stopped = await invoke("get_run_detail", { novelId: novel.id, runId });
    expect(stopped.pauseKind).toBe("safety");
    expect(stopped.artifacts.filter((item) => item.artifactType === "repair_notes").length).toBe(1);
    expect(stopped.artifacts.some((item) => item.payloadJson.includes("repair_candidate_created"))).toBe(true);
    expect(stopped.artifacts.filter((item) => item.artifactType === "consistency_review").length).toBeGreaterThanOrEqual(2);
    expect(stopped.artifacts.some((item) => item.artifactType === "chapter_commit")).toBe(false);
    expect(stopped.legalActions).toContain("cancel");
    expect(stopped.legalActions).not.toContain("resume");
    expect(stopped.legalActions).not.toContain("resume_quality_debt");
    expect(await (await $("[data-testid=director-quality-debt-hint]")).isExisting()).toBe(true);

    for (let guard = 0; guard < 8; guard += 1) {
      const open = await $$("[data-testid=director-accept-quality-debt]");
      let clicked = false;
      for (const button of open) {
        if (await button.isExisting()) {
          await button.click();
          clicked = true;
          await browser.pause(600);
          break;
        }
      }
      if (!clicked) break;
    }

    const continueButton = await $("[data-testid=director-resume-quality-debt]");
    await continueButton.waitForDisplayed({ timeout: 30_000 });
    await continueButton.click();

    await browser.waitUntil(async () => {
      const current = await invoke("get_run_detail", { novelId: novel.id, runId });
      return current.status === "waiting_approval";
    }, { timeout: 90_000, timeoutMsg: "run did not reach the approval gate" });
    const beforeApprove = await invoke("get_chapter", { novelId: novel.id, chapterId: stopped.chapterId });
    expect(beforeApprove.content).toBe("");
    expect(beforeApprove.officialVersion).toBe(0);

    const approve = await $("[data-testid=director-approve-button]");
    await approve.waitForDisplayed({ timeout: 30_000 });
    await approve.click();
    await browser.waitUntil(async () => {
      const chapter = await invoke("get_chapter", { novelId: novel.id, chapterId: stopped.chapterId });
      return chapter.officialVersion >= 1;
    }, { timeout: 90_000, timeoutMsg: "the confirmed chapter was not committed" });
    const committed = await invoke("get_chapter", { novelId: novel.id, chapterId: stopped.chapterId });
    expect(committed.content.length).toBeGreaterThan(0);

    const advanceDeadline = Date.now() + 90_000;
    let chapterTwoRun = false;
    while (Date.now() < advanceDeadline) {
      const current = (await invoke("list_batches", { novelId: novel.id }))[0];
      if (current?.items?.[1]?.runId) {
        chapterTwoRun = true;
        break;
      }
      if (current?.status === "paused") {
        const resumeBatchButton = await $("[data-testid=batch-resume]");
        if (await resumeBatchButton.isExisting() && await resumeBatchButton.isEnabled()) {
          await resumeBatchButton.click();
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 700));
    }
    expect(chapterTwoRun).toBe(true);
    const batch = (await invoke("list_batches", { novelId: novel.id }))[0];
    expect(batch.items[0].status).toBe("succeeded");
    expect(batch.items[1].runId).not.toBeNull();
  });
});
