import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";
import { allowedTargets } from "./modes.mjs";

export async function sha256File(file) {
  const bytes = await readFile(file);
  return createHash("sha256").update(bytes).digest("hex");
}

export async function sha256SumsText(entries) {
  const lines = [];
  for (const entry of entries) {
    const hash = await sha256File(entry.path);
    lines.push(`${hash}  ${entry.name ?? path.basename(entry.path)}`);
  }
  lines.sort();
  return `${lines.join("\n")}\n`;
}

export function buildLatestJson({ version, pubDate, platforms }) {
  const errors = [];
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version ?? "")) {
    errors.push(`版本号不是 SemVer：${version}`);
  }
  if (!pubDate || Number.isNaN(Date.parse(pubDate))) {
    errors.push("pub_date 必须是可解析的时间");
  }
  const allowed = new Set(allowedTargets());
  const normalized = {};
  for (const [target, entry] of Object.entries(platforms ?? {})) {
    if (!allowed.has(target)) {
      errors.push(`未知平台标识：${target}（可用：${[...allowed].join(", ")}）`.replace("）", ")"));
      continue;
    }
    if (!entry?.signature) errors.push(`${target} 缺少签名`);
    if (!entry?.url) errors.push(`${target} 缺少下载地址`);
    normalized[target] = { signature: entry?.signature ?? "", url: entry?.url ?? "" };
  }
  if (Object.keys(normalized).length === 0) {
    errors.push("latest.json 至少需要一个平台条目");
  }
  return {
    ok: errors.length === 0,
    errors,
    manifest: { version, pub_date: pubDate, platforms: normalized },
  };
}

export async function verifyArtifacts(entries) {
  const missing = [];
  const present = [];
  for (const entry of entries) {
    const file = entry.path;
    try {
      const info = await stat(file);
      if (!info.isFile() || info.size === 0) throw new Error("空文件");
      present.push({ name: entry.name ?? path.basename(file), bytes: info.size });
    } catch (error) {
      missing.push({ name: entry.name ?? path.basename(file), reason: error.message });
    }
  }
  return { ok: missing.length === 0, present, missing };
}
