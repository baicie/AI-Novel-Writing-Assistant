#!/usr/bin/env node
/**
 * M1 验收执行器的离线 mock 提供方。
 *
 * 只用于 `M1_DRY_RUN=1` 的本地演练：不联网、不调用付费模型、不读取任何 Key。
 * 响应按 Prompt 注册表写在提示词开头的 `STEP:` 标记选择，覆盖验收所需链路：
 * story_macro(v2) → character_cast → volume_strategy → chapter_list(v3)
 * → chapter_draft → consistency_review(v2) → chapter_repair(v2) → 复审。
 *
 * 用法：node scripts/acceptance/m1-mock-provider.mjs [port]
 */
import http from "node:http";

const story = {
  expansion: {
    expanded_premise: "边陲少年林动在雪夜拾得旧玉佩，目击老铁匠被灭口，被迫卷入一桩旧案。",
    protagonist_core: "被铁匠铺收留的边陲少年，只求安稳却因目击成为活口。",
    conflict_engine: "每揭开一层旧案，就多一个要灭口的追杀者。",
    conflict_layers: {
      external: "山门与旧案主谋的追杀",
      internal: "自保本能与追查真相的拉扯",
      relational: "对收留者的亏欠",
    },
    mystery_box: "玉佩在山门旧案里到底对应什么？",
    emotional_line: "从只想活命到主动承担",
    setpiece_seeds: ["雪夜拾玉", "隔墙目击灭口", "携玉逃亡"],
    tone_reference: "冷峻、克制、雪夜质感",
  },
  decomposition: {
    selling_point: "少年目击灭口后携玉逃亡，被旧案一路追赶",
    core_conflict: "活口与灭口者之间的追逃",
    main_hook: "会发光的旧玉佩",
    progression_loop: "发现线索 -> 被追杀 -> 逼近旧案真相",
    growth_path: "从躲藏到承担",
    major_payoffs: ["旧案主谋现身", "玉佩来历揭晓"],
    ending_flavor: "代价之后的新生",
  },
  keyEvents: [
    {
      id: "ch1-event-1",
      chapterOrder: 1,
      type: "required_event",
      description: "林动亲眼撞见老铁匠被灭口",
      participants: ["林动", "老铁匠"],
      enforcement: "hard",
    },
    {
      id: "ch1-event-2",
      chapterOrder: 1,
      type: "ordering_constraint",
      description: "先发现雪夜异响，再撞见灭口",
      participants: ["林动"],
      enforcement: "hard",
    },
    {
      id: "ch2-event-1",
      chapterOrder: 2,
      type: "required_event",
      description: "林动开始追查旧案线索",
      participants: ["林动"],
      enforcement: "hard",
    },
  ],
  constraints: ["不提前揭示旧案主谋", "第1章必须目击灭口"],
  issues: [],
};

const cast = {
  members: [
    {
      name: "林动",
      role: "主角",
      castRole: "protagonist",
      storyFunction: "承载目击与追查",
      personality: "谨慎、执拗",
      outerGoal: "活着离开边陲",
      innerNeed: "为收留者讨一个说法",
    },
    {
      name: "老铁匠",
      role: "收留者",
      castRole: "supporting",
      storyFunction: "触发旧案",
      personality: "沉默、护短",
      outerGoal: "把玉佩藏好",
      innerNeed: "让林动离开边陲",
    },
    {
      name: "灰斗篷人",
      role: "灭口者",
      castRole: "antagonist",
      storyFunction: "持续施压",
      personality: "冷静、利落",
      outerGoal: "抹掉活口与玉佩",
      innerNeed: "替主谋收尾",
    },
  ],
  relations: [
    {
      sourceName: "林动",
      targetName: "老铁匠",
      surfaceRelation: "师徒",
      conflictSource: "老铁匠隐瞒玉佩来历",
    },
    {
      sourceName: "林动",
      targetName: "灰斗篷人",
      surfaceRelation: "追杀与被追杀",
      conflictSource: "目击灭口",
    },
  ],
};

const volume = {
  volumes: [
    {
      volumeOrder: 1,
      title: "雪夜卷",
      roleLabel: "开局",
      coreReward: "目击与逃亡",
      planningMode: "hard",
    },
  ],
};

