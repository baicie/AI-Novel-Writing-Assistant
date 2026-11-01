import assert from "node:assert/strict";
import test from "node:test";
import {
  buildChapterGroups,
  buildRunAttention,
  chapterNavigationIntent,
  chapterSwitchConfirmation,
  filterChapters,
  formatChapterWordCount,
  formatWordCount,
  hasPendingCandidate,
  isSaveShortcut,
  neighbourOf,
  runAttentionFor,
  shouldBlockChapterSwitch,
  toChapterNavItem,
} from "./chapterNavigatorModel.ts";

const chapters = [
  { id: "c3", title: "第三章", order: 3, content: "有正文内容", officialVersion: 2, volumeId: "v2" },
  { id: "c1", title: "第一章", order: 1, content: "", officialVersion: 0, volumeId: "v1" },
  { id: "c2", title: "第二章", order: 2, content: "  \n\t ", officialVersion: 1, volumeId: "v1" },
];

test("chapters are grouped by volume and ordered", () => {
  const groups = buildChapterGroups(chapters, [
    { id: "v1", title: "第一卷" },
    { id: "v2", title: "第二卷" },
  ]);
  assert.deepEqual(groups.map((group) => group.label), ["第一卷", "第二卷"]);
  assert.deepEqual(groups[0].chapters.map((chapter) => chapter.id), ["c1", "c2"]);
  assert.deepEqual(groups[1].chapters.map((chapter) => chapter.id), ["c3"]);
});

test("nav items keep real content facts instead of a cast stub", () => {
  const item = toChapterNavItem(chapters[0]);
  assert.equal(item.id, "c3");
  assert.equal(item.wordCount, "有正文内容".length, "字数必须来自真实正文");
  assert.equal(item.hasContent, true);
  assert.equal(item.officialVersion, 2);

  const blank = toChapterNavItem(chapters[1]);
  assert.equal(blank.wordCount, 0, "纯空白的章节字数必须是 0");
  assert.equal(blank.hasContent, false);
  assert.equal(formatWordCount(item.wordCount), "5 字");
  assert.equal(formatWordCount(blank.wordCount), "空");
});

test("chapters without a known volume fall back to a safe group", () => {
  const groups = buildChapterGroups([{ id: "c9", title: "", order: 9 }], []);
  assert.equal(groups.length, 1);
  assert.equal(groups[0].label, "未分卷");
  assert.equal(groups[0].chapters[0].id, "c9");
  assert.equal(groups[0].chapters[0].title, "未命名章节");
});

test("search matches by order or title and ignores blank keyword", () => {
  assert.equal(filterChapters(chapters, "").length, 3);
  assert.deepEqual(filterChapters(chapters, "第三").map((chapter) => chapter.id), ["c3"]);
  assert.deepEqual(filterChapters(chapters, "2").map((chapter) => chapter.id), ["c2"]);
});

test("previous and next respect order boundaries", () => {
  assert.deepEqual(neighbourOf(chapters, "c2"), {
    previous: chapters.find((chapter) => chapter.id === "c1"),
    next: chapters.find((chapter) => chapter.id === "c3"),
  });
  assert.equal(neighbourOf(chapters, "c1").previous, null);
  assert.equal(neighbourOf(chapters, "c3").next, null);
  assert.deepEqual(neighbourOf(chapters, "missing"), { previous: null, next: null });
});

test("run attention maps backend states to actionable levels", () => {
  assert.equal(runAttentionFor("waiting_approval"), "action");
  assert.equal(runAttentionFor("paused"), "action");
  assert.equal(runAttentionFor("failed"), "action");
  assert.equal(runAttentionFor("running"), "running");
  assert.equal(runAttentionFor("succeeded"), "done");
  assert.equal(runAttentionFor(null), null);
  assert.equal(runAttentionFor("something_new"), null);
});

test("latest run per chapter decides the badge", () => {
  const attention = buildRunAttention([
    { chapterId: "c1", state: "succeeded", updatedAt: 10 },
    { chapterId: "c1", state: "paused", updatedAt: 20 },
    { chapterId: "c2", state: "running", updatedAt: 5 },
    { chapterId: "c3", state: "succeeded", updatedAt: 7 },
  ]);
  assert.deepEqual(attention, { c1: "action", c2: "running", c3: "done" });
});

