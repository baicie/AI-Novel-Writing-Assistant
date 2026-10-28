import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Shared fixture journey: knowledge and export", () => {
  it("indexes a knowledge source, answers search and exports volumes and the whole book", async function () {
    this.timeout(180_000);
    const fixture = JSON.parse(fs.readFileSync(`${dataDir}/journey-fixture.json`, "utf8"));
    const novelId = fixture.novelId;

    for (let index = 0; index < fixture.chapterIds.length; index += 1) {
      const chapterId = fixture.chapterIds[index];
      const chapter = await invoke("get_chapter", { novelId, chapterId });
      await invoke("update_chapter_meta", {
        novelId,
        chapterId,
        expectedVersion: chapter.officialVersion,
        title: chapter.title,
        sortOrder: chapter.sortOrder,
        volumeId: index < 5 ? fixture.volumeIds[0] : fixture.volumeIds[1],
      });
    }

    await invoke("index_knowledge", { request: {
      id: "journey-doc-1",
      novelId,
      ownerType: "document",
      title: "山门旧事",
      body: "祖祠之下封着失传剑谱，夜色里有人抄录图谱。",
      embedding: null,
      embeddingModel: null,
      sourceRevision: "v1",
      sourceContentHash: "journey-hash-v1",
    } });
    const job = await invoke("retry_knowledge_document_with_job", {
      novelId,
      documentId: "journey-doc-1",
      sourceRevision: "v1",
      indexVersion: 1,
    });
    expect(job.status).toBe("succeeded");
    const hits = await invoke("search_knowledge", { novelId, query: "剑谱", limit: 5 });
    expect(hits.some((hit) => hit.body.includes("剑谱"))).toBe(true);

    const firstVolume = await invoke("export_novel_selection", {
      novelId, selection: "volume", format: "markdown", volumeIds: [fixture.volumeIds[0]],
    });
    expect(firstVolume).toContain("第1章");
    expect(firstVolume).toContain("第5章");
    expect(firstVolume).not.toContain("第6章");

    const wholeBook = await invoke("export_novel_selection", {
      novelId, selection: "entire_book", format: "markdown",
    });
    let cursor = -1;
    for (let index = 1; index <= 10; index += 1) {
      const at = wholeBook.indexOf(`第${index}章`);
      expect(at).toBeGreaterThan(cursor);
      cursor = at;
    }
    expect(wholeBook.length).toBeGreaterThan(200);
  });
});
