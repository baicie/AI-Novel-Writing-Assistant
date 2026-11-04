// P17.3：章节连续性纵向旅程（真实 Tauri IPC / SQLite / UI / 本地 Mock 记忆抽取）。
//
// 覆盖：As-Of 不泄露未来事实、来源章节与证据、事实候选人工确认、冲突裁决、
// 章节版本失效后的重新确认、上下文预览在必要来源缺失时明确阻断。
// 全程不联网、不调用付费模型。
import http from "node:http";

const invoke = (command, args) =>
  browser.execute(
    (cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload),
    command,
    args,
  );

const NOVEL_TITLE = "P17.3 连续性旅程";

async function openChapter(novelId, chapterId) {
  await browser.execute(
    (nid, cid) => {
      window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
    },
    novelId,
    chapterId,
  );
  await browser.pause(2500);
}

async function openCastTab() {
  const tab = await $('[data-testid="panel-tab-cast"]');
  await tab.waitForExist({ timeout: 10000 });
  await tab.click();
  await browser.pause(1200);
}

async function refreshCastTab() {
  const contextTab = await $('[data-testid="panel-tab-context"]');
  await contextTab.click();
  await browser.pause(600);
  await openCastTab();
}

async function panelText() {
  const panel = await $('[data-testid="cast-world-tab"]');
  await panel.waitForExist({ timeout: 15000 });
  return panel.getText();
}

