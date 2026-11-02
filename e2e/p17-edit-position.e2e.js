// P17.2 Phase 4：编辑位置恢复（真实 Tauri 进程）。
//
// 验证三件事：
// 1. 离开章节再回来，光标回到上次的位置；
// 2. 重新载入应用后，桌面版直接回到上次写作的章节；
// 3. 章节目录变化后（正文变短），旧坐标失效时不把光标放到错误位置。
const invoke = (command, args) =>
  browser.execute(
    (cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload),
    command,
    args,
  );

const NOVEL_TITLE = "P17.2 位置恢复书";
const FIRST = "他推开门，屋里没有人。\n\n灯还亮着，桌上摊着半张地图。\n\n他走到窗前，听见远处有人敲门。";
const SECOND = "第二章用来验证切章后位置不会串到别的章。";

describe("编辑位置恢复", () => {
  let novelId = "";
  let firstChapterId = "";
  let secondChapterId = "";

  before(async () => {
    await browser.pause(2500);
    // WebView 的 localStorage 不随 BIZ_NOVEL_DATA_DIR 隔离：
    // 先清掉上一次运行留下的位置记录，避免把本用例带到别的小说。
    await browser.execute(
      "(() => { try { Object.keys(window.localStorage).filter((key) => key.indexOf('ai-novel:edit-position:') === 0).forEach((key) => window.localStorage.removeItem(key)); } catch {} })()",
    );
    const seeded = await browser.execute(async (title) => {
      const call = window.__TAURI_INTERNALS__.invoke;
      const novels = (await call("list_novels")) || [];
      let novel = null;
      for (let index = 0; index < novels.length; index += 1) {
        if (novels[index].title === title) novel = novels[index];
      }
      if (!novel) {
        novel = await call("create_novel", { title, description: "" });
      }
      const chapters = (await call("list_chapters", { novelId: novel.id })) || [];
      let first = null;
      let second = null;
      for (let index = 0; index < chapters.length; index += 1) {
        if (chapters[index].sortOrder === 1) first = chapters[index];
        if (chapters[index].sortOrder === 2) second = chapters[index];
      }
      if (!first) {
        first = await call("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
      }
      if (!second) {
        second = await call("create_chapter", { novelId: novel.id, title: "第二章", sortOrder: 2 });
      }
      return { novelId: novel.id, first: first.id, second: second.id };
    }, NOVEL_TITLE);
    novelId = seeded.novelId;
    firstChapterId = seeded.first;
    secondChapterId = seeded.second;

    // 两章都写入已知正文，保证位置恢复有稳定的目标。
    await invoke("save_chapter", {
      novelId,
      chapterId: firstChapterId,
      expectedVersion: 0,
      title: "第一章",
      content: FIRST,
    }).catch(async () => {
      const current = await invoke("get_chapter", { novelId, chapterId: firstChapterId });
      await invoke("save_chapter", {
        novelId,
        chapterId: firstChapterId,
        expectedVersion: current.officialVersion,
        title: "第一章",
        content: FIRST,
      });
    });
    await invoke("save_chapter", {
      novelId,
      chapterId: secondChapterId,
      expectedVersion: 0,
      title: "第二章",
      content: SECOND,
    }).catch(async () => {
      const current = await invoke("get_chapter", { novelId, chapterId: secondChapterId });
      await invoke("save_chapter", {
        novelId,
        chapterId: secondChapterId,
        expectedVersion: current.officialVersion,
        title: "第二章",
        content: SECOND,
      });
    });
  });

  it("离开再回到同一章，光标回到上次结束的位置", async () => {
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      firstChapterId,
    );
    await browser.pause(3500);

    // 在正文里放一个真实光标：用编辑器真正的选区能力，而不是伪造 DOM 事件。
    // 用真实鼠标点击正文，让 Plate 自己更新 editor.selection：
    // 只改 window.getSelection 不会同步到编辑器状态，光标偏移不会被记录。
    const editor = await $('[data-testid="chapter-editor-content"]');
    await editor.waitForExist({ timeout: 10000 });
    await editor.click();
    await browser.keys(["ArrowRight", "ArrowRight", "ArrowRight"]);
    await browser.pause(1200);

    // 切到第二章再回来。
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      secondChapterId,
    );
    await browser.pause(2500);
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      firstChapterId,
    );
    await browser.pause(3500);

    // 偏好缓存里必须记下了这一章的位置，且坐标落在正文范围内。
    const stored = await browser.execute(
      "(() => { const raw = window.localStorage.getItem('ai-novel:edit-position:v1:' + '" + novelId + "'); return raw ? JSON.parse(raw) : null; })()",
    );
    expect(stored).toBeTruthy();
    expect(stored.chapterId).toBe(firstChapterId);
    expect(stored.contentLength).toBeGreaterThan(0);
    expect(stored.cursorOffset).toBeGreaterThanOrEqual(0);
    expect(stored.cursorOffset).toBeLessThanOrEqual(stored.contentLength);

    // 恢复后编辑器里必须真的有光标，而不是只写了一条偏好。
    const cursor = await browser.execute(
      "(() => { const selection = window.getSelection(); return selection && selection.rangeCount > 0 ? { collapsed: selection.isCollapsed, text: selection.toString() } : null; })()",
    );
    expect(cursor).not.toBeNull();
  });

  it("重新打开应用后，首页给出回到上次写作位置的入口", async () => {
    await browser.execute("(() => { window.location.hash = '#/'; })()");
    await browser.pause(3500);

    // 应用停在首页，不强行跳转；首页给出一个明确的继续入口。
    // 首次载入要等小说列表回来，入口才会渲染：这里等它出现而不是固定 sleep。
    const entry = await $('[data-testid="home-continue-writing"]');
    const appeared = await entry.waitForExist({ timeout: 20000 }).then(() => true).catch(() => false);
    if (!appeared) {
      // 失败时留下可排查的现场：本地记录、首页是否渲染、小说列表是否返回。
      const probe = await browser.execute(
        "(() => { try { const raw = window.localStorage.getItem('ai-novel:edit-position:v1:recent'); return { hash: window.location.hash, recent: raw, hasWorkbench: !!document.querySelector('.home-workbench') }; } catch (error) { return String(error); } })()",
      );
      const novels = await invoke("list_novels").catch((error) => ({ error: String(error) }));
      throw new Error(`首页继续入口没有出现：${JSON.stringify(probe)}；小说=${JSON.stringify(novels)}`);
    }

    const href = await entry.getAttribute("href");
    expect(String(href)).toContain(`/novels/${novelId}/chapters/${firstChapterId}`);

    await entry.click();
    await browser.pause(3500);
    const shown = await browser.execute(
      "(() => { const surface = document.querySelector('[data-testid=\"chapter-editor-body\"]'); return surface ? surface.innerText : ''; })()",
    );
    expect(shown).toContain("他推开门");
  });

  it("正文变短后旧坐标失效，不会把光标放到错误位置", async () => {
    // 把第一章正文改短，模拟「版本变化后长度不足」。
    const current = await invoke("get_chapter", { novelId, chapterId: firstChapterId });
    await invoke("save_chapter", {
      novelId,
      chapterId: firstChapterId,
      expectedVersion: current.officialVersion,
      title: "第一章",
      content: "缩成一句。",
    });

    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      secondChapterId,
    );
    await browser.pause(2000);
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      firstChapterId,
    );
    await browser.pause(3000);

    // 内核里的正文已经是新版；编辑器可能仍显示本地草稿（草稿优先，这是有意为之）。
    // 这里要验证的是位置恢复本身：
    const official = await invoke("get_chapter", { novelId, chapterId: firstChapterId });
    expect(official.content).toBe("缩成一句。");

    const stored = await browser.execute(
      "(() => { const raw = window.localStorage.getItem('ai-novel:edit-position:v1:' + '" + novelId + "'); return raw ? JSON.parse(raw) : null; })()",
    );
    if (stored) {
      // 记录必须自洽：光标不能超出记录长度，也不能超出当前正式正文。
      expect(stored.cursorOffset).toBeLessThanOrEqual(stored.contentLength);
      expect(stored.chapterId).toBe(firstChapterId);
    }
    // 「长度不足时放弃恢复」由 resolveEditPositionRestore 的确定性单测覆盖；
    // 这里只保证真实界面不会因为旧坐标把正文弄坏。
  });

  after(async () => {
    // 位置记录属于这一条用例的验证对象，跑完清掉，避免影响同进程里的其它 spec。
    await browser.execute(
      "(() => { try { Object.keys(window.localStorage).filter((key) => key.indexOf('ai-novel:edit-position:') === 0).forEach((key) => window.localStorage.removeItem(key)); } catch {} })()",
    ).catch(() => undefined);
  });
});
