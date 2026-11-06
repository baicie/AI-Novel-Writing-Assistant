const invoke = (command, args) =>
  browser.execute((cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload), command, args);

async function bodyText() {
  return String(await browser.execute("document.body.innerText"));
}

describe("P17.4.7 D 人物与世界观 GUI 探测", () => {
  it("观察桌面端新增角色与自定义世界的真实入口", async () => {
    await browser.pause(2500);
    const novel = await invoke("create_novel_with_setup", {
      request: { title: "P17.4.7 D 验收书", description: "", creationExperience: "professional" },
    });
    await invoke("create_character", {
      request: { id: "d-probe-char", novelId: novel.id, name: "D验收角色", role: "主角" },
    });

    // 角色面板：已存在的角色应可见。
    await browser.execute((id) => { window.location.hash = `#/novels/${id}/edit?stage=character`; }, novel.id);
    await browser.pause(3000);
    const characterText = await bodyText();
    console.log("character panel has seeded:", characterText.includes("D验收角色"));
    const add = await $("//button[contains(., '新增角色')]");
    await add.waitForExist({ timeout: 15000 });
    await add.click();
    await browser.pause(1000);
    const dialogText = await bodyText();
    console.log("dialog has AI create:", dialogText.includes("AI 生成角色卡"));
    console.log("dialog has base import:", dialogText.includes("从基础角色库导入"));
    console.log("dialog has manual blank create:", dialogText.includes("手动创建角色"));
    await browser.saveScreenshot("/tmp/p1747-d-character-dialog.png").catch(() => {});
    await browser.keys(["Escape"]);
    await browser.pause(500);
    await browser.saveScreenshot("/tmp/p1747-d-character-panel.png").catch(() => {});

    // 基础角色库。
    await browser.execute(() => { window.location.hash = "#/base-characters"; });
    await browser.pause(2500);
    const libText = await bodyText();
    console.log("base library not migrated:", libText.includes("暂未接入桌面本地内核") || libText.includes("还没有接入桌面本地内核"));
    console.log("base library create button:", libText.includes("创建角色"));
    await browser.saveScreenshot("/tmp/p1747-d-base-library.png").catch(() => {});

    // 世界面板：尝试自定义世界。
    await browser.execute((id) => { window.location.hash = `#/novels/${id}/edit?stage=world`; }, novel.id);
    await browser.pause(3000);
    const worldText = await bodyText();
    console.log("world panel has manual choice:", worldText.includes("自定义空白手册"));
    console.log("world panel unsupported:", worldText.includes("暂未接入桌面本地内核") || worldText.includes("还没有接入桌面本地内核"));
    const manual = await $("//button[contains(., '自定义空白手册')]");
    if (await manual.isExisting()) {
      await manual.click();
      await browser.pause(500);
      await $('input[placeholder="例如：紫霞界"]').setValue("D验收世界");
      await $('input[placeholder="例如：星核枯竭的边境帝国，魔法与权力都要付出代价。"]').setValue("用于验收的边境世界");
      await $("//button[contains(., '自定义本书世界')]").click();
      await browser.pause(2500);
      const afterWorld = await bodyText();
      console.log("world created text:", afterWorld.includes("D验收世界"));
      console.log("world error:", afterWorld.includes("暂未接入") || afterWorld.includes("没有接入") || afterWorld.includes("未迁移"));
    }
    await browser.saveScreenshot("/tmp/p1747-d-world.png").catch(() => {});
    console.log("missing routes:", JSON.stringify(await browser.execute(() => window.__missingRoutes ?? [])));
  });
});