describe("章节连续性（桌面原生）", () => {
  let memoryServer;
  let memoryPort;
  let novelId = "";
  let chapterIds = [];
  const heroId = "p173-hero";

  before(async () => {
    memoryServer = http.createServer((req, res) => {
      let body = "";
      req.on("data", (chunk) => (body += chunk));
      req.on("end", () => {
        const content = JSON.stringify({
          facts: [
            {
              factType: "character_state",
              truthfulness: "narrator_fact",
              entityName: "林动",
              value: "状态：受伤",
              evidenceQuote: "他按住伤口",
              confidence: 0.92,
            },
          ],
        });
        const event = JSON.stringify({ choices: [{ delta: { content } }] });
        res.writeHead(200, { "content-type": "text/event-stream" });
        res.end("data: " + event + "\n\ndata: [DONE]\n\n");
      });
    });
    await new Promise((resolve) => memoryServer.listen(0, "127.0.0.1", resolve));
    memoryPort = memoryServer.address().port;

    await browser.pause(2500);
    const seeded = await browser.execute(async (title, hero) => {
      const call = window.__TAURI_INTERNALS__.invoke;
      const novels = (await call("list_novels")) || [];
      const existing = novels.find((item) => item.title === title);
      const novel = existing || (await call("create_novel", { title, description: "" }));
      const existingChapters = (await call("list_chapters", { novelId: novel.id })) || [];
      const contents = [
        "他推开门，屋里没有人。\n\n林动还活着，只是脸色发白。",
        "他按住伤口，靠在门边。\n\n山门在夜色里亮着灯。",
        "第三天，他回到山门。",
        "风雪更大了。",
        "多年后，人们说林动已经死了。",
      ];
      const ids = [];
      for (let order = 1; order <= 5; order += 1) {
        const found = existingChapters.find((item) => item.sortOrder === order);
        const chapterId = found
          ? found.id
          : (await call("create_chapter", {
              novelId: novel.id,
              title: "第" + order + "章",
              sortOrder: order,
            }))?.id;
        const current = found
          ? found
          : ((await call("list_chapters", { novelId: novel.id })) || []).find((item) => item.id === chapterId);
        await call("save_chapter", {
          novelId: novel.id,
          chapterId,
          expectedVersion: current?.officialVersion ?? 0,
          title: "第" + order + "章",
          content: contents[order - 1],
        });
        ids.push(chapterId);
      }
      const characters = (await call("list_characters", { novelId: novel.id })) || [];
      if (!characters.some((item) => item.id === hero)) {
        await call("create_character", {
          request: { id: hero, novelId: novel.id, name: "林动", role: "主角" },
        });
      }
      return { novelId: novel.id, chapterIds: ids };
    }, NOVEL_TITLE, heroId);
    novelId = seeded.novelId;
    chapterIds = seeded.chapterIds;

    const confirm = (id, factType, valueJson, chapterIndex, evidence, effectiveOrder) =>
      invoke("confirm_structured_fact", {
        fact: {
          id,
          novelId,
          entityId: heroId,
          factType,
          valueJson,
          sourceChapterId: chapterIds[chapterIndex],
          sourceRevision: 1,
          evidenceQuote: evidence,
          effectiveChapterOrder: effectiveOrder,
          nowMs: Date.now(),
        },
      });
    await confirm("p173-alive", "life_state", '{"state":"alive"}', 0, "林动还活着", 1);
    await confirm("p173-location", "location", '{"location":"山门"}', 1, "山门在夜色里亮着灯", 2);
    await confirm("p173-dead", "life_state", '{"state":"dead"}', 4, "林动已经死了", 5);
  });

  after(async () => {
    if (memoryServer) {
      await new Promise((resolve) => memoryServer.close(resolve));
    }
  });

  it("第 3 章只显示此前有效事实，并给出可跳转的来源与证据", async () => {
    await openChapter(novelId, chapterIds[2]);
    await openCastTab();
    const text = await panelText();
    expect(text).toContain("截至第 3 章");
    expect(text).toContain("存活");
    expect(text).toContain("山门");
    expect(text).not.toContain("死亡");
    expect(text).toContain("证据：林动还活着");

    const links = await $$('[data-testid="continuity-source-link"]');
    expect(links.length).toBeGreaterThan(0);
    await links[0].click();
    await browser.pause(2000);
    const hash = await browser.execute(() => window.location.hash);
    expect(hash).toContain(chapterIds[1]);
  });

  it("第 4 章仍是存活，第 5 章才出现死亡事实", async () => {
    await openChapter(novelId, chapterIds[3]);
    await openCastTab();
    const before = await panelText();
    expect(before).toContain("存活");
    expect(before).not.toContain("死亡");

    await openChapter(novelId, chapterIds[4]);
    await openCastTab();
    const after = await panelText();
    expect(after).toContain("死亡");
    expect(after).not.toContain("存活");
  });

  it("记忆候选必须由作者确认后才进入正式人物状态", async () => {
    await invoke("extract_memory", {
      request: {
        commandId: "p173-memory-1",
        novelId,
        chapterId: chapterIds[1],
        provider: "openai-compatible",
        model: "p173-mock",
        endpoint: "http://127.0.0.1:" + memoryPort + "/v1",
        allowLocalhostHttp: true,
        tokenBudget: 800,
      },
    });

    await openChapter(novelId, chapterIds[1]);
    await openCastTab();
    await refreshCastTab();
    const candidate = await $('[data-testid="continuity-candidate"]');
    await candidate.waitForExist({ timeout: 15000 });
    const candidateText = await candidate.getText();
    expect(candidateText).toContain("状态：受伤");
    expect(candidateText).toContain("证据：他按住伤口");

    await $('[data-testid="continuity-candidate-confirm"]').click();
    for (let i = 0; i < 12; i += 1) {
      await browser.pause(500);
      if (!(await $('[data-testid="continuity-candidate"]').isExisting())) break;
    }
    const text = await panelText();
    expect(text).toContain("状态：受伤");
    expect(await $('[data-testid="continuity-candidate"]').isExisting()).toBe(false);
  });

  it("冲突展示双方来源，裁决保留原事实", async () => {
    await invoke("confirm_structured_fact", {
      fact: {
        id: "p173-conflicting-location",
        novelId,
        entityId: heroId,
        factType: "location",
        valueJson: '{"location":"别处"}',
        sourceChapterId: chapterIds[1],
        sourceRevision: 1,
        evidenceQuote: "山门在夜色里亮着灯",
        effectiveChapterOrder: 2,
        nowMs: Date.now(),
      },
    });

    await refreshCastTab();
    const conflict = await $('[data-testid="continuity-conflict"]');
    await conflict.waitForExist({ timeout: 15000 });
    const conflictText = await conflict.getText();
    expect(conflictText).toContain("山门");
    expect(conflictText).toContain("别处");
    const sides = await $$('[data-testid="continuity-conflict-side"]');
    expect(sides.length).toBeGreaterThanOrEqual(2);

    const keep = await $('[data-testid="continuity-conflict-keep"]');
    await keep.waitForExist({ timeout: 10000 });
    await keep.click();
    for (let i = 0; i < 12; i += 1) {
      await browser.pause(500);
      if (!(await $('[data-testid="continuity-conflict"]').isExisting())) break;
    }
    expect(await $('[data-testid="continuity-conflict"]').isExisting()).toBe(false);
    const text = await panelText();
    expect(text).toContain("山门");
    expect(text).not.toContain("别处");
  });

  it("来源章节产生新版本后事实标记过期，作者可重新确认", async () => {
    const chapter = await invoke("get_chapter", { novelId, chapterId: chapterIds[1] });
    await invoke("save_chapter", {
      novelId,
      chapterId: chapterIds[1],
      expectedVersion: chapter.officialVersion,
      title: "第2章",
      content: "山门在夜色里亮着灯。\n\n他按住伤口，抬头看见新的旗帜。",
    });

    await refreshCastTab();
    const stale = await $('[data-testid="continuity-stale"]');
    await stale.waitForExist({ timeout: 15000 });
    expect(await stale.getText()).toContain("不能继续作为有效证据");
    await $('[data-testid="continuity-reconfirm"]').click();
    for (let i = 0; i < 12; i += 1) {
      await browser.pause(500);
      if (!(await $('[data-testid="continuity-stale"]').isExisting())) break;
    }
    expect(await $('[data-testid="continuity-stale"]').isExisting()).toBe(false);
    const text = await panelText();
    expect(text).toContain("山门");
  });

  it("上下文预览在必要来源缺失时明确阻断，而不是假装没有资料", async () => {
    const tab = await $('[data-testid="panel-tab-context"]');
    await tab.click();
    const blocked = await $('[data-testid="context-blocked"]');
    await blocked.waitForExist({ timeout: 15000 });
    const text = await blocked.getText();
    expect(text).toContain("本次生成被阻断");
    expect(text).toContain("story_macro");
    expect(await $('[data-testid="context-preview"]').isExisting()).toBe(true);
  });
});
