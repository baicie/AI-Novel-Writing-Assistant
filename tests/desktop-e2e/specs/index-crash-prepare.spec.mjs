import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native index crash prepare", () => {
  it("indexes the first source revision and leaves the updated revision pending", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 索引崩溃重启", description: "" });
    await invoke("index_knowledge", { request: {
      id: "doc-index-crash", novelId: novel.id, ownerType: "document", title: "索引崩溃资料",
      body: "旧稿：林动在祖祠发现失传剑谱。", embedding: null, embeddingModel: null,
      sourceRevision: "v1", sourceContentHash: "hash-v1",
    } });
    const first = await invoke("retry_knowledge_document_with_job", {
      novelId: novel.id, documentId: "doc-index-crash", sourceRevision: "v1", indexVersion: 1,
    });
    expect(first.status).toBe("succeeded");
    const hits = await invoke("search_knowledge", { novelId: novel.id, query: "剑谱", limit: 5 });
    expect(hits.some((hit) => hit.body.includes("旧稿"))).toBe(true);

    await invoke("index_knowledge", { request: {
      id: "doc-index-crash", novelId: novel.id, ownerType: "document", title: "索引崩溃资料",
      body: "新稿：林动在祖祠发现残卷图谱。", embedding: null, embeddingModel: null,
      sourceRevision: "v2", sourceContentHash: "hash-v2",
    } });
    fs.writeFileSync(`${dataDir}/index-crash-state.json`, JSON.stringify({
      novelId: novel.id,
      documentId: "doc-index-crash",
      firstJobId: first.id,
      firstEpoch: first.claimEpoch,
    }));
  });
});
