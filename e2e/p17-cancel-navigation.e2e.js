// P17.0.1：未保存正文时点击「待处理」必须被拦住；确认后才切章并打开正确的运行页签。
describe("章节导航取消与运行关联", () => {
  let novelId = "";
  let chapterOneId = "";
  let chapterTwoId = "";

  before(async () => {
    await browser.pause(2500);
    const seeded = await browser.execute(async () => {
      const call = window.__TAURI_INTERNALS__.invoke;
      const novels = await call("list_novels");
      let novel = (novels || []).find((item) => item.title === "P17 取消导航书");
      if (!novel) {
        novel = await call("create_novel", { title: "P17 取消导航书", description: "" });
      }
      const chapters = (await call("list_chapters", { novelId: novel.id })) || [];
      const ensure = async (title, sortOrder) => {
        const found = chapters.find((item) => item.sortOrder === sortOrder);
        if (found) return found.id;
        const created = await call("create_chapter", { novelId: novel.id, title, sortOrder });
        return created.id ?? created.chapterId;
      };
      const one = await ensure("第一章", 1);
      const two = await ensure("第二章", 2);

      // 一个必然失败的一次性 provider：用来产出真实的 failed Run（触发「待处理」标记）。
      const profiles = await call("list_provider_profiles");
      const existing = (profiles || []).find((item) => item.id === "p17-dead");
      await call("upsert_provider_profile", {
        request: {
          providerId: "p17-dead",
          displayName: "P17 故障网关",
          protocol: "openai-compatible",
          baseUrl: "http://127.0.0.1:9/v1",
          allowLocalhostHttp: true,
          confirmPlaintextHostChange: true,
          sortOrder: 9,
          isBuiltin: false,
          credential: { kind: "replace", apiKey: "sk-p17-dead" },
          expectedRevision: existing ? existing.revision : null,
        },
      });
      const models = (await call("list_provider_models", { providerId: "p17-dead" })) || [];
      if (!models.some((item) => item.modelId === "dead-model")) {
        await call("upsert_provider_model", {
          request: { providerId: "p17-dead", modelId: "dead-model", expectedRevision: null },
        });
      }
      for (const taskType of ["planner", "writer", "review", "repair"]) {
        await call("upsert_model_binding", {
          request: { taskType, providerId: "p17-dead", modelId: "dead-model", expectedRevision: null },
        });
      }
      const runs = (await call("list_runs", { novelId: novel.id })) || [];
      const existingRun = runs.find((run) => run.chapterId === one);
      if (!existingRun) {
        await call("open_run", {
          request: {
            commandId: "p17-cancel-open-" + Date.now(),
            novelId: novel.id,
            targetChapterId: one,
            provider: "p17-dead",
            model: "dead-model",
            promptVersions: { chapter_draft: "chapter-draft-v1" },
            tokenBudget: 1024,
            contextTokenBudget: null,
            maxAutomaticRetries: 0,
            repairBudget: 1,
            endpoint: "http://127.0.0.1:9/v1/chat/completions",
            allowLocalhostHttp: true,
          },
        });
      }
      return { novelId: novel.id, one, two };
    });
    novelId = seeded.novelId;
    chapterOneId = seeded.one;
    chapterTwoId = seeded.two;

    // 等后台 worker 把这一章跑到需要人处理的状态，才有「待处理」标记。
    for (let i = 0; i < 40; i += 1) {
      await browser.pause(700);
      const state = await browser.execute(async (nid, cid) => {
        const runs = (await window.__TAURI_INTERNALS__.invoke("list_runs", { novelId: nid })) || [];
        const run = runs.filter((item) => item.chapterId === cid).sort((a, b) => b.updatedAt - a.updatedAt)[0];
        return run ? run.state : "none";
      }, novelId, chapterOneId);
      if (state === "failed" || state === "paused" || state === "waiting_approval") break;
    }
  });

  it("拒绝切换时章节、页签与未保存正文都不变；确认后才打开正确的运行", async () => {
    // 先到第二章，并制造真实的未保存状态。
    await browser.execute((nid, cid) => {
      window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
    }, novelId, chapterTwoId);
    await browser.pause(3500);

    // 只点第一章自己的「待处理」，避免误点到当前章的徽标。
    const chapterOneBadge = () =>
      $(`[data-testid="chapter-run-action"][data-chapter-id="${chapterOneId}"]`);
    const badge = await chapterOneBadge();
    expect(await badge.isExisting()).toBe(true);

    await browser.execute(() => {
      const surface = document.querySelector('[data-testid="chapter-editor-surface"]');
      const editable = (surface && surface.querySelector('[contenteditable="true"]')) || document.querySelector('[contenteditable="true"]');
      if (editable) {
        editable.focus();
        document.execCommand("insertText", false, "未保存片段");
      }
    });
    await browser.pause(1200);
    const dirtyBefore = await browser.execute(
      () => document.querySelector("[data-local-draft-dirty]")?.getAttribute("data-local-draft-dirty")
    );
    expect(dirtyBefore).toBe("true");

    const hashBefore = await browser.execute(() => window.location.hash);
    const tabBefore = await browser.execute(
      () => document.querySelector('[data-testid="panel-tab-run"]')?.getAttribute("data-active")
    );
    expect(tabBefore).toBe("false");

    // 拒绝确认：一切保持原样。
    await browser.execute(() => {
      window.confirm = () => false;
    });
    await badge.click();
    await browser.pause(1200);


    expect(await browser.execute(() => window.location.hash)).toBe(hashBefore);
    expect(
      await browser.execute(() => document.querySelector('[data-testid="panel-tab-run"]')?.getAttribute("data-active"))
    ).toBe("false");
    expect(
      await browser.execute(() => document.querySelector("[data-local-draft-dirty]")?.getAttribute("data-local-draft-dirty"))
    ).toBe("true");

    // 同意确认：切到第一章并打开运行与恢复页签，显示第一章自己的 Run。
    await browser.execute(() => {
      window.confirm = () => true;
    });
    const badgeAgain = await chapterOneBadge();
    await badgeAgain.click();
    await browser.pause(3000);

    const hashAfter = await browser.execute(() => window.location.hash);
    expect(hashAfter).toContain(chapterOneId);
    expect(
      await browser.execute(() => document.querySelector('[data-testid="panel-tab-run"]')?.getAttribute("data-active"))
    ).toBe("true");

    const statusText = await browser.execute(
      () => document.querySelector('[data-testid="run-status"]')?.textContent?.trim() ?? ""
    );
    expect(statusText.length).toBeGreaterThan(0);

    await browser.execute(() => {
      delete window.confirm;
    });
  });
});
