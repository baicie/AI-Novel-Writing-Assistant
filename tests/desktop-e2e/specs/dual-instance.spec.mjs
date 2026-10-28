import fs from "node:fs";
import path from "node:path";
import { spawn, execFileSync } from "node:child_process";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const binary = process.env.BIZ_NOVEL_E2E_BINARY;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Two app instances on one data directory", () => {
  it("keeps the database consistent and indexes each document once", async function () {
    this.timeout(180_000);
    const novel = await invoke("create_novel", { title: "双实例样本", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("save_chapter", {
      novelId: novel.id, chapterId: chapter.id, expectedVersion: 0,
      title: "第一章", content: "双实例用例的初始正文。",
    });

    const marker = path.join(dataDir, "second-instance.json");
    const child = spawn(binary, [], {
      env: {
        ...process.env,
        BIZ_NOVEL_DATA_DIR: dataDir,
        BIZ_NOVEL_E2E_STARTUP_MARKER: marker,
        TAURI_WEBDRIVER_PORT: "4446",
      },
      stdio: "ignore",
    });
    const deadline = Date.now() + 30_000;
    while (Date.now() < deadline && !fs.existsSync(marker)) await browser.pause(200);
    expect(fs.existsSync(marker)).toBe(true);
    const second = JSON.parse(fs.readFileSync(marker, "utf8"));
    expect(typeof second.pid).toBe("number");

    await invoke("index_knowledge", { request: {
      id: "dual-doc", novelId: novel.id, ownerType: "document", title: "双实例知识",
      body: "双实例下同一个知识来源只应有一份索引。",
      embedding: null, embeddingModel: null, sourceRevision: "v1", sourceContentHash: "dual-h1",
    } });
    const first = await invoke("get_chapter", { novelId: novel.id, chapterId: chapter.id });
    await invoke("save_chapter", {
      novelId: novel.id, chapterId: chapter.id, expectedVersion: first.officialVersion,
      title: first.title, content: "第一个实例补写的正文。",
    });

    process.kill(second.pid, "SIGKILL");
    child.unref();
    await browser.pause(1500);

    const after = await invoke("get_chapter", { novelId: novel.id, chapterId: chapter.id });
    expect(after.content).toContain("第一个实例");
    const hits = await invoke("search_knowledge", { novelId: novel.id, query: "索引", limit: 5 });
    expect(hits.some((hit) => JSON.stringify(hit).includes("双实例"))).toBe(true);
    const backup = await invoke("create_backup");
    expect(await invoke("verify_backup", { fileName: backup.fileName })).toBe("ok");

    const db = path.join(dataDir, "novel.sqlite");
    const documents = execFileSync("sqlite3", [db, "select count(*) from knowledge_document where id='dual-doc';"], { encoding: "utf8" }).trim();
    expect(documents).toBe("1");
    const status = execFileSync("sqlite3", [db, "select index_status from knowledge_document where id='dual-doc';"], { encoding: "utf8" }).trim();
    expect(status).toBe("ready");
    const chunks = execFileSync("sqlite3", [db, "select count(*) from knowledge_chunk where document_id='dual-doc';"], { encoding: "utf8" }).trim();
    expect(Number(chunks)).toBeGreaterThanOrEqual(1);
    const duplicateChunks = execFileSync("sqlite3", [db, "select count(*) from (select ordinal, count(*) c from knowledge_chunk where document_id='dual-doc' group by ordinal having c > 1);"], { encoding: "utf8" }).trim();
    expect(duplicateChunks).toBe("0");
    const integrity = execFileSync("sqlite3", [db, "PRAGMA integrity_check;"], { encoding: "utf8" }).trim();
    expect(integrity).toBe("ok");
  });
});
