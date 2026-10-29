async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native index retry", () => {
  it("shows an injected index failure, retries from the source page, and searches the new content", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 索引恢复", description: "" });
    await invoke("index_knowledge", { request: {
      id: "doc-index-retry", novelId: novel.id, ownerType: "document", title: "索引恢复资料",
      body: "林动在祖祠发现失传剑谱。", embedding: null, embeddingModel: null,
      sourceRevision: "v1", sourceContentHash: "hash-v1",
    } });
    const failed = await invoke("retry_knowledge_document_with_job", {
      novelId: novel.id, documentId: "doc-index-retry", sourceRevision: "v1", indexVersion: 1,
    });
    expect(["failed", "retry_wait"]).toContain(failed.status);
    expect(failed.lastError).toContain("E2E");
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    const retry = await $("[data-testid=native-retry-index-job]");
    await retry.waitForDisplayed({ timeout: 20_000 });
    await retry.click();
    await browser.waitUntil(async () => {
      const jobs = await invoke("list_index_jobs", { novelId: novel.id });
      return jobs.some((job) => job.id === failed.id && job.status === "succeeded");
    }, { timeout: 20_000 });
    const hits = await invoke("search_knowledge", { novelId: novel.id, query: "剑谱", limit: 5 });
    expect(hits.some((hit) => hit.body.includes("剑谱"))).toBe(true);
  });
});
