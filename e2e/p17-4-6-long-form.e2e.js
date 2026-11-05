// P17.4.6：40 章连续故事验收。数据通过真实桌面应用 IPC 按作者顺序产生，
// 不直接写数据库；随后在真实章节工作台检查 As-Of、伏笔与因果状态。
const invoke = (command, args) =>
  browser.execute(
    (cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload),
    command,
    args,
  );

async function openPlotTab() {
  const tab = await $('[data-testid="panel-tab-plot"]');
  await tab.waitForExist({ timeout: 15000 });
  await tab.click();
  const workspace = await $('[data-testid="plot-workspace"]');
  await workspace.waitForExist({ timeout: 15000 });
  return workspace;
}

describe("P17.4.6 40 章长篇创作验收", () => {
  let novelId = "";
  let chapters = [];
  let life = null;

  it("按作者顺序建立 40 章故事、人物、伏笔、时间线与因果", async () => {
    await browser.pause(2500);
    const title = "P17.4.6 长篇验收 " + Date.now();
    life = await browser.execute(async (novelTitle) => {
      try {
      let stage = "novel";
      const call = window.__TAURI_INTERNALS__.invoke;
      const novel = await call("create_novel_with_setup", {
        request: {
          title: novelTitle,
          description: "40 章连续故事验收",
          creationExperience: "professional",
          narrativeForm: "long_novel",
          projectMode: "co_pilot",
          estimatedChapterCount: 40,
          defaultChapterLength: 1200,
        },
      });
      const nid = novel.id;
      stage = "characters";
      const chars = [];
      for (const [name, role] of [
        ["林动", "主角"], ["苏晚", "同伴"], ["陆沉", "对手"],
        ["阿福", "导师"], ["沈青", "盟友"], ["黑衣人", "反派"],
      ]) {
        chars.push((await call("create_character", {
          request: { id: `p1746-char-${chars.length}`, novelId: nid, name, role },
        })).id);
      }
      const ids = [];
      for (let order = 1; order <= 40; order += 1) {
        stage = "chapter-" + order;
        const chapter = await call("create_chapter", {
          novelId: nid,
          title: `第${order}章`,
          sortOrder: order,
        });
        const content = `第${order}章：林动在城中行动，线索${order}出现，同伴继续追踪。`;
        await call("save_chapter", {
          novelId: nid,
          chapterId: chapter.id,
          expectedVersion: 0,
          title: `第${order}章`,
          content,
        });
        ids.push(chapter.id);
        if ([1, 5, 10, 15, 20].includes(order)) {
          await call("upsert_character_state", {
            request: {
              id: `p1746-state-${order}`,
              characterId: chars[0],
              chapterOrder: order,
              body: `林动在第${order}章的位置与状态`,
              sourceChapterId: chapter.id,
              sourceRevision: "1",
              status: "confirmed",
            },
          });
        }
        if ([1, 5, 10, 15, 25].includes(order)) {
          await call("upsert_timeline_event", {
            request: {
              id: `p1746-event-${order}`,
              novelId: nid,
              chapterOrder: order,
              body: `事件${order}：线索${order}出现`,
              sourceChapterId: chapter.id,
              sourceRevision: "1",
              status: "confirmed",
            },
          });
        }
      }
      stage = "plots";
      for (let i = 1; i <= 11; i += 1) {
        const order = i + 2;
        await call("create_plot_thread", {
          request: {
            plotId: `p1746-plot-${i}`,
            novelId: nid,
            title: `伏笔${i}`,
            body: `第${order}章埋下的伏笔${i}`,
            chapterId: ids[order - 1],
            sourceRevision: 1,
            evidence: `线索${order}出现`,
            commandId: `p1746-plot-create-${i}`,
          },
        });
      }
      stage = "plot1-develop";
      await call("transition_plot_thread", {
        request: {
          plotId: "p1746-plot-1", novelId: nid, expectedVersion: 1,
          action: "develop", chapterId: ids[7], sourceRevision: 1,
          evidence: "线索8出现", commandId: "p1746-plot-1-develop",
        },
      });
      stage = "plot1-develop";
      stage = "plot1-defer";
      await call("transition_plot_thread", {
        request: {
          plotId: "p1746-plot-1", novelId: nid, expectedVersion: 2,
          action: "defer", chapterId: ids[14], sourceRevision: 1,
          evidence: "线索15出现", commandId: "p1746-plot-1-defer",
        },
      });
      stage = "plot1-develop";
      stage = "plot1-payoff";
      await call("transition_plot_thread", {
        request: {
          plotId: "p1746-plot-1", novelId: nid, expectedVersion: 3,
          action: "payoff", chapterId: ids[24], sourceRevision: 1,
          evidence: "线索25出现", commandId: "p1746-plot-1-payoff",
        },
      });
      // 因果关系：A -> B -> C -> D，D -> A 必须被拒绝。
      stage = "causality";
      const edge = (id, from, to) => call("create_plot_causality", {
        request: {
          edgeId: id, novelId: nid,
          fromKind: "timeline_event", fromId: from,
          toKind: "timeline_event", toId: to,
          relation: "导致", chapterId: ids[0], sourceRevision: 1,
          evidence: "线索1出现",
        },
      });
      await edge("p1746-edge-1", "p1746-event-1", "p1746-event-5");
      await edge("p1746-edge-2", "p1746-event-5", "p1746-event-10");
      await edge("p1746-edge-3", "p1746-event-10", "p1746-event-15");
      const cycle = await call("create_plot_causality", {
        request: {
          edgeId: "p1746-edge-cycle", novelId: nid,
          fromKind: "timeline_event", fromId: "p1746-event-15",
          toKind: "timeline_event", toId: "p1746-event-1",
          relation: "导致", chapterId: ids[0], sourceRevision: 1,
          evidence: "线索1出现",
        },
      }).then(() => "unexpected-success", (error) => String(error));
      // 三次正文修订：10/20/30 章。
      stage = "rewrite";
      for (const order of [10, 20, 30]) {
        const current = await call("get_chapter", { novelId: nid, chapterId: ids[order - 1] });
        await call("save_chapter", {
          novelId: nid,
          chapterId: ids[order - 1],
          expectedVersion: current.officialVersion,
          title: `第${order}章`,
          content: `第${order}章改写：旧线索消失，新线索在修订后出现。`,
        });
      }
      // 第 10 章伏笔来源失效后，用第 30 章新证据恢复。
      stage = "plot8-recover";
      await call("transition_plot_thread", {
        request: {
          plotId: "p1746-plot-8", novelId: nid, expectedVersion: 2,
          action: "recover", chapterId: ids[29], sourceRevision: 2,
          evidence: "新线索在修订后出现", commandId: "p1746-plot-8-recover",
        },
      });
      return { nid, ids, cycle, chars };
      } catch (error) {
        return { error: String(error && error.message ? error.message : error), stage };
      }
    }, title);
    if (life.error) throw new Error("seed failed at " + life.stage + ": " + life.error);
    novelId = life.nid;
    chapters = life.ids;
    expect(life.cycle).not.toBe("unexpected-success");

    const ws5 = await invoke("get_plot_workspace", { novelId, chapterId: chapters[4] });
    const plot1At5 = ws5.threads.find((item) => item.id === "p1746-plot-1");
    expect(plot1At5.status).toBe("open");
    expect(plot1At5.resolvedChapterOrder).toBeNull();

    const ws15 = await invoke("get_plot_workspace", { novelId, chapterId: chapters[14] });
    expect(ws15.threads.find((item) => item.id === "p1746-plot-1").status).toBe("deferred");

    const ws40 = await invoke("get_plot_workspace", { novelId, chapterId: chapters[39] });
    expect(ws40.threads.find((item) => item.id === "p1746-plot-1").status).toBe("resolved");
    expect(ws40.threads.find((item) => item.id === "p1746-plot-8").status).toBe("developing");
    const ws20 = await invoke("get_plot_workspace", { novelId, chapterId: chapters[19] });
    expect(ws20.threads.find((item) => item.id === "p1746-plot-8").status).toBe("stale");

    const factsAt5 = await invoke("list_story_facts", { novelId, beforeChapterOrder: 5 });
    expect(factsAt5.some((item) => item.id === "p1746-state-5")).toBe(true);
    expect(factsAt5.some((item) => item.id === "p1746-state-10")).toBe(false);
    const factsAt40 = await invoke("list_story_facts", { novelId, beforeChapterOrder: 40 });
    expect(factsAt40.some((item) => item.id === "p1746-event-10")).toBe(false);
    expect(factsAt40.some((item) => item.id === "p1746-event-15")).toBe(true);
    expect(ws40.edges.length).toBe(3);
  });

  it("真实章节工作台在历史章节不泄露未来，并在 40 章显示回收与恢复", async () => {
    expect(novelId).toBeTruthy();
    await browser.execute(
      (nid, cid) => { window.location.hash = `#/novels/${nid}/chapters/${cid}`; },
      novelId,
      chapters[4],
    );
    await browser.pause(3000);
    await openPlotTab();
    const earlyText = await $('[data-testid="plot-workspace"]').getText();
    expect(earlyText).toContain("伏笔1");
    expect(earlyText).not.toContain("线索25出现");
    await browser.saveScreenshot("/tmp/p1746-long-ch5.png").catch(() => {});

    await browser.execute(
      (nid, cid) => { window.location.hash = `#/novels/${nid}/chapters/${cid}`; },
      novelId,
      chapters[39],
    );
    await browser.pause(3000);
    await openPlotTab();
    const lateText = await $('[data-testid="plot-workspace"]').getText();
    expect(lateText).toContain("已回收");
    expect(lateText).toContain("推进中");
    await browser.saveScreenshot("/tmp/p1746-long-ch40.png").catch(() => {});
  });
});
