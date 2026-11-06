// P17.4.6：真实桌面 GUI 验收（新建小说、章节、正文保存、剧情线索）。
// 真实 Tauri 应用 + 真实 SQLite + 本地 UI；复杂生命周期数据由既有 p17-4 spec 覆盖。
const invoke = (command, args) =>
  browser.execute(
    (cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload),
    command,
    args,
  );

async function screenshot(name) {
  try {
    await browser.saveScreenshot(`/tmp/p1746-gui-${name}.png`);
  } catch (error) {
    console.log("screenshot skipped:", name, String(error));
  }
}

describe("P17.4.6 真实桌面 GUI 验收", () => {
  it("从新建小说到章节正文保存，再创建伏笔", async () => {
    await browser.pause(2500);
    await browser.execute("(() => { window.location.hash = '#/novels/create'; })()");
    await browser.pause(1500);
    const title = await $("#basic-title");
    await title.waitForExist({ timeout: 15000 });
    await title.setValue("P17.4.6 GUI 验收书");
    await $("#basic-estimated-chapters").setValue("100");
    await screenshot("create-form");

    const submit = await $("//button[contains(., '创建并进入项目')]");
    await submit.waitForClickable({ timeout: 10000 });
    await submit.click();
    await browser.pause(2500);
    const hash = await browser.execute("window.location.hash");
    console.log("after create hash:", hash);
    const novelId = String(hash).match(/\/novels\/([^/]+)/)?.[1];
    expect(novelId).toBeTruthy();

    await browser.execute(
      (id) => { window.location.hash = `#/novels/${id}/edit?stage=chapter`; },
      novelId,
    );
    await browser.pause(3000);
    console.log("after stage chapter hash:", await browser.execute("window.location.hash"));
    console.log("body text:", (await browser.execute("document.body.innerText")).slice(0, 500));
    await screenshot("chapter-stage");
    const createChapter = await $("//button[contains(., '新建章节')]");
    await createChapter.waitForExist({ timeout: 15000 });
    for (let i = 0; i < 2; i += 1) {
      await createChapter.waitForEnabled({ timeout: 10000 });
      await createChapter.click();
      await browser.pause(1500);
    }
    const chapters = await invoke("list_chapters", { novelId });
    expect(chapters.length).toBeGreaterThanOrEqual(2);

    await browser.execute(
      (id, chapterId) => { window.location.hash = `#/novels/${id}/chapters/${chapterId}`; },
      novelId,
      chapters[0].id,
    );
    await browser.pause(3000);
    const editor = await $('[data-testid="chapter-editor-content"]');
    await editor.waitForExist({ timeout: 15000 });
    await editor.click();
    await browser.execute(() => {
      const surface = document.querySelector('[data-testid="chapter-editor-content"]');
      surface?.focus();
      document.execCommand("insertText", false, "GUI acceptance body.");
    });
    await browser.pause(1200);
    console.log("editor text:", await browser.execute(() => document.querySelector('[data-testid="chapter-editor-content"]')?.innerText));
    const save = await $('[data-testid="chapter-save-button"]');
    await save.waitForClickable({ timeout: 10000 });
    await save.click();
    await browser.pause(2500);
    await screenshot("chapter-saved");

    const current = await invoke("get_chapter", { novelId, chapterId: chapters[0].id });
    expect(current.content).toContain("GUI acceptance body");

    const plotTab = await $('[data-testid="panel-tab-plot"]');
    await plotTab.waitForExist({ timeout: 10000 });
    await plotTab.click();
    await browser.pause(1000);
    const newTitle = await $('[data-testid="plot-new-title"]');
    await newTitle.waitForExist({ timeout: 10000 });
    await newTitle.setValue("GUI 验收伏笔");
    await $('[data-testid="plot-new-body"]').setValue("作者在 GUI 中创建的一条伏笔");
    await $('[data-testid="plot-new-evidence"]').setValue("GUI acceptance body");
    await screenshot("plot-form");
    await $('[data-testid="plot-create-submit"]').click();
    await browser.pause(1200);
    const workspaceText = await $('[data-testid="plot-workspace"]').getText();
    expect(workspaceText).toContain("GUI 验收伏笔");
    await screenshot("plot-created");
  });
});
