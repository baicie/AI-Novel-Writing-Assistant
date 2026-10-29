import { useEffect, useState } from "react";
import * as desktopShell from "@/platform/desktopTransport";
import { APP_RUNTIME } from "./constants";

export type DesktopBootstrapState = "launching" | "starting-server" | "loading-ui" | "ready" | "error";
export type DesktopUpdaterStatus =
  | "disabled"
  | "idle"
  | "checking"
  | "update-available"
  | "downloading"
  | "downloaded"
  | "not-available"
  | "error";

export interface DesktopBootstrapSnapshot {
  state: DesktopBootstrapState;
  stage: string;
  title: string;
  detail: string;
  logDir: string;
  logFile: string;
  updatedAt: string;
  canRetry: boolean;
}

export interface DesktopUpdaterSnapshot {
  status: DesktopUpdaterStatus;
  message: string;
  currentVersion: string;
  availableVersion: string | null;
  progressPercent: number | null;
  bytesPerSecond: number | null;
  channel: string;
  isPortable: boolean;
  isPackaged: boolean;
  isSupported: boolean;
  canInstall: boolean;
  updatedAt: string;
  lastCheckedAt: string | null;
}

export interface DesktopDataImportSnapshot {
  currentDatabasePath: string;
  currentDatabaseLikelyFresh: boolean;
  suggestedSourcePath: string | null;
  suggestedSourceLabel: string | null;
  backupDirectory: string;
}

export interface DesktopDataImportResult {
  scheduled: boolean;
  cancelled: boolean;
  sourcePath?: string;
}

const DEFAULT_BOOTSTRAP_SNAPSHOT: DesktopBootstrapSnapshot = {
  state: "launching",
  stage: "launching",
  title: "正在启动桌面工作区",
  detail: "正在准备桌面本地运行时。",
  logDir: "",
  logFile: "",
  updatedAt: "",
  canRetry: false,
};

const DEFAULT_UPDATER_SNAPSHOT: DesktopUpdaterSnapshot = {
  status: "disabled",
  message: "当前运行环境无法使用桌面版更新。",
  currentVersion: "0.0.0",
  availableVersion: null,
  progressPercent: null,
  bytesPerSecond: null,
  channel: "beta",
  isPortable: false,
  isPackaged: false,
  isSupported: false,
  canInstall: false,
  updatedAt: "",
  lastCheckedAt: null,
};

type DesktopBridge = NonNullable<Window["__AI_NOVEL_DESKTOP__"]>;

const SHELL_STARTUP_POLL_MS = 400;
const SHELL_WATCH_POLL_MS = 3_000;

let shellBridge: DesktopBridge | null = null;

async function readShellBootstrapSnapshot(): Promise<DesktopBootstrapSnapshot> {
  const { updatedAtMs, ...snapshot } = await desktopShell.getDesktopShellBootstrap();
  return { ...snapshot, updatedAt: new Date(updatedAtMs).toISOString() };
}

function createShellBridge(): DesktopBridge {
  return {
    getBootstrapSnapshot: readShellBootstrapSnapshot,
    subscribeBootstrapState: (listener) => {
      let stopped = false;
      let timer: number | undefined;
      let lastKey = "";

      const poll = async () => {
        const snapshot = await readShellBootstrapSnapshot().catch(() => null);
        if (stopped) {
          return;
        }
        if (snapshot) {
          const key = `${snapshot.state}|${snapshot.stage}|${snapshot.updatedAt}`;
          if (key !== lastKey) {
            lastKey = key;
            listener(snapshot);
          }
        }
        // The shell keeps reporting after startup so a local server crash brings back the recovery screen.
        timer = window.setTimeout(poll, snapshot?.state === "ready" ? SHELL_WATCH_POLL_MS : SHELL_STARTUP_POLL_MS);
      };

      void poll();
      return () => {
        stopped = true;
        window.clearTimeout(timer);
      };
    },
    openLogsDirectory: () => desktopShell.openDesktopShellLogsDirectory(),
    copyLogPath: async () => {
      const { logFile } = await readShellBootstrapSnapshot();
      await navigator.clipboard?.writeText(logFile).catch(() => undefined);
      return logFile;
    },
    restartApp: () => desktopShell.restartDesktopShell(),
  };
}

function getDesktopBridge(): DesktopBridge | null {
  if (typeof window === "undefined" || APP_RUNTIME !== "desktop") {
    return null;
  }
  if (window.__AI_NOVEL_DESKTOP__) {
    return window.__AI_NOVEL_DESKTOP__;
  }
  if (!desktopShell.hasDesktopShell()) {
    return null;
  }
  shellBridge ??= createShellBridge();
  return shellBridge;
}

