/**
 * 桌面适配层的最小纯函数：不依赖任何运行时模块，方便直接单测。
 */

import type { ApiResponse } from "@ai-novel/shared/types/api";

export type JsonRecord = Record<string, unknown>;

export function iso(timestamp: number): string {
  return new Date(timestamp).toISOString();
}

export function asRecord(value: unknown): JsonRecord {
  if (typeof value === "string") {
    try {
      const parsed: unknown = JSON.parse(value);
      return parsed && typeof parsed === "object" ? (parsed as JsonRecord) : {};
    } catch {
      return {};
    }
  }
  return value && typeof value === "object" ? (value as JsonRecord) : {};
}

export function appError(error: unknown): { code: string; message: string; retryable: boolean } {
  const record = asRecord(error);
  return {
    code: typeof record.code === "string" ? record.code : "INTEGRITY_ERROR",
    message: typeof record.message === "string" ? record.message : "本地创作内核没有完成这次操作。",
    retryable: record.retryable === true,
  };
}

export function ok<T>(data: T): ApiResponse<T> {
  return { success: true, data };
}

export function taskStatusForRun(
  state: string,
): "queued" | "running" | "waiting_approval" | "succeeded" | "failed" | "cancelled" {
  switch (state) {
    case "running":
      return "running";
    case "paused":
      return "waiting_approval";
    case "succeeded":
      return "succeeded";
    case "failed":
      return "failed";
    case "cancelled":
    case "canceled":
      return "cancelled";
    default:
      return "queued";
  }
}

/**
 * 桌面端是否把这次「创建快照」请求降级为章节修订保护。
 *
 * 只有编辑器内部的保护性调用（采纳候选、载入版本前）才降级；
 * 作者主动创建整书快照时内核没有等价能力，必须走「未迁移」而不是伪造成功。
 */
export function isEditorSnapshotRequest(label: unknown): boolean {
  return typeof label === "string" && label.startsWith("chapter-editor:");
}
