import { readdir, readFile, writeFile, appendFile, rename } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { parseReleaseTag, assertVersionConsistency, readVersions } from "./versions.mjs";
import { resolveMode } from "./modes.mjs";
import { buildLatestJson, sha256SumsText, verifyArtifacts } from "./manifest.mjs";

const TARGET_RULES = [
  { target: "darwin-aarch64", match: /aarch64\.app\.tar\.gz$/ },
  { target: "darwin-x86_64", match: /(x64|x86_64)\.app\.tar\.gz$/ },
  // Tauri v2 的更新包就是安装包本身（v1 的 .nsis.zip / .AppImage.tar.gz 仍兼容）
  { target: "windows-x86_64", match: /(x64|x86_64)[-_].*(nsis\.zip|msi\.zip)$/ },
  { target: "windows-x86_64", match: /(x64|x86_64)[-_].*(setup\.exe|msi)$/ },
  { target: "linux-x86_64", match: /(amd64|x86_64)\.AppImage(\.tar\.gz)?$/ },
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

// 只认真正的分发产物：deb 控制包（control.tar.gz / data.tar.gz）和 AppImage 内部文件都不能被上传。
export const RELEASE_ASSET_SUFFIXES = [
  ".dmg",
  ".exe",
  ".msi",
  ".deb",
  ".AppImage",
  ".app.tar.gz",
  ".AppImage.tar.gz",
  ".nsis.zip",
  ".msi.zip",
  ".sig",
];


// download-artifact 保留了 target/<triple>/... 的路径，三元组是判断架构最可靠的依据。
export const TRIPLE_TO_TARGET = {
  'aarch64-apple-darwin': 'darwin-aarch64',
  'x86_64-apple-darwin': 'darwin-x86_64',
  'x86_64-pc-windows-msvc': 'windows-x86_64',
  'x86_64-unknown-linux-gnu': 'linux-x86_64',
};

const TRIPLE_TO_ARCH = {
  'aarch64-apple-darwin': 'aarch64',
  'x86_64-apple-darwin': 'x64',
  'x86_64-pc-windows-msvc': 'x64',
  'x86_64-unknown-linux-gnu': 'amd64',
};

const UPDATER_ARCHIVE_SUFFIXES = [
  '.app.tar.gz',
  '.AppImage',
  '.AppImage.tar.gz',
  '.nsis.zip',
  '.msi.zip',
  '.msi',
  '-setup.exe',
];

export function updaterArchiveSuffix(name) {
  return UPDATER_ARCHIVE_SUFFIXES.find((suffix) => name.endsWith(suffix)) ?? null;
}

export function tripleFromPath(filePath) {
  // upload-artifact 按路径列表的公共前缀裁剪，落地后可能没有 target/ 前缀，
  // 因此直接匹配已知三元组，而不是依赖固定前缀。
  const match =
    /(?:^|\/)((?:aarch64|x86_64|i686|universal)-(?:apple-darwin|unknown-linux-gnu|pc-windows-msvc))\//.exec(
      filePath,
    );
  return match ? match[1] : null;
}

export function targetFromPath(filePath) {
  const triple = tripleFromPath(filePath);
  return triple ? (TRIPLE_TO_TARGET[triple] ?? null) : null;
}

export function normalizeUpdaterName(base, triple) {
  const slug = base.replace(/\s+/g, '_');
  const arch = triple ? TRIPLE_TO_ARCH[triple] : null;
  if (!arch) return slug;
  if (/(aarch64|x86_64|x64|amd64|arm64)[._-]/i.test(slug)) return slug;
  const suffix = updaterArchiveSuffix(slug);
  if (!suffix) return slug;
  return slug.slice(0, -suffix.length) + '_' + arch + suffix;
}

