// P17.4.7：手动创建小说的自然导航验收。
//
// 本用例只通过真实控件操作：首页 -> 手动创建小说 -> 创建并进入项目 -> 等待应用自己跳转，
// 不修改 location.hash，也不调用路由 API。创建后必须落在专业创作工作台（不能是简易书架），
// 并能从工作台步骤进入章节执行。
const invoke = (command, args) =>
  browser.execute((cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload), command, args);

const NOVEL_TITLE = "P17.4.7 自然导航验收书";

async function currentHash() {
  return String(await browser.execute("window.location.hash"));
}

describe("P17.4.7 手动创建自然导航", () => {
  it("从首页创建后自然进入专业工作台，并能进入章节执行", async () => {
    await browser.pause(2500);
    expect(await currentHash()).not.toContain("/novels/create");

    const manualCreate = await $("//a[contains(., '手动创建小说')]");
    await manualCreate.waitForExist({ timeout: 15000 });
    await manualCreate.click();

    const title = await $("#basic-title");
    await title.waitForExist({ timeout: 15000 });
    await title.setValue(NOVEL_TITLE);
    await $("#basic-estimated-chapters").setValue("5");

    const submit = await $("//button[contains(., '创建并进入项目')]");
    await submit.waitForClickable({ timeout: 10000 });
    await submit.click();

    // 等待应用自己完成跳转；这里不写 hash，只观察。
    await browser.waitUntil(
      async () => {
        const hash = await currentHash();
        return hash.includes("/novels/") && !hash.includes("/create") && !hash.includes("/simple");
      },
      { timeout: 20000, timeoutMsg: "创建后没有自然进入专业工作台" },
    );
    const naturalHash = await currentHash();
    console.log("natural create route:", naturalHash);
    expect(naturalHash).not.toContain("/simple");
    expect(naturalHash).toContain("/edit");

    const novelId = naturalHash.match(/\/novels\/([^/?]+)/)?.[1];
    expect(novelId).toBeTruthy();
    const novel = await invoke("get_novel", { novelId });
    expect(novel.title).toBe(NOVEL_TITLE);
    expect(novel.creationExperience).toBe("professional");

    // 从工作台步骤进入章节执行（真实点击，不改 hash）。
    const chapterStep = await $("//nav//button[contains(., '章节执行')]");
    await chapterStep.waitForExist({ timeout: 15000 });
    await chapterStep.click();
    const createChapter = await $("//button[contains(., '新建章节')]");
    await createChapter.waitForExist({ timeout: 15000 });
    expect(await currentHash()).toContain("stage=chapter");

    await createChapter.click();
    await browser.waitUntil(
      async () => {
        const chapters = await invoke("list_chapters", { novelId });
        return Array.isArray(chapters) && chapters.length === 1;
      },
      { timeout: 15000, timeoutMsg: "章节执行页没有创建出章节" },
    );
    const chapters = await invoke("list_chapters", { novelId });
    expect(chapters[0].title).toBeTruthy();
    await browser.saveScreenshot("/tmp/p1747-create-natural.png").catch(() => {});
  });
});
