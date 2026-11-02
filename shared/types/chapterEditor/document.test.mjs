import assert from "node:assert/strict";
import test from "node:test";
import {
  applySelectedChapterEditorDiff,
  buildChapterEditorDiff,
  buildChapterEditorDiffChunks,
  chapterParagraphs,
  DIFF_DEGRADED_NOTE,
  groupChapterEditorDiffChanges,
  insertChapterContinuation,
  normalizeChapterText,
  replaceChapterSelection,
  withDiffComplexityNote,
} from "./document.ts";

test("chapter document coordinates preserve authored whitespace", () => {
  const content = normalizeChapterText("  第一段\r\n\r\n第二段\n");
  assert.equal(content, "  第一段\n\n第二段\n");
  assert.deepEqual(chapterParagraphs(content), [
    { index: 1, text: "  第一段", from: 0, to: 5 },
    { index: 2, text: "第二段\n", from: 7, to: 11 },
  ]);
});

test("chapter selection replacement rejects stale text", () => {
  assert.equal(replaceChapterSelection("甲\n乙", { from: 2, to: 3, text: "乙" }, "丙"), "甲\n丙");
  assert.throws(
    () => replaceChapterSelection("甲\n新乙", { from: 2, to: 3, text: "乙" }, "丙"),
    /正文片段已变化/,
  );
});

test("chapter continuation inserts at an exact cursor offset", () => {
  assert.equal(insertChapterContinuation("甲\n乙", 2, "，然后转身"), "甲\n，然后转身乙");
  assert.throws(
    () => insertChapterContinuation("甲", 2, "乙"),
    /正文光标位置已变化/,
  );
});

test("chapter diff groups replacements so partial acceptance cannot duplicate source text", () => {
  const chunks = [
    { id: "equal-1", type: "equal", text: "她抬头" },
    { id: "delete-1", type: "delete", text: "看向门口" },
    { id: "insert-1", type: "insert", text: "望向漆黑的门口" },
    { id: "equal-2", type: "equal", text: "。" },
    { id: "insert-2", type: "insert", text: "\n" },
  ];
  const changes = groupChapterEditorDiffChanges(chunks);
  assert.deepEqual(changes.map(({ kind, originalText, candidateText }) => ({ kind, originalText, candidateText })), [
    { kind: "replace", originalText: "看向门口", candidateText: "望向漆黑的门口" },
    { kind: "insert", originalText: "", candidateText: "\n" },
  ]);
  const candidate = "她抬头望向漆黑的门口。\n";
  assert.equal(applySelectedChapterEditorDiff("她抬头看向门口。", candidate, chunks, new Set()), "她抬头看向门口。");
  assert.equal(applySelectedChapterEditorDiff("她抬头看向门口。", candidate, chunks, new Set(["change-1"])), "她抬头望向漆黑的门口。");
  assert.equal(applySelectedChapterEditorDiff("她抬头看向门口。", candidate, chunks, new Set(["change-2"])), "她抬头看向门口。\n");
});

test("built diff chunks always satisfy the apply path's reconstruction check", () => {
  // buildChapterEditorDiffChunks 的产物必须能被 applySelectedChapterEditorDiff 接受：
  // 这两个函数一个在生成侧、一个在采纳侧，分片口径不一致会让作者采纳时直接报错。
  const cases = [
    ["她抬头看向门口。", "她抬头望向漆黑的门口。"],
    ["", "全新的一段正文"],
    ["要删掉的旧段落", ""],
    ["完全一致的文本", "完全一致的文本"],
    ["第一段\n\n第二段", "第一段\n\n改写后的第二段\n\n新增第三段"],
    ["Mixed 中英 text 42", "Mixed 中英文 text 7"],
  ];
  for (const [original, candidate] of cases) {
    const chunks = buildChapterEditorDiffChunks(original, candidate);
    assert.equal(
      chunks.filter((chunk) => chunk.type !== "insert").map((chunk) => chunk.text).join(""),
      original,
      `删除+相同必须还原原文：${JSON.stringify(original)}`,
    );
    assert.equal(
      chunks.filter((chunk) => chunk.type !== "delete").map((chunk) => chunk.text).join(""),
      candidate,
      `插入+相同必须还原候选：${JSON.stringify(candidate)}`,
    );
    // 全不选 = 原文，全选 = 候选。这是作者在 Diff 面板里的两个端点。
    assert.equal(applySelectedChapterEditorDiff(original, candidate, chunks, new Set()), original);
    const allChanges = new Set(groupChapterEditorDiffChanges(chunks).map((change) => change.id));
    assert.equal(applySelectedChapterEditorDiff(original, candidate, chunks, allChanges), candidate);
  }
});