async function cmdNormalizeUpdater(argv) {
  const dir = argValue(argv, 'dir', 'artifacts');
  const files = (await collectFiles(dir)).filter((entry) => !entry.isDirectory);
  const sigs = files.filter((entry) => entry.name.endsWith('.sig'));
  const moves = [];
  for (const sig of sigs) {
    const archiveName = sig.name.slice(0, -4);
    if (!updaterArchiveSuffix(archiveName)) continue;
    // 同名文件可能出现在不同 target/<triple> 目录下，必须按同级路径取，不能按文件名找。
    const archivePath = sig.path.slice(0, -4);
    const archive = files.find((entry) => entry.path === archivePath);
    if (!archive) continue;
    const triple = tripleFromPath(archive.path);
    const normalized = normalizeUpdaterName(archiveName, triple);
    moves.push({ archive, sig, normalized });
  }
  if (moves.length === 0) {
    process.stdout.write('[release] 没有需要规范化的更新包\n');
    return;
  }
  for (const move of moves) {
    const target = path.join(dir, move.normalized);
    if (move.archive.path !== target) await rename(move.archive.path, target);
    const sigTarget = target + '.sig';
    if (move.sig.path !== sigTarget) await rename(move.sig.path, sigTarget);
    process.stdout.write(`[release] 更新包 ${move.archive.name} -> ${move.normalized}\n`);
  }
}

export const RELEASE_MANIFEST_NAMES = ["SHA256SUMS.txt", "latest.json", "nightly-latest.json"];

export function isReleaseAsset(name, suffixes = RELEASE_ASSET_SUFFIXES) {
  return suffixes.some((suffix) => name.endsWith(suffix));
}

async function cmdWriteSums(argv) {
  const dir = argValue(argv, "dir", ".");
  const out = argValue(argv, "out", path.join(dir, "SHA256SUMS.txt"));
  const only = argValue(argv, "extensions", "");
  const extensions = only ? only.split(",").map((item) => item.trim()).filter(Boolean) : null;
  const outName = path.basename(out);
  // bundle 目录是按平台/格式分层的（macos/*.app、deb/*.deb …），必须递归收集文件。
  const entries = (await collectFiles(dir))
    .filter((entry) => !entry.isDirectory && entry.name !== outName)
    .filter((entry) => (extensions ? isReleaseAsset(entry.name, extensions) : true))
    .map((entry) => ({ path: entry.path, name: entry.name }));
  if (entries.length === 0) {
    process.stderr.write("[release] 没有可计算校验和的产物\n");
    process.exit(1);
  }
  const text = await sha256SumsText(entries);
  await writeFile(out, text);
  process.stdout.write(`[release] 已写入 ${out}（${entries.length} 个文件）\n`);
}