const chapters = {
  chapters: [
    {
      chapterOrder: 1,
      title: "第一章 雪夜",
      volumeTitle: "雪夜卷",
      summary: "林动在雪夜发现玉佩，随后亲眼撞见老铁匠被灭口，被迫逃亡。",
      characterNames: ["林动", "老铁匠", "灰斗篷人"],
      obligations: [
        {
          id: "ch1-req-1",
          type: "required_event",
          description: "林动亲眼撞见老铁匠被灭口",
          enforcement: "hard",
          participants: ["林动", "老铁匠"],
          source: "story_macro.keyEvents",
          evidence: "第1章目标",
          sourceEventId: "ch1-event-1",
        },
        {
          id: "ch1-ord-1",
          type: "ordering_constraint",
          description: "先发现雪夜异响，再撞见灭口",
          enforcement: "hard",
          participants: ["林动"],
          source: "story_macro.keyEvents",
          evidence: "第1章目标",
          sourceEventId: "ch1-event-2",
        },
        {
          id: "ch1-adv-1",
          type: "continuity_constraint",
          description: "可以补一句雪夜环境描写",
          enforcement: "advisory",
          participants: ["林动"],
          source: "chapter_plan",
          evidence: "文风建议",
        },
      ],
    },
    {
      chapterOrder: 2,
      title: "第二章 追查",
      volumeTitle: "雪夜卷",
      summary: "林动开始追查旧案线索。",
      characterNames: ["林动"],
      obligations: [
        {
          id: "ch2-req-1",
          type: "required_event",
          description: "林动开始追查旧案线索",
          enforcement: "hard",
          participants: ["林动"],
          source: "story_macro.keyEvents",
          evidence: "第2章目标",
          sourceEventId: "ch2-event-1",
        },
      ],
    },
  ],
};

// 故意违反 hard 义务：写成“事后到达”。
const violatingDraft =
  "林动赶到铁匠铺时，雪已经停了。院门半开着，老铁匠倒在炉子旁边，人早就没气了。";

const issue = {
  issues: [
    {
      type: "obligation_violation",
      severity: "high",
      evidence: "老铁匠倒在炉子旁边，人早就没气了",
      suggestion: "改为林动亲眼撞见老铁匠被灭口",
      obligationId: "ch1-req-1",
    },
  ],
};

const repaired =
  "雪夜里，林动先听见铁匠铺后巷传来一声闷响。他绕过墙豁口，正看见灰斗篷人举刀，" +
  "老铁匠倒在炉边，血在雪地上散开。林动亲眼撞见了老铁匠被灭口的一刻，" +
  "脚下一块冻雪被踩碎，灰斗篷人猛地抬头。";

const clean = { issues: [] };

const state = { reviews: 0 };

function responseFor(prompt) {
  if (prompt.includes("STEP:story_macro")) return story;
  if (prompt.includes("STEP:character_cast")) return cast;
  if (prompt.includes("STEP:volume_strategy")) return volume;
  if (prompt.includes("STEP:chapter_list")) return chapters;
  if (prompt.includes("STEP:chapter_draft")) return violatingDraft;
  if (prompt.includes("STEP:chapter_repair")) return repaired;
  if (prompt.includes("STEP:consistency_review")) {
    state.reviews += 1;
    return state.reviews === 1 ? issue : clean;
  }
  return clean;
}

function sseChunk(payload) {
  return `data: ${JSON.stringify(payload)}\n\n`;
}

function answer(res, value) {
  const content = typeof value === "string" ? value : JSON.stringify(value);
  const body = [
    sseChunk({ choices: [{ delta: { content } }] }),
    sseChunk({ usage: { prompt_tokens: 120, completion_tokens: 80 } }),
    "data: [DONE]\n\n",
  ].join("");
  res.writeHead(200, { "content-type": "text/event-stream", "content-length": Buffer.byteLength(body) });
  res.end(body);
}

const server = http.createServer((req, res) => {
  if (req.method === "GET") {
    const body = JSON.stringify({ data: [{ id: "dry-run-model" }] });
    res.writeHead(200, { "content-type": "application/json", "content-length": Buffer.byteLength(body) });
    res.end(body);
    return;
  }
  let raw = "";
  req.on("data", (chunk) => {
    raw += chunk;
  });
  req.on("end", () => {
    let prompt = raw;
    try {
      const body = JSON.parse(raw);
      prompt = (body.messages ?? []).map((message) => String(message.content ?? "")).join("\n");
    } catch {
      // 保持原始请求体作为提示词，和真实提供方的宽松解析一致。
    }
    answer(res, responseFor(prompt));
  });
});

const port = Number(process.argv[2] ?? 18766);
server.listen(port, "127.0.0.1", () => {
  process.stderr.write(`[m1-mock] listening on http://127.0.0.1:${port}/v1\n`);
});
