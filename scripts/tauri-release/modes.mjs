import { parseReleaseTag } from "./versions.mjs";

export const MODES = ["unsigned-qa", "updater-signed"];

const ALLOWED_TARGETS = [
  "darwin-aarch64",
  "darwin-x86_64",
  "windows-x86_64",
  "linux-x86_64",
];

export function allowedTargets() {
  return [...ALLOWED_TARGETS];
}

export function resolveMode({ eventName, tag, inputs = {}, hasUpdaterKey, hasUpdaterPassword }) {
  const errors = [];
  const requested = String(inputs.mode ?? "").trim();
  const parsed = parseReleaseTag(tag ?? "");

  if (eventName === "push") {
    if (parsed.kind === "invalid") {
      errors.push(parsed.reason);
    }
  } else if (eventName === "workflow_dispatch") {
    if (requested && !MODES.includes(requested)) {
      errors.push(`未知模式 ${requested}，可用：${MODES.join(", ")}`);
    }
    const sha = String(inputs.source_sha ?? "").trim();
    if (!/^[0-9a-f]{7,40}$/.test(sha)) {
      errors.push("手动触发必须提供 source_sha（7-40 位十六进制），避免从任意分支产出正式版");
    }
  }

  const mode = MODES.includes(requested)
    ? requested
    : parsed.kind === "nightly"
      ? "unsigned-qa"
      : hasUpdaterKey
        ? "updater-signed"
        : "unsigned-qa";

  let channel = "prerelease";
  if (parsed.kind === "release") channel = "stable";
  else if (parsed.kind === "prerelease") channel = "prerelease";
  else if (parsed.kind === "nightly") channel = "nightly";
  else if (inputs.channel === "prerelease" || inputs.channel === "qa") channel = "prerelease";

  if (parsed.kind === "nightly") {
    if (mode !== "unsigned-qa" && !hasUpdaterKey) {
      errors.push("nightly 通道要求 unsigned-qa 模式或已配置 updater 私钥");
    }
  }

  if (channel === "stable" && mode !== "updater-signed") {
    errors.push("stable 通道必须使用 updater-signed 模式：无签名更新索引不允许进入 stable");
  }
  if (mode === "updater-signed" && !hasUpdaterKey) {
    errors.push("updater-signed 模式缺少 TAURI_SIGNING_PRIVATE_KEY，拒绝自动降级为 unsigned");
  }
  if (hasUpdaterPassword && !hasUpdaterKey) {
    errors.push("配置了 TAURI_SIGNING_PRIVATE_KEY_PASSWORD 但缺少私钥");
  }

  return {
    ok: errors.length === 0,
    mode,
    channel,
    createUpdaterArtifacts: mode === "updater-signed",
    tagKind: parsed.kind,
    version: parsed.version ?? null,
    nightlyDate: parsed.date ?? null,
    nightlySha: parsed.sha ?? null,
    errors,
  };
}
