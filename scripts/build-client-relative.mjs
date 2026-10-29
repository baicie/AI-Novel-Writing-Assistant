#!/usr/bin/env node
// Tauri 的 beforeBuildCommand 在各平台用不同 shell 执行，
// 这里用 Node 统一设置 AI_NOVEL_CLIENT_BASE=relative 再调用 client 构建，避免 Windows 上 bash 语法失败。
import { spawnSync } from "node:child_process";

const result = spawnSync("pnpm", ["--filter", "@ai-novel/client", "build"], {
  stdio: "inherit",
  env: { ...process.env, AI_NOVEL_CLIENT_BASE: "relative" },
  shell: process.platform === "win32",
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