export async function collectUpdaterPlatforms({ dir, baseUrl }) {
  // 下载下来的产物保留了工作区相对路径（target/<triple>/release/bundle/...），必须递归查找。
  const files = (await collectFiles(dir)).filter((entry) => !entry.isDirectory);
  const byName = new Map(files.map((entry) => [entry.name, entry.path]));
  const platforms = {};
  const errors = [];
  for (const [name, fullPath] of byName) {
    if (!name.endsWith(".sig")) continue;
    const archive = name.slice(0, -4);
    // 安装包（dmg/deb/exe）也会被签名，但只有更新包能进 latest.json。
    if (!updaterArchiveSuffix(archive)) continue;
    if (!byName.has(archive)) {
      errors.push(`${name} 缺少对应的更新包 ${archive}`);
      continue;
    }
    const target = targetFromPath(fullPath) ?? detectTarget(archive);
    if (!target) {
      errors.push(`无法从 ${archive} 判断平台，拒绝生成 latest.json`);
      continue;
    }
    if (platforms[target]) {
      errors.push(`平台 ${target} 出现多个更新包：${platforms[target].file} 与 ${archive}`);
      continue;
    }
    const signature = (await readFile(fullPath, "utf8")).trim();
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

async function cmdListReleaseAssets(argv) {
  const dir = argValue(argv, "dir", "artifacts");
  const files = (await collectFiles(dir))
    .filter((entry) => !entry.isDirectory)
    .filter((entry) => {
      // 只有更新包的签名需要上传，安装包的 .sig 对客户端没有意义。
      if (entry.name.endsWith(".sig")) return Boolean(updaterArchiveSuffix(entry.name.slice(0, -4)));
      if (RELEASE_MANIFEST_NAMES.includes(entry.name)) {
        // 每个平台目录里也有一份 SHA256SUMS.txt，只有汇总目录那份该上传，
        // 否则同名资产会在上传时触发 HTTP 422。
        return path.dirname(entry.path) === dir;
      }
      return isReleaseAsset(entry.name);
    })
    .map((entry) => path.relative(dir, entry.path))
    .sort();
  if (files.length === 0) {
    process.stderr.write("[release] 没有找到可上传的发布产物\n");
    process.exit(1);
  }
  process.stdout.write(files.join("\n") + "\n");
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

const KIND_RULES = {
  app: /\.app$/,
  dmg: /\.dmg$/,
  nsis: /-setup\.exe$/,
  msi: /\.msi$/,
  deb: /\.deb$/,
  appimage: /\.AppImage$/,
  "updater-sig": /\.sig$/,
};

export async function assertBundles({ dir, kinds }) {
  const found = await collectFiles(dir);
  const missing = [];
  const matched = {};
  for (const kind of kinds) {
    const rule = KIND_RULES[kind];
    const hits = rule ? found.filter((file) => rule.test(file.name)) : [];
    matched[kind] = hits.map((file) => file.name);
    if (hits.length === 0) missing.push(kind);
  }
  return { ok: missing.length === 0, matched, missing };
}

async function collectFiles(dir) {
  const out = [];
  const stack = [dir];
  while (stack.length > 0) {
    const current = stack.pop();
    const entries = await readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        // macOS 的 .app 是目录，产物检查必须能匹配目录名；但不展开它的内部文件，
        // 否则校验和清单会把整个 app 包里上百个文件都列出来。
        out.push({ name: entry.name, path: full, isDirectory: true });
        if (!entry.name.endsWith(".app")) stack.push(full);
      } else {
        out.push({ name: entry.name, path: full, isDirectory: false });
      }
    }
  }
  return out;
}

async function cmdAssertBundles(argv) {
  const dir = argValue(argv, "dir", "");
  const kinds = String(argValue(argv, "kinds", "")).split(",").map((item) => item.trim()).filter(Boolean);
  const known = kinds.filter((kind) => !KIND_RULES[kind]);
  if (known.length > 0) {
    process.stderr.write(`[release] 未知产物类型：${known.join(", ")}（可用：${Object.keys(KIND_RULES).join(", ")}）\n`);
    process.exit(1);
  }
  const result = await assertBundles({ dir, kinds });
  if (!result.ok) {
    process.stderr.write(`[release] 缺少产物：${result.missing.join(", ")}（目录 ${dir}）\n`);
    process.exit(1);
  }
  process.stdout.write(`[release] 产物齐全：${JSON.stringify(result.matched)}\n`);
}

async function cmdCheckWorkflows() {
  const { checkWorkflows } = await import("./check-workflows.mjs");
  const result = await checkWorkflows(process.cwd());
  if (!result.ok) {
    process.stderr.write(`[release] workflow 自检失败：\n- ${result.errors.join("\n- ")}\n`);
    process.exit(1);
  }
  process.stdout.write("[release] workflow 自检通过\n");
}

async function cmdReadBaseVersion() {
  const versions = await readVersions(process.cwd());
  process.stdout.write(versions.cargo);
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
  "normalize-updater": cmdNormalizeUpdater,
  "list-release-assets": cmdListReleaseAssets,
  "assert-bundles": cmdAssertBundles,
  "read-base-version": cmdReadBaseVersion,
  "check-workflows": cmdCheckWorkflows,
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