const CLEAN_GUARD = {
  hasUnsavedContent: false,
  isDraftWriteBusy: false,
  hasPendingCandidate: false,
};

const guard = (overrides) => ({ ...CLEAN_GUARD, ...overrides });

/** 会丢弃候选稿的三个出口共用一套判定，只有去向文案不同。 */
const LEAVE_INTENTS = ["switch_chapter", "leave_editor", "load_version"];

test("pending candidate is only the ready session that still holds candidates", () => {
  assert.equal(hasPendingCandidate({ status: "ready", candidates: [{ id: "cand1" }] }), true);
  assert.equal(hasPendingCandidate({ status: "ready", candidates: [] }), false, "ready 但没有候选稿时无可丢失");
  assert.equal(hasPendingCandidate({ status: "loading", candidates: [] }), false, "生成中还没有结果");
  assert.equal(
    hasPendingCandidate({ status: "loading", candidates: [{ id: "stale" }] }),
    false,
    "重新生成时旧候选稿已被清空，loading 一律不算待确认",
  );
  assert.equal(hasPendingCandidate({ status: "error", candidates: [{ id: "cand1" }] }), false);
  assert.equal(hasPendingCandidate({ status: "idle", candidates: [] }), false, "采用或放弃后会话回到 idle");
  assert.equal(hasPendingCandidate(null), false);
  assert.equal(hasPendingCandidate(undefined), false);
  assert.equal(hasPendingCandidate({ status: "ready" }), false, "缺少 candidates 字段时按无候选稿处理");
});

test("a fully clean chapter never interrupts the author", () => {
  let asked = 0;
  assert.equal(chapterSwitchConfirmation(CLEAN_GUARD), null);
  assert.equal(
    shouldBlockChapterSwitch(CLEAN_GUARD, () => {
      asked += 1;
      return true;
    }),
    false,
  );
  assert.equal(asked, 0, "没有任何待处理成果时不应打扰作者");
});

test("each loss reason raises its own confirmation copy", () => {
  const unsaved = chapterSwitchConfirmation(guard({ hasUnsavedContent: true }));
  assert.equal(unsaved.reason, "unsaved_content");
  assert.deepEqual(unsaved.reasons, ["unsaved_content"]);
  assert.match(unsaved.message, /可以从本地草稿恢复/, "未保存正文必须说明可以恢复");

  const busy = chapterSwitchConfirmation(guard({ isDraftWriteBusy: true }));
  assert.equal(busy.reason, "draft_write_busy");
  assert.deepEqual(busy.reasons, ["draft_write_busy"]);
  assert.match(busy.message, /保存还没有完成/);

  const candidate = chapterSwitchConfirmation(guard({ hasPendingCandidate: true }));
  assert.equal(candidate.reason, "candidate_pending");
  assert.deepEqual(candidate.reasons, ["candidate_pending"]);
  assert.match(candidate.message, /会丢弃它/, "候选稿必须如实说明无法保留");
  assert.doesNotMatch(candidate.message, /恢复/, "候选稿不能承诺可以恢复");
});

test("candidate loss is announced before the recoverable ones", () => {
  const all = chapterSwitchConfirmation({
    hasUnsavedContent: true,
    isDraftWriteBusy: true,
    hasPendingCandidate: true,
  });
  assert.equal(all.reason, "candidate_pending", "最严重的损失排在最前");
  assert.deepEqual(all.reasons, ["candidate_pending", "draft_write_busy", "unsaved_content"]);
  assert.ok(
    all.message.indexOf("候选稿") < all.message.indexOf("未保存的修改"),
    "候选稿无法找回，必须先告诉作者",
  );

  const candidateAndUnsaved = chapterSwitchConfirmation(
    guard({ hasUnsavedContent: true, hasPendingCandidate: true }),
  );
  assert.deepEqual(candidateAndUnsaved.reasons, ["candidate_pending", "unsaved_content"]);
  assert.match(candidateAndUnsaved.message, /会丢弃它/);
  assert.match(candidateAndUnsaved.message, /可以从本地草稿恢复/, "两项损失的归属要分别说清");
});