test("diff chunk ids are unique so partial acceptance can address each change", () => {
  const chunks = buildChapterEditorDiffChunks("她抬头看向门口。", "她抬头望向漆黑的门口。\n");
  assert.equal(new Set(chunks.map((chunk) => chunk.id)).size, chunks.length, "分片 id 不能重复");
  assert.ok(chunks.every((chunk) => ["equal", "insert", "delete"].includes(chunk.type)));
});

// ---- P17.2 P1.2：长文本 diff 的有界性与正确性 ----

/** 生成接近真实章节的中文正文，段落之间用空行分隔。 */
function proseOfLength(length) {
  const sentence = "林动抬头望向远处的山门风雪正紧他握紧手里的旧刀继续向前走去";
  const paragraphs = [];
  let consumed = 0;
  let index = 0;
  while (consumed < length) {
    const piece = sentence.repeat(2).slice(0, 80);
    const paragraph = `${piece}${index % 5 === 0 ? "。" : "，这一点他心里清楚。"}`;
    paragraphs.push(paragraph);
    consumed += paragraph.length + 2;
    index += 1;
  }
  return paragraphs.join("\n\n").slice(0, length);
}

function assertRebuilds(original, candidate, chunks) {
  assert.equal(
    chunks.filter((chunk) => chunk.type !== "insert").map((chunk) => chunk.text).join(""),
    original,
    "分片必须能重建原文",
  );
  assert.equal(
    chunks.filter((chunk) => chunk.type !== "delete").map((chunk) => chunk.text).join(""),
    candidate,
    "分片必须能重建候选",
  );
}

test("diff 边界在 Emoji、代理对与重复段落上仍然精确", () => {
  const cases = [
    ["开头🌙结尾", "开头🌕结尾"],                       // Emoji 单码点替换
    ["他笑了😀😀，没有回头。", "他笑了😀，没有回头。"],   // 重复 Emoji 只删一个
    ["👨‍👩‍👧一家人在门口。", "👨‍👩‍👦一家人在门口。"],        // ZWJ 组合序列
    ["第一段\n\n第一段\n\n第一段", "第一段\n\n第二段\n\n第一段"], // 重复段落
    ["甲\r\n\r\n乙", "甲\n\n乙"],                        // CRLF 归一化
    ["𝄞 音符与中文混排", "𝄞 音符和中文混排"],           // 星形平面字符
  ];
  for (const [original, candidate] of cases) {
    const chunks = buildChapterEditorDiffChunks(original, candidate);
    assertRebuilds(normalizeChapterText(original), normalizeChapterText(candidate), chunks);
    const all = new Set(groupChapterEditorDiffChanges(chunks).map((change) => change.id));
    assert.equal(
      applySelectedChapterEditorDiff(normalizeChapterText(original), normalizeChapterText(candidate), chunks, all),
      normalizeChapterText(candidate),
    );
  }
});

