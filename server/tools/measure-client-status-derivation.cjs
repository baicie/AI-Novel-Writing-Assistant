#!/usr/bin/env node
/**
 * 测量 clientStatusDerivation 的当前分布，供 ratchet 基线与人工复核使用。
 *
 * 用法：
 *   node server/tools/measure-client-status-derivation.cjs           # 当前工作区
 *   node server/tools/measure-client-status-derivation.cjs <git-ref> # 指定提交（用 git show 读取）
 *
 * 与 server/tests/directorSimplificationGuard.test.js 使用同一套匹配规则与排除范围，
 * 避免「测试里一个口径、复核时另一个口径」。
 */
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..", "..");
const PATTERN = /pendingManualRecovery|checkpointType|["']waiting_approval["']/g;
const EXCLUDE_PREFIXES = ["client/src/api/", "client/src/pages/tasks/"];
const EXTENSIONS = new Set([".ts", ".tsx"]);

function listFiles(dir) {
  const result = [];
  const walk = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (entry.name === "node_modules" || entry.name === "dist") continue;
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (EXTENSIONS.has(path.extname(entry.name))) result.push(full);
    }
  };
  walk(dir);
  return result.sort();
}

function measureFromWorktree() {
  const perFile = {};
  let total = 0;
  for (const file of listFiles(path.join(repoRoot, "client", "src"))) {
    const repoPath = path.relative(repoRoot, file).split(path.sep).join("/");
    if (EXCLUDE_PREFIXES.some((prefix) => repoPath.startsWith(prefix))) continue;
    const matches = fs.readFileSync(file, "utf8").match(new RegExp(PATTERN.source, "g"));
    if (matches?.length) {
      perFile[repoPath] = matches.length;
      total += matches.length;
    }
  }
  return { total, perFile };
}

function measureFromRef(ref) {
  const files = execFileSync("git", ["ls-tree", "-r", "--name-only", ref, "client/src"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).split("\n");
  const perFile = {};
  let total = 0;
  for (const repoPath of files) {
    if (!EXTENSIONS.has(path.extname(repoPath))) continue;
    if (EXCLUDE_PREFIXES.some((prefix) => repoPath.startsWith(prefix))) continue;
    const text = execFileSync("git", ["show", `${ref}:${repoPath}`], {
      cwd: repoRoot,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    });
    const matches = text.match(new RegExp(PATTERN.source, "g"));
    if (matches?.length) {
      perFile[repoPath] = matches.length;
      total += matches.length;
    }
  }
  return { total, perFile };
}

const ref = process.argv[2];
const result = ref ? measureFromRef(ref) : measureFromWorktree();
console.log(JSON.stringify({ ref: ref ?? "working-tree", ...result }, null, 2));
