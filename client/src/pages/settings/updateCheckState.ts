export type UpdatePhase =
  | "idle"
  | "checking"
  | "up-to-date"
  | "available"
  | "downloading"
  | "ready"
  | "error";

export interface UpdateProgress {
  downloadedBytes: number;
  totalBytes: number;
}

export interface UpdateState {
  phase: UpdatePhase;
  availableVersion?: string;
  notes?: string;
  progress?: UpdateProgress;
  error?: string;
}

export type UpdateEvent =
  | { type: "check-started" }
  | { type: "check-none" }
  | { type: "check-found"; version: string; notes?: string }
  | { type: "install-started" }
  | { type: "install-progress"; downloadedBytes: number; totalBytes: number }
  | { type: "install-finished" }
  | { type: "failed"; message: string }
  | { type: "reset" };

export const initialUpdateState: UpdateState = { phase: "idle" };

export function updateReducer(current: UpdateState, event: UpdateEvent): UpdateState {
  switch (event.type) {
    case "check-started":
      return { phase: "checking" };
    case "check-none":
      return { phase: "up-to-date" };
    case "check-found":
      return {
        phase: "available",
        availableVersion: event.version,
        notes: event.notes?.trim() ? event.notes.trim() : undefined,
      };
    case "install-started":
      return {
        ...current,
        phase: "downloading",
        progress: { downloadedBytes: 0, totalBytes: current.progress?.totalBytes ?? 0 },
        error: undefined,
      };
    case "install-progress": {
      const totalBytes = event.totalBytes > 0 ? event.totalBytes : (current.progress?.totalBytes ?? 0);
      return {
        ...current,
        phase: "downloading",
        progress: {
          downloadedBytes: Math.max(0, event.downloadedBytes),
          totalBytes,
        },
      };
    }
    case "install-finished":
      return { ...current, phase: "ready", progress: undefined };
    case "failed":
      return { phase: "error", error: event.message || "更新失败，请稍后再试。" };
    case "reset":
      return initialUpdateState;
    default:
      return current;
  }
}

export function progressPercent(progress?: UpdateProgress): number | null {
  if (!progress || progress.totalBytes <= 0) return null;
  const ratio = progress.downloadedBytes / progress.totalBytes;
  return Math.max(0, Math.min(100, Math.round(ratio * 100)));
}

export interface UpdateView {
  label: string;
  detail: string;
  action: "check" | "install" | "none";
  busy: boolean;
}

// 文案面向使用者：说明当前状态与下一步动作，不描述实现过程。
export function describeUpdateState(state: UpdateState, options: { desktop: boolean }): UpdateView {
  if (!options.desktop) {
    return {
      label: "当前是浏览器版本",
      detail: "更新检查只在桌面版可用，安装桌面版后可以在设置里检查新版本。",
      action: "none",
      busy: false,
    };
  }
  switch (state.phase) {
    case "idle":
      return {
        label: "可以检查新版本",
        detail: "检查后会告诉你是否有新版本，以及更新包含哪些改动。",
        action: "check",
        busy: false,
      };
    case "checking":
      return {
        label: "正在检查新版本",
        detail: "正在获取最新版本信息，请稍候。",
        action: "none",
        busy: true,
      };
    case "up-to-date":
      return {
        label: "已经是最新版本",
        detail: "当前版本不需要更新，有新版本时可以在这里下载。",
        action: "check",
        busy: false,
      };
    case "available": {
      const version = state.availableVersion ? ` ${state.availableVersion}` : "";
      return {
        label: `发现新版本${version}`,
        detail: state.notes ?? "下载完成后重启应用即可完成更新。",
        action: "install",
        busy: false,
      };
    }
    case "downloading": {
      const percent = progressPercent(state.progress);
      return {
        label: percent == null ? "正在下载更新" : `正在下载更新（${percent}%）`,
        detail: "下载期间可以继续写作，完成后会提示你重启应用。",
        action: "none",
        busy: true,
      };
    }
    case "ready":
      return {
        label: "更新已下载",
        detail: "关闭并重新打开应用即可完成更新。",
        action: "none",
        busy: false,
      };
    case "error":
      return {
        label: "更新没有完成",
        detail: state.error ?? "请稍后再试，或到发布页面手动下载新版本。",
        action: "check",
        busy: false,
      };
    default:
      return {
        label: "可以检查新版本",
        detail: "检查后会告诉你是否有新版本。",
        action: "check",
        busy: false,
      };
  }
}