test("an unfinished write does not promise draft recovery", () => {
  const both = chapterSwitchConfirmation(guard({ hasUnsavedContent: true, isDraftWriteBusy: true }));
  assert.deepEqual(both.reasons, ["draft_write_busy", "unsaved_content"]);
  assert.doesNotMatch(both.message, /可以从本地草稿恢复/, "写入未落地时不能承诺草稿一定能恢复");
  assert.match(both.message, /写不进去/);
});

test("confirmation copy stays in task wording without change narration", () => {
  const states = [
    guard({ hasUnsavedContent: true }),
    guard({ isDraftWriteBusy: true }),
    guard({ hasPendingCandidate: true }),
    { hasUnsavedContent: true, isDraftWriteBusy: true, hasPendingCandidate: true },
  ];
  for (const intent of LEAVE_INTENTS) {
    for (const state of states) {
      const message = chapterSwitchConfirmation(state, intent).message;
      for (const banned of ["现在", "不再", "已经", "之前", "原本"]) {
        assert.ok(!message.includes(banned), `确认文案不能出现过程叙述「${banned}」：${message}`);
      }
      assert.match(message, /吗？$/, "确认文案必须以一个明确的问题收尾");
    }
  }
});

test("every exit asks about the same losses in its own wording", () => {
  const state = guard({ hasPendingCandidate: true, hasUnsavedContent: true });
  const messages = new Map();
  for (const intent of LEAVE_INTENTS) {
    const confirmation = chapterSwitchConfirmation(state, intent);
    assert.deepEqual(
      confirmation.reasons,
      ["candidate_pending", "unsaved_content"],
      `${intent} 必须与其他出口共用同一套原因判定`,
    );
    messages.set(intent, confirmation.message);
  }

  assert.match(messages.get("switch_chapter"), /确定切换到其他章节吗？$/);
  assert.match(messages.get("leave_editor"), /确定离开正文编辑吗？$/);
  assert.match(messages.get("load_version"), /确定载入这个版本吗？$/);
  assert.equal(new Set(messages.values()).size, LEAVE_INTENTS.length, "去向不同，文案应当分别说明");
});

test("loading a version says the unsaved text is replaced instead of recoverable", () => {
  // 载入版本会把编辑区内容换掉，本地草稿随后被同一份新内容覆盖，
  // 所以这个出口不能复用「可从本地草稿恢复」的说法。
  const unsaved = chapterSwitchConfirmation(guard({ hasUnsavedContent: true }), "load_version");
  assert.equal(unsaved.reason, "unsaved_content");
  assert.doesNotMatch(unsaved.message, /可以从本地草稿恢复/, "载入版本不能承诺未保存正文可以恢复");
  assert.match(unsaved.message, /替换|覆盖|找不回/, "必须如实说明未保存正文会被顶掉");

  // 写入未落地时，其他出口会降级成不提恢复的说法；载入版本本来就不提，两者不应互相影响。
  const busy = chapterSwitchConfirmation(
    guard({ hasUnsavedContent: true, isDraftWriteBusy: true }),
    "load_version",
  );
  assert.deepEqual(busy.reasons, ["draft_write_busy", "unsaved_content"]);
  assert.doesNotMatch(busy.message, /可以从本地草稿恢复/);

  // 候选稿仍然排在最前，且同样不承诺可恢复。
  const withCandidate = chapterSwitchConfirmation(
    guard({ hasUnsavedContent: true, hasPendingCandidate: true }),
    "load_version",
  );
  assert.equal(withCandidate.reason, "candidate_pending");
  assert.ok(
    withCandidate.message.indexOf("候选稿") < withCandidate.message.indexOf("未保存"),
    "候选稿无法找回，必须先告诉作者",
  );
});

