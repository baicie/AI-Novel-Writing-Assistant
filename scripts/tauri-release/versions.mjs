import { readFile } from "node:fs/promises";
import path from "node:path";

const TAG_PATTERNS = [
  { kind: "nightly", re: /^tauri-nightly-(\d{8})-([0-9a-f]{7,40})$/ },
  { kind: "release", re: /^tauri-v(\d+)\.(\d+)\.(\d+)$/ },
  { kind: "prerelease", re: /^tauri-v(\d+)\.(\d+)\.(\d+)-([0-9A-Za-z.-]+)$/ },
];

export function parseReleaseTag(tag) {
  if (typeof tag !== "string" || tag.length === 0) {
    return { kind: "invalid", reason: "tag 为空" };
  }
  for (const { kind, re } of TAG_PATTERNS) {
    const match = tag.match(re);
    if (!match) continue;
    if (kind === "nightly") {
      return { kind, date: match[1], sha: match[2] };
    }
    const [, major, minor, patch, prerelease] = match;
    return {
      kind,
      version: `${major}.${minor}.${patch}${prerelease ? `-${prerelease}` : ""}`,
      prerelease: prerelease ?? null,
    };
  }
  return { kind: "invalid", reason: "tag 必须形如 tauri-v1.2.3 / tauri-v1.2.3-beta.1 / tauri-nightly-20260101-abcdef1" };
}

async function readWorkspaceVersion(root) {
  const text = await readFile(path.join(root, "Cargo.toml"), "utf8");
  const section = text.split(/^\[workspace\.package\]$/m)[1];
  if (!section) throw new Error("Cargo.toml 缺少 [workspace.package]");
  const match = section.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error("[workspace.package] 缺少 version");
  return match[1];
}

async function readJsonVersion(file) {
  const parsed = JSON.parse(await readFile(file, "utf8"));
  if (typeof parsed.version !== "string") throw new Error(`${file} 缺少 version`);
  return parsed.version;
}

export async function readVersions(root) {
  return {
    cargo: await readWorkspaceVersion(root),
    client: await readJsonVersion(path.join(root, "client/package.json")),
    tauri: await readJsonVersion(path.join(root, "src-tauri/tauri.conf.json")),
  };
}

export async function assertVersionConsistency({ root, expected }) {
  const versions = await readVersions(root);
  const errors = [];
  const unique = new Set(Object.values(versions));
  if (unique.size !== 1) {
    errors.push(`三处版本不一致：${JSON.stringify(versions)}`);
  }
  if (expected && versions.cargo !== expected) {
    errors.push(`Tag/输入版本 ${expected} 与源码版本 ${versions.cargo} 不一致`);
  }
  return { ok: errors.length === 0, versions, errors };
}
