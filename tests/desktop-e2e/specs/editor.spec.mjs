import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;

async function tauriInvoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

async function createFixture(title, chapters = 2) {
  const novel = await tauriInvoke("create_novel", { title, description: "" });
  const ids = [];
  for (let index = 1; index <= chapters; index += 1) {
    const chapter = await tauriInvoke("create_chapter", { novelId: novel.id, title: `第${index}章`, sortOrder: index });
    ids.push(chapter.id);
  }
  return { novelId: novel.id, chapterIds: ids };
}

async function openEditor(novelId, chapterId) {
  await browser.execute(([n, c]) => { window.location.hash = `#/novels/${n}/chapters/${c}`; }, [novelId, chapterId]);
  await (await $("[data-testid=chapter-editor-surface]")).waitForDisplayed({ timeout: 20_000 });
  const editor = await $('[contenteditable="true"]');
  await editor.waitForDisplayed({ timeout: 20_000 });
  return editor;
}

async function typeText(editor, text) {
  await editor.click();
  await editor.addValue(text);
}

describe("Chapter editor local draft safety", () => {
  it("autosaves an unsaved local draft and restores it after page refresh", async () => {
    const fixture = await createFixture("WDIO 草稿刷新", 1);
    const editor = await openEditor(fixture.novelId, fixture.chapterIds[0]);
    await typeText(editor, "测试草稿刷新");
    await (await $("[data-testid=chapter-local-draft-saved]")).waitForDisplayed({ timeout: 20_000 });
    await browser.refresh();
    await (await $("[data-testid=chapter-editor-surface]")).waitForDisplayed({ timeout: 20_000 });
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("测试草稿刷新"), { timeout: 20_000, timeoutMsg: "durable draft was not restored after refresh" });
    fs.writeFileSync(`${dataDir}/editor-state.json`, JSON.stringify(fixture));
  });

  it("saves official text, switches chapters, and returns without losing content", async () => {
    const fixture = await createFixture("WDIO 正式保存", 2);
    const editor = await openEditor(fixture.novelId, fixture.chapterIds[0]);
    await typeText(editor, "正式保存正文");
    const save = await $("[data-testid=chapter-save-button]");
    await save.waitForEnabled({ timeout: 20_000 });
    await save.click();
    await browser.waitUntil(async () => {
      const button = await $("[data-testid=chapter-save-button]");
      return (await button.isEnabled()) === false;
    }, { timeout: 20_000, timeoutMsg: "official save did not complete" });
    const saved = await tauriInvoke("get_chapter", { novelId: fixture.novelId, chapterId: fixture.chapterIds[0] });
    expect(saved.content).toContain("正式保存正文");
    await browser.execute(([n, c]) => { window.location.hash = `#/novels/${n}/chapters/${c}`; }, [fixture.novelId, fixture.chapterIds[1]]);
    await (await $("[data-testid=chapter-editor-surface]")).waitForDisplayed({ timeout: 20_000 });
    await browser.execute(([n, c]) => { window.location.hash = `#/novels/${n}/chapters/${c}`; }, [fixture.novelId, fixture.chapterIds[0]]);
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("正式保存正文"), { timeout: 20_000, timeoutMsg: "saved chapter content missing after switch" });
  });
});