test("the guard blocks on refusal and releases on consent for every reason", () => {
  const cases = [
    guard({ hasUnsavedContent: true }),
    guard({ isDraftWriteBusy: true }),
    guard({ hasPendingCandidate: true }),
    guard({ hasUnsavedContent: true, hasPendingCandidate: true }),
    { hasUnsavedContent: true, isDraftWriteBusy: true, hasPendingCandidate: true },
  ];
  for (const intent of LEAVE_INTENTS) {
    for (const state of cases) {
      const seen = [];
      assert.equal(
        shouldBlockChapterSwitch(
          state,
          (message) => {
            seen.push(message);
            return false;
          },
          intent,
        ),
        true,
        `作者拒绝时必须留在原地：${intent} ${JSON.stringify(state)}`,
      );
      assert.equal(seen.length, 1, "每次只问一次");
      assert.equal(
        seen[0],
        chapterSwitchConfirmation(state, intent).message,
        "拦截文案必须来自同一套判定",
      );
      assert.equal(
        shouldBlockChapterSwitch(state, () => true, intent),
        false,
        `作者同意后应当放行：${intent} ${JSON.stringify(state)}`,
      );
    }
  }

  // 不传 intent 时按切章处理，保持既有调用点的行为不变。
  const state = guard({ hasUnsavedContent: true });
  assert.equal(shouldBlockChapterSwitch(state, () => false), true);
  assert.equal(
    chapterSwitchConfirmation(state).message,
    chapterSwitchConfirmation(state, "switch_chapter").message,
    "默认 intent 必须等价于切章",
  );
});

test("navigation intent only guards real chapter switches", () => {
  assert.deepEqual(chapterNavigationIntent("c1", "c2", false), {
    targetChapterId: "c1",
    openRunTab: false,
    requiresSwitchGuard: true,
  });
  assert.deepEqual(chapterNavigationIntent("c2", "c2", true), {
    targetChapterId: "c2",
    openRunTab: true,
    requiresSwitchGuard: false,
  });
});

test("chapter word count ignores blank content", () => {
  assert.equal(formatChapterWordCount(""), "空");
  assert.equal(formatChapterWordCount(null), "空");
  assert.equal(formatChapterWordCount("   "), "空");
  assert.equal(formatChapterWordCount("少年入山"), "4 字");
});

test("save shortcut only matches cmd/ctrl + s without alt", () => {
  assert.equal(isSaveShortcut({ key: "s", metaKey: true }), true);
  assert.equal(isSaveShortcut({ key: "S", ctrlKey: true }), true);
  assert.equal(isSaveShortcut({ key: "s" }), false);
  assert.equal(isSaveShortcut({ key: "s", metaKey: true, altKey: true }), false);
  assert.equal(isSaveShortcut({ key: "k", metaKey: true }), false);
});

// ---------------------------------------------------------------------------
// 规模化导航：长篇小说常见的 100 / 500 / 1000 章，正确性优先，另加一条宽松的耗时护栏。
// ---------------------------------------------------------------------------

const CHAPTERS_PER_VOLUME = 20;

/** 构造 count 章、每 20 章一卷的导航输入；order 故意打乱，确保排序由被测函数负责。 */
function buildScaleFixture(count) {
  const volumeCount = Math.ceil(count / CHAPTERS_PER_VOLUME);
  const volumes = Array.from({ length: volumeCount }, (_, index) => ({
    id: `v${index + 1}`,
    title: `第 ${index + 1} 卷`,
  }));
  const chapters = Array.from({ length: count }, (_, index) => {
    const order = index + 1;
    return {
      id: `c${order}`,
      title: `第 ${order} 章 山海行`,
      order,
      content: order % 7 === 0 ? "" : `第 ${order} 章的正文`,
      officialVersion: order % 3,
      volumeId: `v${Math.floor(index / CHAPTERS_PER_VOLUME) + 1}`,
    };
  });
  // 逆序交给被测函数：分组与相邻计算都不能依赖输入顺序。
  return { volumes, chapters: chapters.reverse() };
}

