import fs from "node:fs";
import path from "node:path";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const reportPath = process.env.BIZ_NOVEL_E2E_PERF_REPORT ?? path.join(dataDir, "perf-report.json");
const CHAPTERS = Number(process.env.BIZ_NOVEL_E2E_PERF_CHAPTERS ?? 1000);
const KNOWLEDGE_DOCS = Number(process.env.BIZ_NOVEL_E2E_PERF_DOCS ?? 200);
const NOVEL_TITLE = `性能基准${CHAPTERS}章`;

async function invoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

function pct(samples, p) {
  const sorted = [...samples].sort((left, right) => left - right);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil((p / 100) * sorted.length) - 1));
  return Math.round(sorted[index] * 10) / 10;
}

async function measure(command, args, samples) {
  const result = await browser.execute(async (name, payload, count) => {
    const warmupStarted = performance.now();
    await window.__TAURI__.core.invoke(name, payload);
    const warmup = performance.now() - warmupStarted;
    const out = [];
    for (let index = 0; index < count; index += 1) {
      const started = performance.now();
      await window.__TAURI__.core.invoke(name, payload);
      out.push(performance.now() - started);
    }
    return { warmup, samples: out };
  }, command, args, samples);
  const timings = result.samples;
  return {
    warmupMs: Math.round(result.warmup * 10) / 10,
    samples: timings.length,
    p50: pct(timings, 50),
    p95: pct(timings, 95),
  };
}

function paragraph(seed) {
  const sentences = [
    "夜雨压低山门灯影，林动握紧断剑沿石阶向上。",
    "祖祠的铜铃声在三更响起，苏晚按住袖中残卷。",
    "陆沉立在廊下，眼底的血色比烛火更冷。",
    "风穿过经阁，纸页翻动像有人在低声念咒。",
    "他记得师父说过，剑谱从来不是写给活人看的。",
  ];
  let state = seed >>> 0 || 1;
  const parts = [];
  for (let index = 0; index < 24; index += 1) {
    state = (state * 1664525 + 1013904223) >>> 0;
    parts.push(sentences[state % sentences.length]);
  }
  return parts.join("");
}

describe("Native 1000 chapter desktop benchmark", () => {
  it("seeds a long-form library and records desktop latencies", async () => {
    const novels = await invoke("list_novels");
    let novel = novels.find((item) => item.title === NOVEL_TITLE);
    if (!novel) {
      novel = await invoke("create_novel", { title: NOVEL_TITLE, description: "P15.8 桌面性能基准" });
    }
    let chapters = await invoke("list_chapters", { novelId: novel.id });
    const seedStarted = Date.now();
    if (chapters.length < CHAPTERS) {
      for (let index = chapters.length + 1; index <= CHAPTERS; index += 1) {
        const created = await invoke("create_chapter", {
          novelId: novel.id,
          title: `第${index}章 山门夜行`,
          sortOrder: index,
        });
        await invoke("save_chapter", {
          novelId: novel.id,
          chapterId: created.id,
          expectedVersion: 0,
          title: `第${index}章 山门夜行`,
          content: paragraph(index),
        });
      }
      chapters = await invoke("list_chapters", { novelId: novel.id });
    }
    const seedMs = Date.now() - seedStarted;

    const documents = await invoke("list_knowledge_documents");
    let indexed = documents.filter((item) => String(item.id).startsWith("perf-doc-")).length;
    const knowledgeStarted = Date.now();
    if (indexed < KNOWLEDGE_DOCS) {
      for (let index = indexed + 1; index <= KNOWLEDGE_DOCS; index += 1) {
        await invoke("index_knowledge", { request: {
          id: `perf-doc-${index}`, novelId: novel.id, ownerType: "document",
          title: `资料 ${index}`, body: paragraph(index + 7),
          embedding: null, embeddingModel: null,
          sourceRevision: "v1", sourceContentHash: `hash-${index}`,
        } });
        await invoke("retry_knowledge_document_with_job", {
          novelId: novel.id, documentId: `perf-doc-${index}`, sourceRevision: "v1", indexVersion: 1,
        });
      }
      indexed = KNOWLEDGE_DOCS;
    }
    const knowledgeMs = Date.now() - knowledgeStarted;

    const sampleChapter = chapters[Math.floor(chapters.length / 2)];
    const report = {
      targetChapters: CHAPTERS,
      chapters: chapters.length,
      knowledgeDocs: indexed,
      characters: chapters.reduce((sum, item) => sum + String(item.content ?? "").length, 0),
      totalBytes: chapters.reduce((sum, item) => sum + Buffer.byteLength(String(item.content ?? ""), "utf8"), 0),
      seedMs,
      knowledgeMs,
      dbBytes: fs.statSync(path.join(dataDir, "novel.sqlite")).size,
      metrics: {
        chapterList: await measure("list_chapters", { novelId: novel.id }, 10),
        chapterOpen: await measure("get_chapter", { novelId: novel.id, chapterId: sampleChapter.id }, 10),
        revisionHistory: await measure("list_chapter_revisions", { novelId: novel.id, chapterId: sampleChapter.id }, 10),
        taskCenter: await measure("list_native_tasks", {}, 10),
        batchList: await measure("list_batches", { novelId: novel.id }, 10),
        ftsSearch: await measure("search_knowledge", { novelId: novel.id, query: "剑谱", limit: 20 }, 10),
        fullExport: await measure("export_novel_selection", { novelId: novel.id, selection: "entire_book", format: "markdown" }, 3),
        backup: await measure("create_backup", {}, 3),
      },
      machine: process.env.BIZ_NOVEL_E2E_MACHINE ?? "unknown",
    };
    fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
    console.log("[perf]", JSON.stringify(report.metrics));
    expect(report.chapters).toBe(CHAPTERS);
    expect(report.metrics.chapterList.p95).toBeGreaterThan(0);
  });
});
