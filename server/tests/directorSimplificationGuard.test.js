const test = require("node:test");
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

// Ratchet guard for docs/plans/auto-director-simplification-execution-plan.md.
// Each metric may only go down. When a count drops, lower the baseline in the same commit.
// Raising a baseline or editing a metric definition requires explicit human approval
// recorded in the plan's status ledger.

const repoRoot = path.resolve(__dirname, "..", "..");
const baselinePath = path.join(__dirname, "fixtures", "directorSimplification.baseline.json");

const SOURCE_EXTENSIONS = new Set([".ts", ".tsx"]);
const DIRECTOR_DIR = "server/src/services/novel/director";

const METRICS = {
  adHocCurrentTaskLookups: {
    phase: 1,
    description: "绕过 resolveCurrentDirectorTask 自行按书查找导演任务",
    dirs: ["server/src"],
    excludePrefixes: [`${DIRECTOR_DIR}/state/currentDirectorTask.ts`],
    pattern: /(findActiveTaskByNovelAndLane|findLatestVisibleTaskByNovelId|listVisibleTasksByNovelAndLane|listActiveTasksByNovelAndLane)\([^)]*["']auto_director["']/,
  },
  clientUrlTaskIdParams: {
    phase: 2,
    description: "前端把 directorTaskId / workspaceTaskId 当作 URL 参数读写",
    dirs: ["client/src"],
    excludePrefixes: ["client/src/lib/legacyTaskUrlParams.ts"],
    pattern: /["'`?&](directorTaskId|workspaceTaskId)["'`=]|withNovelEdit(Director|Workspace)TaskId/,
  },
  healOnReadCallSites: {
    phase: 3,
    description: "读取路径中调用 healAutoDirectorTaskState（读操作写数据）",
    dirs: ["server/src"],
    excludePrefixes: ["server/src/workers/"],
    pattern: /\.healAutoDirectorTaskState\(/,
  },
  directorTaskWritesOutsideState: {
    phase: 3,
    description: "director 目录中 state/ 以外直接写任务状态",
    dirs: [DIRECTOR_DIR],
    excludePrefixes: [`${DIRECTOR_DIR}/state/`],
    pattern: /\.(markTaskRunning|markTaskWaitingApproval|markTaskFailed|markTaskCompleted|markTaskSucceeded|markTaskCancelled|recordCheckpoint|bootstrapTask|updateTaskWithRetry)\(/,
  },
  directorSeedPayloadRefs: {
    phase: 3,
    description: "director 目录中 state/ 以外直接读写 seedPayload",
    dirs: [DIRECTOR_DIR],
    excludePrefixes: [`${DIRECTOR_DIR}/state/`],
    pattern: /seedPayload/,
  },
  continuationModeRefs: {
    phase: 4,
    description: "服务端 director 中按 continuationMode 分支",
    dirs: [DIRECTOR_DIR],
    excludePrefixes: [`${DIRECTOR_DIR}/commands/legacyContinuationAdapter.ts`],
    pattern: /continuationMode/,
  },
  runModeBranches: {
    phase: 5,
    description: "服务端 director 中直接按 runMode 分支",
    dirs: [DIRECTOR_DIR],
    excludePrefixes: [`${DIRECTOR_DIR}/runtime/directorRunContract.ts`],
    pattern: /isFullBookAutopilotRunMode\(|isDirectorAutoExecutionRunMode\(|runMode\s*[!=]==/,
  },
  inProcessBackgroundScheduling: {
    phase: 5,
    description: "director 中用 setImmediate / scheduleBackgroundRun 在进程内伪装后台任务",
    dirs: [DIRECTOR_DIR],
    pattern: /setImmediate\(|scheduleBackgroundRun\(/,
  },
  stepRuntimeServiceLocator: {
    phase: 5,
    description: "StepModule 通过 getDirectorCoreStepRuntime() 回调 PipelineRuntime",
    dirs: [DIRECTOR_DIR],
    pattern: /getDirectorCoreStepRuntime\(/,
  },
  clientStatusDerivation: {
    phase: 6,
    description: "前端在 api/ 与运行记录页以外自行根据 pendingManualRecovery / checkpointType / waiting_approval 推断导演状态",
    dirs: ["client/src"],
    excludePrefixes: ["client/src/api/", "client/src/pages/tasks/"],
    pattern: /pendingManualRecovery|checkpointType|["']waiting_approval["']/,
  },
  legacyRuntimeTableAccess: {
    phase: 7,
    description: "访问旧运行时表 DirectorRuntimeInstance/Command/Execution/Checkpoint/Event",
    dirs: ["server/src"],
    pattern: /\.directorRuntime(Instance|Command|Execution|Checkpoint|Event)\b/,
  },
};

function listSourceFiles(relativeDir) {
  const root = path.join(repoRoot, relativeDir);
  if (!fs.existsSync(root)) {
    return [];
  }
  const result = [];
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (entry.name === "node_modules" || entry.name === "dist") {
        continue;
      }
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(fullPath);
      } else if (SOURCE_EXTENSIONS.has(path.extname(entry.name))) {
        result.push(fullPath);
      }
    }
  };
  walk(root);
  return result.sort();
}

function measure(name) {
  const metric = METRICS[name];
  const excludePrefixes = metric.excludePrefixes ?? [];
  const perFile = {};
  let total = 0;
  for (const dir of metric.dirs) {
    for (const file of listSourceFiles(dir)) {
      const repoPath = path.relative(repoRoot, file).split(path.sep).join("/");
      if (excludePrefixes.some((prefix) => repoPath.startsWith(prefix))) {
        continue;
      }
      const matches = fs.readFileSync(file, "utf8").match(new RegExp(metric.pattern.source, "g"));
      if (matches && matches.length > 0) {
        perFile[repoPath] = matches.length;
        total += matches.length;
      }
    }
  }
  return { total, perFile };
}

function readBaseline() {
  return JSON.parse(fs.readFileSync(baselinePath, "utf8").replace(/^\uFEFF/, ""));
}

if (process.env.DIRECTOR_GUARD_PRINT === "1") {
  const snapshot = {};
  for (const name of Object.keys(METRICS)) {
    snapshot[name] = measure(name);
  }
  console.log(JSON.stringify(snapshot, null, 2));
}

test("director simplification baseline covers every metric", () => {
  const baseline = readBaseline();
  assert.deepEqual(Object.keys(baseline.metrics).sort(), Object.keys(METRICS).sort());
});

// 基线一旦滞后，ratchet 就不再防回归：它只会拦住「再涨」，却让人误以为指标已经收敛。
// 这里要求基线必须处在可解释的状态：要么没调整过，要么每条调整都带证据。
test("director simplification baseline adjustments carry evidence", () => {
  const baseline = readBaseline();
  assert.equal(typeof baseline.measuredAt, "string", "基线必须记录测量日期");
  assert.match(baseline.measuredAt, /^\d{4}-\d{2}-\d{2}$/);

  const adjustments = baseline.adjustments ?? [];
  assert.ok(Array.isArray(adjustments), "adjustments 必须是数组");
  for (const entry of adjustments) {
    assert.ok(Object.prototype.hasOwnProperty.call(METRICS, entry.metric), `未知指标：${entry.metric}`);
    assert.equal(
      baseline.metrics[entry.metric],
      entry.to,
      `指标 ${entry.metric} 的调整记录与当前值不一致：记录 ${entry.to}，基线 ${baseline.metrics[entry.metric]}`,
    );
    assert.ok(entry.from < entry.to, `指标 ${entry.metric} 的上调必须真的抬高基线`);
    assert.match(entry.measuredAt, /^\d{4}-\d{2}-\d{2}$/, "每次调整都要记录测量日期");
    assert.ok(
      typeof entry.reason === "string" && entry.reason.length >= 40,
      `指标 ${entry.metric} 的调整必须写明原因（至少 40 字），否则无法区分真实债务与误计数`,
    );
    assert.ok(
      typeof entry.evidence === "string" && entry.evidence.length > 0,
      `指标 ${entry.metric} 的调整必须给出可复核的证据`,
    );
  }
});

// 防止「基线滞后后悄悄抬阈值」：基线值必须与基准分支的实际计数一致。
// 也就是说，上调基线只能反映基准分支上已经存在的债务，不能用来给新代码开绿灯。
test("director simplification baseline matches the base branch", () => {
  const baselineRef = process.env.DIRECTOR_GUARD_BASE_REF ?? "origin/main";
  const filesByDir = new Map();
  try {
    for (const dir of new Set(Object.values(METRICS).flatMap((metric) => metric.dirs))) {
      const listing = execFileSync("git", ["ls-tree", "-r", "--name-only", baselineRef, dir], {
        cwd: repoRoot,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      });
      filesByDir.set(dir, new Set(listing.split("\n").filter(Boolean)));
    }
  } catch {
    // 没有可比基准（浅克隆、脱机或尚未 fetch）时不伪装通过，也不算失败。
    console.log(`skip: 无法读取基准分支 ${baselineRef}，跳过基线一致性校验`);
    return;
  }

  const baseline = readBaseline();

  for (const name of Object.keys(METRICS)) {
    const metric = METRICS[name];
    const excludePrefixes = metric.excludePrefixes ?? [];
    let baseTotal = 0;
    for (const dir of metric.dirs) {
      const baseFileSet = filesByDir.get(dir) ?? new Set();
      for (const repoPath of baseFileSet) {
        if (!repoPath.startsWith(`${dir}/`) || !SOURCE_EXTENSIONS.has(path.extname(repoPath))) continue;
        if (excludePrefixes.some((prefix) => repoPath.startsWith(prefix))) continue;
        let text;
        try {
          text = execFileSync("git", ["show", `${baselineRef}:${repoPath}`], {
            cwd: repoRoot,
            encoding: "utf8",
            maxBuffer: 64 * 1024 * 1024,
            stdio: ["ignore", "pipe", "ignore"],
          });
        } catch {
          continue;
        }
        baseTotal += (text.match(new RegExp(metric.pattern.source, "g")) ?? []).length;
      }
    }
    assert.equal(
      baseline.metrics[name],
      baseTotal,
      `指标 ${name} 的基线 ${baseline.metrics[name]} 与基准分支 ${baselineRef} 的实际值 ${baseTotal} 不一致。` +
        "若基准分支已经降低，请在同一提交下调基线锁定成果；若基准分支升高，请先确认那是已合入的债务，再带证据上调基线。",
    );
  }
});

for (const name of Object.keys(METRICS)) {
  test(`director simplification ratchet: ${name}`, () => {
    const baseline = readBaseline().metrics[name];
    const { total, perFile } = measure(name);
    const topFiles = Object.entries(perFile)
      .sort((left, right) => right[1] - left[1])
      .slice(0, 8)
      .map(([file, count]) => `  ${count}\t${file}`)
      .join("\n");
    assert.ok(
      total <= baseline,
      `${METRICS[name].description}：当前 ${total}，基线 ${baseline}。禁止新增，请按计划文档第 ${METRICS[name].phase} 阶段的做法处理。\n${topFiles}`,
    );
    assert.ok(
      total >= baseline,
      `${METRICS[name].description}：当前 ${total} 已低于基线 ${baseline}。请在同一提交中把 fixtures/directorSimplification.baseline.json 下调到 ${total} 锁定成果。`,
    );
  });
}
