import http from "node:http";

const story = {
  expansion: {
    expanded_premise: "E2E 主角被卷入宗门秘密。",
    protagonist_core: "被逐出家族却拥有未知血脉。",
    conflict_engine: "每揭开一层秘密，旧秩序就追捕一次。",
    conflict_layers: { external: "宗门追杀", internal: "身份恐惧", relational: "师门背叛" },
    mystery_box: "山门之下封着什么？",
    emotional_line: "从自保到承担",
    setpiece_seeds: ["山门夜战", "祖祠认亲"],
    tone_reference: "冷峻、克制、悬念推进"
  },
  decomposition: {
    selling_point: "被逐少年逆袭宗门", core_conflict: "个人真相与宗门秩序",
    main_hook: "血脉秘密", progression_loop: "发现 -> 被追杀 -> 变强 -> 揭开更大秘密",
    growth_path: "从逃避到承担", major_payoffs: ["宗门大比", "祖祠真相"], ending_flavor: "代价后的新生"
  },
  constraints: ["不提前揭露终极秘密"], issues: []
};
const cast = {
  members: [
    { name: "林动", role: "主角", castRole: "protagonist", storyFunction: "承载成长与真相", personality: "坚韧", outerGoal: "查明身世", innerNeed: "被认可" },
    { name: "苏晚", role: "同伴", castRole: "ally", storyFunction: "关系压力与协助", personality: "冷静", outerGoal: "保护师门", innerNeed: "赎罪" },
    { name: "陆沉", role: "对立者", castRole: "antagonist", storyFunction: "持续施压", personality: "偏执", outerGoal: "夺取血脉", innerNeed: "证明自己" }
  ],
  relations: [
    { sourceName: "林动", targetName: "苏晚", surfaceRelation: "同门", conflictSource: "秘密隐瞒" },
    { sourceName: "林动", targetName: "陆沉", surfaceRelation: "敌对", conflictSource: "宗门继承" }
  ]
};
const volume = { volumes: [
  { volumeOrder: 1, title: "山门卷", roleLabel: "开局", coreReward: "进入宗门", planningMode: "hard" },
  { volumeOrder: 2, title: "祖祠卷", roleLabel: "揭示", coreReward: "血脉真相", planningMode: "soft" }
] };
const chapters = { chapters: [
  { chapterOrder: 1, title: "第一章 山门", volumeTitle: "山门卷", summary: "林动入山", characterNames: ["林动", "苏晚"] },
  { chapterOrder: 2, title: "第二章 夜战", volumeTitle: "山门卷", summary: "夜战", characterNames: ["林动", "陆沉"] }
] };
const cleanReview = { issues: [] };

function outputFor(body, scenario) {
  if (body.includes("STEP:story_macro")) return JSON.stringify(story);
  if (body.includes("STEP:character_cast")) return JSON.stringify(cast);
  if (body.includes("STEP:volume_strategy")) return JSON.stringify(volume);
  if (body.includes("STEP:chapter_list")) return JSON.stringify(chapters);
  if (body.includes("STEP:memory_extraction")) {
    return JSON.stringify({ facts: [{
      factType: "timeline_event", truthfulness: "narrator_fact", entityName: "林动",
      value: "E2E 记忆抽取事实：林动进入山门。", evidenceQuote: "林动踏上山门石阶", confidence: 0.9
    }] });
  }
  if (body.includes("STEP:consistency_review")) {
    return scenario === "issues"
      ? JSON.stringify({ issues: [
          { type: "timeline", severity: "high", evidence: "时间线冲突", suggestion: "修正时间" },
          { type: "character", severity: "medium", evidence: "状态冲突", suggestion: "修正状态" }
        ] })
      : JSON.stringify(cleanReview);
  }
  return "E2E 候选正文：林动踏上山门石阶，夜雨压低了灯影。";
}

export async function startMockProvider() {
  const server = http.createServer((request, response) => {
    let body = "";
    request.on("data", chunk => { body += chunk; });
    request.on("end", () => {
      const output = outputFor(body, process.env.BIZ_NOVEL_E2E_SCENARIO ?? "clean");
      const event = JSON.stringify({ choices: [{ delta: { content: output } }] });
      response.writeHead(200, { "Content-Type": "text/event-stream", "Cache-Control": "no-cache", Connection: "close" });
      response.write(`data: ${event}\n\n`);
      response.end("data: [DONE]\n\n");
    });
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  return {
    endpoint: `http://127.0.0.1:${address.port}/v1/chat/completions`,
    close: () => new Promise(resolve => server.close(resolve)),
  };
}