test("一万到五万字的章节对比在预算内完成，且不产生错误分片", () => {
  // 旧实现是 O(n·m) 二维矩阵：实测 1 万字约 2.4 秒 / 630MB，3 万字直接 JS 堆溢出。
  // 这里对三种规模都要求「有限时间内完成 + 分片可精确重建」，防止性能回退。
  for (const size of [10_000, 30_000, 50_000]) {
    const original = proseOfLength(size);
    // 真实改写只动其中一小段：公共前后缀之外的工作量应当很小。
    const offset = Math.floor(size / 2);
    const candidate = `${original.slice(0, offset)}他忽然停下脚步，回头望了一眼。${original.slice(offset + 10)}`;
    const started = performance.now();
    const build = buildChapterEditorDiff(original, candidate);
    const elapsed = performance.now() - started;

    assertRebuilds(original, candidate, build.chunks);
    assert.equal(build.degraded, false, `${size} 字的局部改写应当保持细粒度对比`);
    const changes = groupChapterEditorDiffChanges(build.chunks);
    // 局部改写只应影响被改的那一小段；LCS 可能因改动词句里恰好相同的字样拆成两处，
    // 但改动总量必须仍然收敛在原选区附近，不能扩散成整章对比。
    assert.ok(changes.length <= 3, `${size} 字局部改写产生了 ${changes.length} 处修改`);
    const changedOriginalLength = changes.reduce((total, change) => total + change.originalText.length, 0);
    assert.ok(changedOriginalLength <= 40, `${size} 字对比改动了 ${changedOriginalLength} 个原文字符`);
    const allChanges = new Set(changes.map((change) => change.id));
    assert.equal(
      applySelectedChapterEditorDiff(original, candidate, build.chunks, allChanges),
      candidate,
      "勾选全部改动后应当等于候选",
    );
    // 机器速度差异很大，这里只挡住「回到 O(n·m)」这种量级退化。
    assert.ok(elapsed < 1500, `${size} 字对比耗时 ${Math.round(elapsed)}ms 超出预算`);
  }
});

test("结构完全不同的大文本退化为整段替换并明确标记，不伪装成精细对比", () => {
  const original = proseOfLength(50_000);
  const candidate = original.split("").reverse().join("");
  const build = buildChapterEditorDiff(original, candidate);
  assertRebuilds(original, candidate, build.chunks);
  assert.equal(build.degraded, true, "无法细分时必须显式标记退化");
  // 退化路径仍要能被采纳路径接受：整段拒绝 = 原文，整段接受 = 候选。
  assert.equal(applySelectedChapterEditorDiff(original, candidate, build.chunks, new Set()), original);
  const all = new Set(groupChapterEditorDiffChanges(build.chunks).map((change) => change.id));
  assert.equal(applySelectedChapterEditorDiff(original, candidate, build.chunks, all), candidate);
});

test("退化提示只在需要时并入候选风险说明", () => {
  assert.deepEqual(withDiffComplexityNote(["语气更重"], false), ["语气更重"]);
  assert.deepEqual(withDiffComplexityNote(["语气更重"], true), ["语气更重", DIFF_DEGRADED_NOTE]);
  assert.deepEqual(withDiffComplexityNote([DIFF_DEGRADED_NOTE], true), [DIFF_DEGRADED_NOTE], "同一句不能重复");
});

test("小文本仍保持旧的细粒度语义：逐处对比、相邻同类型分片合并、id 稳定", () => {
  // 这是旧实现在同一输入上的输出：整段替换是有界实现引入的新路径，
  // 但小文本必须继续走逐处对比，不能因为公共前后缀裁剪而改变对外语义。
  const chunks = buildChapterEditorDiffChunks("她抬头看向门口。", "她抬头望向漆黑的门口。\n");
  assert.deepEqual(
    chunks.map((chunk) => chunk.type),
    ["equal", "delete", "insert", "equal", "insert", "equal", "insert"],
  );
  assert.deepEqual(
    chunks.map((chunk) => chunk.id),
    ["chunk-1", "chunk-2", "chunk-3", "chunk-4", "chunk-5", "chunk-6", "chunk-7"],
  );
  for (let index = 1; index < chunks.length; index += 1) {
    assert.notEqual(chunks[index].type, chunks[index - 1].type, "相邻同类型分片必须合并");
  }
});
