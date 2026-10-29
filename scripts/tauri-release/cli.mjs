import { readdir, readFile, writeFile, appendFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { parseReleaseTag, assertVersionConsistency } from "./versions.mjs";
import { resolveMode } from "./modes.mjs";
import { buildLatestJson, sha256SumsText, verifyArtifacts } from "./manifest.mjs";

const TARGET_RULES = [
  { target: "darwin-aarch64", match: /aarch64\.app\.tar\.gz$/ },
  { target: "darwin-x86_64", match: /(x64|x86_64)\.app\.tar\.gz$/ },
  { target: "windows-x86_64", match: /(x64|x86_64)[-_].*(nsis\.zip|msi\.zip)$/ },
  { target: "linux-x86_64", match: /(amd64|x86_64)\.AppImage\.tar\.gz$/ },
];

export function detectTarget(fileName) {
  for (const rule of TARGET_RULES) {
    if (rule.match.test(fileName)) return rule.target;
  }
  return null;
}

function argValue(argv, name, fallback = undefined) {
  const index = argv.indexOf(`--${name}`);
  if (index === -1) return fallback;
  return argv[index + 1];
}

async function writeGithubOutput(lines) {
  const file = process.env.GITHUB_OUTPUT;
  const text = Object.entries(lines)
    .map(([key, value]) => `${key}=${value}`)
    .join("\n") + "\n";
  if (file) await appendFile(file, text);
  process.stdout.write(text);
}

async function cmdResolveMode(argv) {
  const tag = argValue(argv, "tag", process.env.GITHUB_REF_NAME ?? "");
  const eventName = argValue(argv, "event", process.env.GITHUB_EVENT_NAME ?? "workflow_dispatch");
  const result = resolveMode({
    eventName,
    tag,
    inputs: {
      mode: argValue(argv, "mode", ""),
      channel: argValue(argv, "channel", ""),
      source_sha: argValue(argv, "source-sha", ""),
    },
    hasUpdaterKey: Boolean(process.env.TAURI_SIGNING_PRIVATE_KEY),
    hasUpdaterPassword: Boolean(process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD),
  });
  if (!result.ok) {
    process.stderr.write(`[release] 模式解析失败：\n- ${result.errors.join("\n- ")}\n`);
    process.exit(1);
  }
  await writeGithubOutput({
    mode: result.mode,
    channel: result.channel,
    create_updater_artifacts: result.createUpdaterArtifacts,
    version: result.version ?? "",
    nightly_date: result.nightlyDate ?? "",
    nightly_sha: result.nightlySha ?? "",
  });
}

async function cmdCheckVersions(argv) {
  const root = argValue(argv, "root", process.cwd());
  const expected = argValue(argv, "expected", "");
  const result = await assertVersionConsistency({ root, expected: expected || undefined });
  if (!result.ok) {
    process.stderr.write(`[release] 版本校验失败：\n- ${result.errors.join("\n- ")}\n`);
    process.exit(1);
  }
  process.stdout.write(`[release] 版本一致：${JSON.stringify(result.versions)}\n`);
}

async function cmdWriteSums(argv) {
  const dir = argValue(argv, "dir", ".");
  const out = argValue(argv, "out", path.join(dir, "SHA256SUMS.txt"));
  const entries = (await readdir(dir, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && entry.name !== path.basename(out))
    .map((entry) => ({ path: path.join(dir, entry.name), name: entry.name }));
  if (entries.length === 0) {
    process.stderr.write("[release] 没有可计算校验和的产物\n");
    process.exit(1);
  }
  const text = await sha256SumsText(entries);
  await writeFile(out, text);
  process.stdout.write(`[release] 已写入 ${out}（${entries.length} 个文件）\n`);
}

export async function collectUpdaterPlatforms({ dir, baseUrl }) {
  const names = await readdir(dir);
  const platforms = {};
  const errors = [];
  for (const name of names.filter((item) => item.endsWith(".sig"))) {
    const archive = name.slice(0, -4);
    if (!names.includes(archive)) {
      errors.push(`${name} 缺少对应的更新包 ${archive}`);
      continue;
    }
    const target = detectTarget(archive);
    if (!target) {
      errors.push(`无法从 ${archive} 判断平台，拒绝生成 latest.json`);
      continue;
    }
    if (platforms[target]) {
      errors.push(`平台 ${target} 出现多个更新包：${platforms[target].file} 与 ${archive}`);
      continue;
    }
    const signature = (await readFile(path.join(dir, name), "utf8")).trim();
    platforms[target] = {
      signature,
      url: `${baseUrl.replace(/\/$/, "")}/${encodeURIComponent(archive)}`,
      file: archive,
    };
  }
  if (Object.keys(platforms).length === 0) {
    errors.push("没有找到任何 .sig 更新包，updater-signed 模式下不允许生成空的 latest.json");
  }
  return { platforms, errors };
}

async function cmdWriteLatest(argv) {
  const dir = argValue(argv, "dir", ".");
  const version = argValue(argv, "version", "");
  const baseUrl = argValue(argv, "base-url", "");
  const pubDate = argValue(argv, "pub-date", new Date().toISOString());
  const out = argValue(argv, "out", path.join(dir, "latest.json"));
  if (!baseUrl) {
    process.stderr.write("[release] 必须提供 --base-url（Release 资产下载前缀）\n");
    process.exit(1);
  }
  const { platforms, errors } = await collectUpdaterPlatforms({ dir, baseUrl });
  const built = buildLatestJson({ version, pubDate, platforms });
  const allErrors = [...errors, ...built.errors];
  if (allErrors.length > 0) {
    process.stderr.write(`[release] latest.json 生成失败：\n- ${allErrors.join("\n- ")}\n`);
    process.exit(1);
  }
  await writeFile(out, `${JSON.stringify(built.manifest, null, 2)}\n`);
  process.stdout.write(`[release] 已写入 ${out}：${Object.keys(built.manifest.platforms).join(", ")}\n`);
}

async function cmdVerifyArtifacts(argv) {
  const dir = argValue(argv, "dir", "");
  const names = argValue(argv, "names", "");
  const entries = names
    ? names.split(",").filter(Boolean).map((name) => ({ path: path.join(dir, name), name }))
    : [];
  if (entries.length === 0) {
    process.stderr.write("[release] 必须提供 --names 或 --dir\n");
    process.exit(1);
  }
  const result = await verifyArtifacts(entries);
  if (!result.ok) {
    process.stderr.write(`[release] 产物缺失：\n- ${result.missing.map((item) => `${item.name}: ${item.reason}`).join("\n- ")}\n`);
    process.exit(1);
  }
  process.stdout.write(`[release] 产物齐全：${result.present.map((item) => `${item.name}(${item.bytes}B)`).join(", ")}\n`);
}

async function cmdParseTag(argv) {
  const tag = argValue(argv, "tag", process.env.GITHUB_REF_NAME ?? "");
  const parsed = parseReleaseTag(tag);
  if (parsed.kind === "invalid") {
    process.stderr.write(`[release] Tag 不合法：${parsed.reason}\n`);
    process.exit(1);
  }
  process.stdout.write(`${JSON.stringify(parsed)}\n`);
}

const COMMANDS = {
  "resolve-mode": cmdResolveMode,
  "check-versions": cmdCheckVersions,
  "write-sums": cmdWriteSums,
  "write-latest": cmdWriteLatest,
  "verify-artifacts": cmdVerifyArtifacts,
  "parse-tag": cmdParseTag,
};

const isDirectRun = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
if (isDirectRun) {
  const [, , command, ...rest] = process.argv;
  if (!COMMANDS[command]) {
    process.stderr.write(`用法：node scripts/tauri-release/cli.mjs <${Object.keys(COMMANDS).join("|")}> [--参数 值]\n`);
    process.exit(2);
  }
  await COMMANDS[command](rest);
}