export function notifyDesktopRendererReady(): void {
  getDesktopBridge()?.notifyRendererReady?.();
}

export function notifyDesktopAppShellReady(): void {
  getDesktopBridge()?.notifyAppShellReady?.();
}

export async function checkForDesktopUpdates(): Promise<void> {
  await getDesktopBridge()?.checkForUpdates?.();
}

export async function quitAndInstallDesktopUpdate(): Promise<void> {
  await getDesktopBridge()?.quitAndInstall?.();
}

export async function openDesktopLogsDirectory(): Promise<void> {
  await getDesktopBridge()?.openLogsDirectory?.();
}

export async function copyDesktopLogPath(): Promise<string | undefined> {
  return getDesktopBridge()?.copyLogPath?.();
}

export function canBundleDesktopLogs(): boolean {
  return Boolean(getDesktopBridge()?.bundleLogs);
}

export async function bundleDesktopLogs(): Promise<string | null> {
  return getDesktopBridge()?.bundleLogs?.() ?? null;
}

export async function restartDesktopApp(): Promise<void> {
  await getDesktopBridge()?.restartApp?.();
}

export async function getDesktopDataImportSnapshot(): Promise<DesktopDataImportSnapshot | null> {
  return getDesktopBridge()?.getDataImportSnapshot?.() ?? null;
}

export async function importDesktopLegacyDatabase(options?: { preferSuggested?: boolean }): Promise<DesktopDataImportResult | null> {
  return getDesktopBridge()?.importLegacyDatabase?.(options) ?? null;
}

export function useDesktopBootstrap(): DesktopBootstrapSnapshot {
  const [snapshot, setSnapshot] = useState<DesktopBootstrapSnapshot>(DEFAULT_BOOTSTRAP_SNAPSHOT);

  useEffect(() => {
    if (typeof window === "undefined") {
      return undefined;
    }
    const runtimeGlobal = (window as Window & { __AI_NOVEL_RUNTIME__?: unknown }).__AI_NOVEL_RUNTIME__;
    const isPackaged = (runtimeGlobal && typeof runtimeGlobal === "object" && "isPackaged" in runtimeGlobal)
      ? Boolean((runtimeGlobal as { isPackaged?: unknown }).isPackaged)
      : false;
    // dev 模式下后端是 KernelIntegrated：axum server 在同一个 Tauri 进程里，
    // bootstrap 状态在 setup 时已经 publish Ready，前端不必再调 Tauri IPC 拉一遍。
    // 这里直接 assume ready 避免 Tauri 2 dev 模式下 WebView 注入 IPC channel 不稳定导致卡在启动页。
    if (!isPackaged) {
      setSnapshot({
        state: "ready",
        stage: "server-healthy",
        title: "创作工作台已就绪",
        detail: "本地创作服务运行正常。",
        logDir: "",
        logFile: "",
        updatedAt: new Date().toISOString(),
        canRetry: true,
      });
      return undefined;
    }
    const bridge = getDesktopBridge();
    if (!bridge?.getBootstrapSnapshot) {
      console.warn("[desktop-bridge] getDesktopBridge returned null, 启动页会一直 hold");
      return undefined;
    }

    let cancelled = false;

    void bridge.getBootstrapSnapshot().then((nextSnapshot) => {
      if (!cancelled && nextSnapshot) {
        setSnapshot(nextSnapshot);
      }
    }).catch((error: unknown) => {
      console.error("[desktop-bridge] getBootstrapSnapshot failed", error);
    });

    const unsubscribe = bridge.subscribeBootstrapState?.((nextSnapshot) => {
      if (!cancelled && nextSnapshot) {
        setSnapshot(nextSnapshot);
      }
    });

    return () => {
      cancelled = true;
      if (typeof unsubscribe === "function") {
        unsubscribe();
      }
    };
  }, []);

  return snapshot;
}

export function useDesktopUpdater(): DesktopUpdaterSnapshot {
  const [snapshot, setSnapshot] = useState<DesktopUpdaterSnapshot>(DEFAULT_UPDATER_SNAPSHOT);

  useEffect(() => {
    const bridge = getDesktopBridge();
    if (!bridge?.getUpdaterSnapshot) {
      return undefined;
    }

    let cancelled = false;

    void bridge.getUpdaterSnapshot().then((nextSnapshot) => {
      if (!cancelled && nextSnapshot) {
        setSnapshot(nextSnapshot);
      }
    });

    const unsubscribe = bridge.subscribeUpdaterStatus?.((nextSnapshot) => {
      if (!cancelled && nextSnapshot) {
        setSnapshot(nextSnapshot);
      }
    });

    return () => {
      cancelled = true;
      if (typeof unsubscribe === "function") {
        unsubscribe();
      }
    };
  }, []);

  return snapshot;
}