for (const size of [100, 500, 1000]) {
  test(`grouping stays complete and ordered at ${size} chapters`, () => {
    const { chapters, volumes } = buildScaleFixture(size);
    const groups = buildChapterGroups(chapters, volumes);

    assert.equal(groups.length, Math.ceil(size / CHAPTERS_PER_VOLUME), "卷数必须完整");
    assert.deepEqual(
      groups.map((group) => group.label),
      volumes.map((volume) => volume.title),
      "卷的先后顺序必须稳定",
    );

    const flat = groups.flatMap((group) => group.chapters);
    assert.equal(flat.length, size, "分组不能丢章也不能重复章");
    assert.equal(new Set(flat.map((item) => item.id)).size, size, "章节 id 必须唯一");
    assert.deepEqual(
      flat.map((item) => item.order),
      Array.from({ length: size }, (_, index) => index + 1),
      "展平后必须是 1..n 的连续章号",
    );
    for (const group of groups) {
      assert.ok(group.chapters.length > 0 && group.chapters.length <= CHAPTERS_PER_VOLUME);
      const orders = group.chapters.map((item) => item.order);
      assert.deepEqual([...orders].sort((a, b) => a - b), orders, "卷内必须按章号升序");
    }
  });

  test(`search hits the exact expected set at ${size} chapters`, () => {
    const { chapters } = buildScaleFixture(size);

    assert.equal(filterChapters(chapters, "").length, size, "空关键词返回全部");
    assert.equal(filterChapters(chapters, "山海行").length, size, "共有标题应当全部命中");
    assert.equal(filterChapters(chapters, "没有这一章").length, 0);

    // 章号子串命中：期望集由同一条规则独立算出，避免拿被测实现验证自己。
    const expected = chapters
      .filter((chapter) => String(chapter.order).includes("23"))
      .map((chapter) => chapter.id)
      .sort();
    const actual = filterChapters(chapters, "23").map((chapter) => chapter.id).sort();
    assert.deepEqual(actual, expected, "章号搜索的命中集必须精确");
    assert.ok(expected.length > 0, "fixture 必须覆盖到章号搜索");

    const single = filterChapters(chapters, `第 ${size} 章`);
    assert.deepEqual(single.map((chapter) => chapter.id), [`c${size}`], "完整标题应当唯一命中");
  });

  test(`neighbour boundaries hold at ${size} chapters`, () => {
    const { chapters } = buildScaleFixture(size);

    const first = neighbourOf(chapters, "c1");
    assert.equal(first.previous, null, "首章没有上一章");
    assert.equal(first.next.id, "c2");

    const last = neighbourOf(chapters, `c${size}`);
    assert.equal(last.next, null, "末章没有下一章");
    assert.equal(last.previous.id, `c${size - 1}`);

    const middleOrder = Math.floor(size / 2);
    const middle = neighbourOf(chapters, `c${middleOrder}`);
    assert.equal(middle.previous.id, `c${middleOrder - 1}`);
    assert.equal(middle.next.id, `c${middleOrder + 1}`);

    assert.deepEqual(neighbourOf(chapters, "missing"), { previous: null, next: null });

    // 从首章一路走到末章，必须恰好经过每一章一次。
    let cursor = chapters.find((chapter) => chapter.order === 1);
    const walked = [cursor.id];
    while (true) {
      const { next } = neighbourOf(chapters, cursor.id);
      if (!next) break;
      cursor = next;
      walked.push(cursor.id);
      assert.ok(walked.length <= size, "相邻链不能出现环");
    }
    assert.equal(walked.length, size, "上/下一章必须能走完全书");
    assert.equal(new Set(walked).size, size, "相邻链不能重复经过同一章");
  });
}

test("1000-chapter grouping and search finish well inside an interactive budget", () => {
  const { chapters, volumes } = buildScaleFixture(1000);
  // 宽松上界：本机实测合计约 10ms，留 100 倍余量，慢机器与 CI 也不会偶发失败。
  const budgetMs = 2000;

  const startedAt = performance.now();
  const groups = buildChapterGroups(filterChapters(chapters, "山海行"), volumes);
  const matched = filterChapters(chapters, "7");
  const elapsed = performance.now() - startedAt;

  // 先断言结果正确，耗时上界只是附加护栏，不能单独成立。
  assert.equal(groups.length, 50);
  assert.equal(groups.flatMap((group) => group.chapters).length, 1000);
  assert.deepEqual(
    matched.map((chapter) => chapter.id).sort(),
    chapters.filter((chapter) => String(chapter.order).includes("7")).map((chapter) => chapter.id).sort(),
  );
  assert.ok(elapsed < budgetMs, `1000 章的分组与搜索耗时 ${elapsed.toFixed(1)}ms，超过 ${budgetMs}ms 上界`);
});
