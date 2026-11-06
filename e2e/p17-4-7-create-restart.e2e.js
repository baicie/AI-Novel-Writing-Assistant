// P17.4.7：重新打开桌面应用后，创建的小说仍进入专业工作台。
//
// 新进程读取同一数据目录；从首页的“编辑小说”入口打开作品，不修改 location.hash。
const invoke = (command, args) =>
  browser.execute((cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload), command, args);

describe("P17.4.7 创建后的工作区恢复", () => {
  it("重新打开应用后仍从专业工作台进入，而不是简易书架", async () => {
    await browser.pause(2500);
    expect(String(await browser.execute("window.location.hash"))).not.toContain("/novels/");

    const openNovel = await $("//a[contains(., '编辑小说')]");
    await openNovel.waitForExist({ timeout: 15000 });
    await openNovel.click();

    await browser.waitUntil(
      async () => {
        const hash = String(await browser.execute("window.location.hash"));
        return hash.includes("/novels/") && hash.includes("/edit");
      },
      { timeout: 20000, timeoutMsg: "重新打开后没有进入专业工作台" },
    );
    const hash = String(await browser.execute("window.location.hash"));
    console.log("restart open route:", hash);
    expect(hash).not.toContain("/simple");

    const novelId = hash.match(/\/novels\/([^/?]+)/)?.[1];
    expect(novelId).toBeTruthy();
    const novel = await invoke("get_novel", { novelId });
    expect(novel.creationExperience).toBe("professional");

    const chapterStep = await $("//nav//button[contains(., '章节执行')]");
    await chapterStep.waitForExist({ timeout: 15000 });
    await chapterStep.click();
    const createChapter = await $("//button[contains(., '新建章节')]");
    await createChapter.waitForExist({ timeout: 15000 });

    const chapters = await invoke("list_chapters", { novelId });
    expect(Array.isArray(chapters)).toBe(true);
    expect(chapters.length).toBe(1);
    await browser.saveScreenshot("/tmp/p1747-create-restart.png").catch(() => {});
  });
});
