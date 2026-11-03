const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const serverRoot = path.resolve(__dirname, "..");
const testsRoot = path.join(serverRoot, "tests");

const integrationTests = new Set([
  "directorTaskFactInspection.test.js",
  "directorCurrentTask.test.js",
  "directorReadPathsArePure.test.js",
  "directorManualRecoveryLock.test.js",
  "directorResourceConfirmation.test.js",
  "directorVersionIsolationSqlite.test.js",
  "directorWorkflowStepModules.test.js",
  "persistence.test.js",
  "directorNextReadOnlySqlite.test.js",
  "commandRepositorySqlite.test.js",
  "batchProcessRecoverySqlite.test.js",
  "confirmationRoutesSqlite.test.js",
  "gateProcessRecoverySqlite.test.js",
  "gateService.test.js",
  "openRunSwitchSqlite.test.js",
  "planningInventorySqlite.test.js",
  "processRecoverySqlite.test.js",
  "productionProgressSqlite.test.js",
  "recoveryBudgetSqlite.test.js",
  "readProjection.test.js",
  "commandService.test.js",
  "runExecutor.test.js",
  "worker.test.js",
  "runtime.test.js",
  "directorNextHttp.test.js",
  "novelDirectorPipelineRuntime.test.js",
  "novelDirectorRetry.test.js",
  "novelWorkflowRuntime.test.js",
  "p0bRealPrismaChain.test.js",
  "marketRadarRepeatAnalysis.test.js",
  "prompting-governance.test.js",
  "prompting.test.js",
  "promptWorkbench.test.js",
  "ragCompatibilityBootstrap.test.js",
  "runtimeMigrations.test.js",
]);

function listTestFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const fullPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        return listTestFiles(fullPath);
      }
      return entry.isFile() && entry.name.endsWith(".test.js") ? [fullPath] : [];
    })
    .sort((left, right) => left.localeCompare(right));
}

function selectTestFiles(mode) {
  const allFiles = listTestFiles(testsRoot);
  if (mode === "integration") {
    return allFiles.filter((file) => integrationTests.has(path.basename(file)));
  }
  if (mode === "fast") {
    return allFiles.filter((file) => !integrationTests.has(path.basename(file)));
  }
  if (mode === "all") {
    return allFiles;
  }
  throw new Error(`Unknown test mode: ${mode}`);
}

const mode = process.argv[2] ?? "fast";
const files = selectTestFiles(mode);

if (files.length === 0) {
  console.error(`No tests selected for mode ${mode}.`);
  process.exit(1);
}

// `node:sqlite` 在 Node 22 是实验特性，需要显式开启；Node 24 起默认可用；
// Node 20 完全没有这个内置模块。这里按能力探测决定是否加参数：
// - 已能 require：不加参数；
// - 加参数后可 require：加 --experimental-sqlite；
// - 都不行：直接失败并说明原因，绝不静默跳过依赖 SQLite 的用例。
const SQLITE_TESTS = ["automaticAttempt.test.js", "pipelineExecutionLease.test.js"];
const usesSqlite = files.some((file) => SQLITE_TESTS.includes(path.basename(file)));

function probeSqlite(extraFlags) {
  const probe = spawnSync(process.execPath, [...extraFlags, "-e", 'require("node:sqlite")'], {
    cwd: serverRoot,
    stdio: "ignore",
  });
  return probe.status === 0;
}

const nodeFlags = [];
if (usesSqlite && !probeSqlite([])) {
  if (probeSqlite(["--experimental-sqlite"])) {
    nodeFlags.push("--experimental-sqlite");
  } else {
    console.error(
      [
        `当前 Node ${process.versions.node} 无法加载 node:sqlite，依赖它的测试不能被静默跳过：`,
        ...SQLITE_TESTS.map((name) => `  - ${name}`),
        "请使用 Node 24（默认支持），或 Node 22 且支持 --experimental-sqlite 的版本。",
      ].join("\n"),
    );
    process.exit(1);
  }
}

// File isolation prevents cache replacements and root hooks from leaking across tests.
const result = spawnSync(
  process.execPath,
  [...nodeFlags, "--test", "--test-concurrency=4", ...files.map((file) => path.relative(serverRoot, file))],
  {
    cwd: serverRoot,
    stdio: "inherit",
  },
);

if (result.error) {
  console.error(`Unable to start tests: ${result.error.message}`);
}
process.exit(result.status ?? 1);
