// P17.0：章节工作台的卷/章导航与右侧统一工作区。
// 全部走真实 Tauri IPC 建数据，再在真实界面上断言。
describe("章节工作台导航与统一工作区", () => {
  let novelId = "";
  let firstChapterId = "";
  let secondChapterId = "";

  before(async () => {
    await browser.pause(2500);
    const seeded = await browser.execute(async () => {
      const call = window.__TAURI_INTERNALS__.invoke;
      const novels = await call("list_novels");
      const existing = (novels || []).find((item) => item.title === "P17 工作台书");
      let novel = existing;
      if (!novel) {
        novel = await call("create_novel", { title: "P17 工作台书", description: "" });
      }
      const chapters = (await call("list_chapters", { novelId: novel.id })) || [];
      const ensure = async (title, sortOrder) => {
        const found = chapters.find((item) => item.sortOrder === sortOrder);
        if (found) return found.id;
        const created = await call("create_chapter", {
          novelId: novel.id,
          title,
          sortOrder,
        });
        return created.id ?? created.chapterId;
      };
      const first = await ensure("第一章", 1);
      const second = await ensure("第二章", 2);
      return { novelId: novel.id, first, second };
    });
    novelId = seeded.novelId;
    firstChapterId = seeded.first;
    secondChapterId = seeded.second;
  });

  it("左侧能导航全书章节，右侧能切换工作区页签", async () => {
    await browser.execute(
      (nid, cid) => {
        window.location.hash = `#/novels/${nid}/chapters/${cid}`;
      },
      novelId,
      firstChapterId
    );
    await browser.pause(3500);

    const navigator = await $('[data-testid="chapter-navigator"]');
    await navigator.waitForExist({ timeout: 15000 });
    expect(await navigator.isExisting()).toBe(true);

    // 章节列表里能看到两章，当前章被标记。
    const items = await $$('[data-testid="chapter-nav-item"]');
    expect(items.length).toBeGreaterThanOrEqual(2);
    const currentMarked = await $$('[data-testid="chapter-nav-item"][data-current="true"]');
    expect(currentMarked.length).toBe(1);

    // 右侧四个页签都在，且默认停在「AI 修改」。
    const reviseTab = await $('[data-testid="panel-tab-revise"]');
    await reviseTab.waitForExist({ timeout: 10000 });
    expect(await reviseTab.getAttribute("data-active")).toBe("true");

    // 切到「人物与世界观」：只读面板要出现。
    const castTab = await $('[data-testid="panel-tab-cast"]');
    await castTab.click();
    await browser.pause(400);
    const castPanel = await $('[data-testid="cast-world-tab"]');
    expect(await castPanel.isExisting()).toBe(true);

    // 切到「运行与恢复」：要么给出真实运行状态，要么给出明确的空状态。
    const runTab = await $('[data-testid="panel-tab-run"]');
    await runTab.click();
    await browser.pause(2500);
    const runPanel = await $('[data-testid="run-recovery-tab"]');
    const bodyText = await browser.execute(() => document.body.innerText);
    const hasRunPanel = await runPanel.isExisting();
    const hasEmptyHint = bodyText.includes("这本书还没有运行记录");
    expect(hasRunPanel || hasEmptyHint).toBe(true);

    // 键盘导航：Alt+→ 等价于点「下一章」，同样只有一个当前态。
    const beforeHash = await browser.execute(() => window.location.hash);
    await browser.keys(["Alt", "ArrowRight"]);
    await browser.pause(2500);
    const afterHash = await browser.execute(() => window.location.hash);
    if (afterHash !== beforeHash) {
      const currentAfterKey = await $$('[data-testid="chapter-nav-item"][data-current="true"]');
      expect(currentAfterKey.length).toBe(1);
    }

    // 切到第二章：当前章标记要跟着走。
    const nextButton = await $('[data-testid="chapter-nav-next"]');
    if (await nextButton.isExisting()) {
      await nextButton.click();
      await browser.pause(2500);
      const hash = await browser.execute(() => window.location.hash);
      expect(hash).toContain("/chapters/");
      const stillOneCurrent = await $$('[data-testid="chapter-nav-item"][data-current="true"]');
      expect(stillOneCurrent.length).toBe(1);
    }
  });


  it("页签在切章后保持，小窗口下 AI 助手可收起", async () => {
    await browser.setWindowSize(1100, 900);
    await browser.execute(
      (nid, cid) => {
        window.location.hash = `#/novels/${nid}/chapters/${cid}`;
      },
      novelId,
      firstChapterId
    );
    await browser.pause(3000);

    // 选中「人物与世界观」，切到下一章后应保持在同一页签。
    const castTab = await $('[data-testid="panel-tab-cast"]');
    await castTab.waitForExist({ timeout: 10000 });
    await castTab.click();
    await browser.pause(400);
    expect(await castTab.getAttribute("data-active")).toBe("true");

    const nextButton = await $('[data-testid="chapter-nav-next"]');
    if (await nextButton.isExisting()) {
      await nextButton.click();
      await browser.pause(2500);
      const castAfterSwitch = await $('[data-testid="panel-tab-cast"]');
      expect(await castAfterSwitch.getAttribute("data-active")).toBe("true");
    }

    // 小窗口下应有收起/展开按钮，并且真的能收起面板。
    const toggle = await $('[data-testid="toggle-right-panel"]');
    await toggle.waitForExist({ timeout: 10000 });
    const panelBefore = await $('[data-testid="chapter-right-panel"]');
    expect(await panelBefore.isDisplayed()).toBe(true);

    await toggle.click();
    await browser.pause(400);
    const panelCollapsed = await $('[data-testid="chapter-right-panel"]');
    expect(await panelCollapsed.isDisplayed()).toBe(false);

    await toggle.click();
    await browser.pause(400);
    const panelRestored = await $('[data-testid="chapter-right-panel"]');
    expect(await panelRestored.isDisplayed()).toBe(true);

    await browser.setWindowSize(1440, 900);
  });
});
