// P17.4.7 D 修复验收：桌面端手动创建角色与本书世界（真实 GUI / Tauri IPC / SQLite）。
const invoke = (command, args) =>
  browser.execute((cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload), command, args);

async function bodyText() {
  return String(await browser.execute("document.body.innerText"));
}

describe("P17.4.7 D 场景修复验收", () => {
  it("不依赖模型创建空白角色并绑定本书世界", async () => {
    await browser.pause(2500);
    const novel = await invoke("create_novel_with_setup", {
      request: { title: "P17.4.7 D 修复验收书", description: "", creationExperience: "professional" },
    });

    // 角色：新增角色 -> 直接创建空白角色。
    await browser.execute((id) => { window.location.hash = `#/novels/${id}/edit?stage=character`; }, novel.id);
    const addCharacter = await $("//button[contains(., '新增角色')]");
    await addCharacter.waitForExist({ timeout: 20000 });
    await addCharacter.click();
    const nameInput = await $('input[placeholder="角色名称（必填）"]');
    await nameInput.waitForExist({ timeout: 10000 });
    await nameInput.setValue("D修复角色");
    await $('input[placeholder="当前目标（例如：三个月内突破）"]').setValue("三个月内突破");
    await $('input[placeholder="与主角关系（如：试探合作）"]').setValue("试探合作");
    const manualCreate = await $("//button[contains(., '直接创建空白角色')]");
    await manualCreate.waitForClickable({ timeout: 10000 });
    await manualCreate.click();
    await browser.waitUntil(
      async () => {
        const characters = await invoke("list_characters", { novelId: novel.id });
        return Array.isArray(characters) && characters.some((item) => item.name === "D修复角色");
      },
      { timeout: 20000, timeoutMsg: "桌面端没有创建出空白角色" },
    );
    await browser.waitUntil(
      async () => (await bodyText()).includes("D修复角色"),
      { timeout: 20000, timeoutMsg: "角色面板没有刷新出新角色" },
    );
    const createdCharacter = (await invoke("list_characters", { novelId: novel.id }))
      .find((item) => item.name === "D修复角色");
    expect(createdCharacter.profile?.outerGoal).toBe("三个月内突破");
    expect(createdCharacter.profile?.relationToProtagonist).toBe("试探合作");
    await browser.waitUntil(
      async () => (await bodyText()).includes("三个月内突破"),
      { timeout: 20000, timeoutMsg: "角色面板没有展示当前目标" },
    );
    await browser.waitUntil(
      async () => !(await $('input[placeholder="角色名称（必填）"]').isExisting()),
      { timeout: 20000, timeoutMsg: "角色创建成功后弹窗没有关闭" },
    );
    await browser.saveScreenshot("/tmp/p1747-d-fix-character.png").catch(() => {});

    // 世界：自定义空白手册 -> 自定义本书世界。
    await browser.execute((id) => { window.location.hash = `#/novels/${id}/edit?stage=world`; }, novel.id);
    const manualWorld = await $("//button[contains(., '自定义空白手册')]");
    await manualWorld.waitForExist({ timeout: 20000 });
    await manualWorld.click();
    await $('input[placeholder="例如：紫霞界"]').setValue("D修复世界");
    await $('input[placeholder="例如：星核枯竭的边境帝国，魔法与权力都要付出代价。"]')
      .setValue("用于 D 场景修复验收的世界");
    await $("//button[contains(., '自定义本书世界')]").click();
    await browser.waitUntil(
      async () => {
        const detail = await invoke("get_novel", { novelId: novel.id });
        return Boolean(detail.worldId);
      },
      { timeout: 20000, timeoutMsg: "本书没有绑定新创建的世界" },
    );
    await browser.waitUntil(
      async () => (await bodyText()).includes("D修复世界"),
      { timeout: 20000, timeoutMsg: "世界面板没有刷新出新世界" },
    );
    const detail = await invoke("get_novel", { novelId: novel.id });
    expect(detail.worldId).toBeTruthy();
    const characters = await invoke("list_characters", { novelId: novel.id });
    expect(characters.some((item) => item.name === "D修复角色")).toBe(true);
    await browser.saveScreenshot("/tmp/p1747-d-fix-world.png").catch(() => {});
  });
});
