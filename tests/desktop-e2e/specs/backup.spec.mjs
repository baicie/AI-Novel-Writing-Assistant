async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native backup", () => {
  it("creates and verifies a consistent local backup", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 备份", description: "" });
    await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    const panel = await $("[data-testid=backup-panel]");
    await panel.waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=backup-create]")).click();
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("已创建备份"), { timeout: 20_000 });
    const backups = await invoke("list_backups");
    expect(backups.length).toBeGreaterThanOrEqual(1);
    const verify = await invoke("verify_backup", { fileName: backups[0].fileName });
    expect(verify).toBe("ok");
    await (await $("[data-testid=backup-verify]")).click();
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("备份完整性"), { timeout: 20_000 });
  });
});
