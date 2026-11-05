// P17.4：伏笔生命周期、时间线与剧情因果（真实 Tauri IPC / SQLite / UI）。
//
// 覆盖：第一章埋设、第五章推进、第十章回收；第五章不能提前看到回收；
// 章节改写后证据失效，必须重新选取证据；三个事件的因果关系、循环拒绝与来源失效；
// 时间线筛选与来源跳转。全程不联网、不调用付费模型。
const invoke = (command, args) =>
  browser.execute(
    (cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload),
    command,
    args,
  );

const NOVEL_TITLE = "P17.4 剧情旅程";

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

async function openPlotTab() {
  const tab = await $('[data-testid="panel-tab-plot"]');
  await tab.waitForExist({ timeout: 10000 });
  await tab.click();
  const workspace = await $('[data-testid="plot-workspace"]');
  await workspace.waitForExist({ timeout: 15000 });
  return workspace;
}

async function refreshPlotTab() {
  const other = await $('[data-testid="panel-tab-context"]');
  await other.click();
  await browser.pause(400);
  return openPlotTab();
}

describe("伏笔生命周期与剧情因果（桌面原生）", () => {
  let novelId = "";
  let chapterIds = [];
  let plotId = "p174-plot";

  before(async () => {
    await browser.pause(2500);
    const seeded = await browser.execute(async (title, plot) => {
      const call = window.__TAURI_INTERNALS__.invoke;
      const novels = (await call("list_novels")) || [];
      const existing = novels.find((item) => item.title === title);
      const novel = existing || (await call("create_novel", { title, description: "" }));
      const found = (await call("list_chapters", { novelId: novel.id })) || [];
      const contents = [
        "第一章埋下玉佩。",
        "第二章风雪未停。",
        "第三章山门无事。",
        "第四章旧案未结。",
        "第五章玉佩发光。",
        "第六章有人跟踪。",
        "第七章夜色更深。",
        "第八章线索断开。",
        "第九章回到山门。",
        "第十章玉佩碎裂。",
        "第十一章收起碎片。",
        "第十二章一切结束。",
      ];
      const ids = [];
      for (let order = 1; order <= 12; order += 1) {
        const chapter = found.find((item) => item.sortOrder === order);
        const chapterId = chapter
          ? chapter.id
          : (await call("create_chapter", { novelId: novel.id, title: "第" + order + "章", sortOrder: order }))?.id;
        const current = chapter
          ? chapter
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
      await call("create_plot_thread", {
        request: {
          plotId: plot,
          novelId: novel.id,
          title: "玉佩",
          body: "祖传玉佩里有一道裂痕",
          chapterId: ids[0],
          sourceRevision: 1,
          evidence: "第一章埋下玉佩。",
          commandId: "p174-create-plot",
        },
      });
      await call("transition_plot_thread", {
        request: {
          plotId: plot,
          novelId: novel.id,
          expectedVersion: 1,
          action: "develop",
          chapterId: ids[4],
          sourceRevision: 1,
          evidence: "第五章玉佩发光。",
          commandId: "p174-develop",
        },
      });
      await call("transition_plot_thread", {
        request: {
          plotId: plot,
          novelId: novel.id,
          expectedVersion: 2,
          action: "payoff",
          chapterId: ids[9],
          sourceRevision: 1,
          evidence: "第十章玉佩碎裂。",
          commandId: "p174-payoff",
        },
      });
      return { novelId: novel.id, ids };
    }, NOVEL_TITLE, plotId);
    novelId = seeded.novelId;
    chapterIds = seeded.ids;
  });

  it("第五章只显示推进中的伏笔，第十一章才显示已回收", async () => {
    await openChapter(novelId, chapterIds[4]);
    await openPlotTab();
    const status = await $('[data-testid="plot-status"]');
    expect(await status.getText()).toBe("推进中");
    const timeline = await $('[data-testid="plot-workspace"]');
    const text = await timeline.getText();
    expect(text).toContain("第五章玉佩发光");
    expect(text).not.toContain("第十章玉佩碎裂");

    await openChapter(novelId, chapterIds[10]);
    await openPlotTab();
    expect(await $('[data-testid="plot-status"]').getText()).toBe("已回收");
    const resolvedText = await $('[data-testid="plot-workspace"]').getText();
    expect(resolvedText).toContain("第十章玉佩碎裂");
  });

  it("来源章节改写后必须重新选取证据，旧证据不能直接确认", async () => {
    const chapter = await invoke("get_chapter", { novelId, chapterId: chapterIds[9] });
    await invoke("save_chapter", {
      novelId,
      chapterId: chapterIds[9],
      expectedVersion: chapter.officialVersion,
      title: "第10章",
      content: "第十章已经改写。",
    });

    await openChapter(novelId, chapterIds[10]);
    await refreshPlotTab();
    expect(await $('[data-testid="plot-status"]').getText()).toBe("来源失效");
    const staleText = await $('[data-testid="plot-workspace"]').getText();
    expect(staleText).toContain("生命周期已回收");

    const payoffButton = await $('[data-testid="plot-action-payoff"]');
    await payoffButton.waitForExist({ timeout: 10000 });
    await payoffButton.click();
    await browser.pause(300);
    const evidence = await $('[data-testid="plot-evidence"]');
    await evidence.setValue("第十章玉佩碎裂。");
    await $('[data-testid="plot-transition-submit"]').click();
    await browser.pause(1200);
    expect(await $('[data-testid="plot-status"]').getText()).toBe("来源失效");

    await evidence.setValue("第十一章收起碎片。");
    await $('[data-testid="plot-transition-submit"]').click();
    for (let i = 0; i < 16; i += 1) {
      await browser.pause(500);
      if ((await $('[data-testid="plot-status"]').getText()) === "已回收") break;
    }
    expect(await $('[data-testid="plot-status"]').getText()).toBe("已回收");
  });

  it("时间线展示三个事件，因果关系拒绝循环并在来源失效后标记", async () => {
    for (const [id, order, body] of [
      ["p174-e1", 1, "事件一：得到玉佩"],
      ["p174-e2", 5, "事件二：被追捕"],
      ["p174-e3", 10, "事件三：逃入山门"],
    ]) {
      // 第十章在 Journey 2 已被改写，时间线事件必须记录当前的正式版本，
      // 否则它不能作为可信因果边端点。
      const sourceChapter = await invoke("get_chapter", {
        novelId,
        chapterId: chapterIds[order - 1],
      });
      await invoke("upsert_timeline_event", {
        request: {
          id,
          novelId,
          chapterOrder: order,
          body,
          sourceChapterId: chapterIds[order - 1],
          sourceRevision: String(sourceChapter.officialVersion),
          status: "confirmed",
        },
      });
    }
    const edge = (edgeId, from, to, commandId) =>
      invoke("create_plot_causality", {
        request: {
          edgeId,
          novelId,
          fromKind: "timeline_event",
          fromId: from,
          toKind: "timeline_event",
          toId: to,
          relation: "导致",
          chapterId: chapterIds[0],
          sourceRevision: 1,
          evidence: "第一章埋下玉佩。",
        },
      });
    await edge("p174-edge-1", "p174-e1", "p174-e2", "p174-edge-1");
    await edge("p174-edge-2", "p174-e2", "p174-e3", "p174-edge-2");
    const cycle = await invoke("create_plot_causality", {
      request: {
        edgeId: "p174-edge-cycle",
        novelId,
        fromKind: "timeline_event",
        fromId: "p174-e3",
        toKind: "timeline_event",
        toId: "p174-e1",
        relation: "导致",
        chapterId: chapterIds[0],
        sourceRevision: 1,
        evidence: "第一章埋下玉佩。",
      },
    }).then(() => null, (error) => error);
    expect(cycle).toBeTruthy();

    await openChapter(novelId, chapterIds[10]);
    await refreshPlotTab();
    const kindFilter = await $('[data-testid="plot-timeline-filter-kind"]');
    await kindFilter.selectByAttribute("value", "timeline_event");
    await browser.pause(500);
    const entries = await $$('[data-testid="plot-timeline-entry"]');
    expect(entries.length).toBeGreaterThanOrEqual(3);
    const edges = await $$('[data-testid="plot-causality-edge"]');
    expect(edges.length).toBe(2);
    expect(await edges[0].getText()).toContain("导致");

    // 上游来源章节改写后，边保留但显示来源失效。
    const upstream = await invoke("get_chapter", { novelId, chapterId: chapterIds[0] });
    await invoke("save_chapter", {
      novelId,
      chapterId: chapterIds[0],
      expectedVersion: upstream.officialVersion,
      title: "第1章",
      content: "第一章已经改写。",
    });
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      chapterIds[10],
    );
    await browser.pause(2500);
    await refreshPlotTab();
    const refreshedEdges = await $$('[data-testid="plot-causality-edge"]');
    expect(await refreshedEdges[0].getText()).toContain("来源已失效");

    // 伏笔埋设事件的来源章节被改写：时间线仍保留历史，但必须显示来源失效。
    const kindFilterAfter = await $('[data-testid="plot-timeline-filter-kind"]');
    await kindFilterAfter.selectByAttribute("value", "plot_setup");
    await browser.pause(500);
    const setupEntries = await $$('[data-testid="plot-timeline-entry"]');
    expect(setupEntries.length).toBeGreaterThanOrEqual(1);
    expect(await setupEntries[0].getText()).toContain("来源失效");
  });
});
