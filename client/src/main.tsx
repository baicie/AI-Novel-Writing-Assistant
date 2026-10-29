import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, HashRouter } from "react-router-dom";
import "highlight.js/styles/github.css";
import DesktopBootstrapBoundary from "./components/layout/DesktopBootstrapBoundary";
import AccessGate from "./components/layout/AccessGate";
import ServerStartupGate from "./components/layout/ServerStartupGate";
import { APP_RUNTIME } from "./lib/constants";
import AppRouter from "./router";
import { Toaster } from "./components/ui/toast";
import "./index.css";
import { ThemeProvider } from "./components/theme/ThemeProvider";
import {
  createChapter,
  createNovelWithSetup,
  getChapter,
  getDesktopShellBootstrap,
  hasDesktopShell,
  reportClientRuntime,
  saveChapter,
  smokeChannelMessage,
} from "./platform/desktopTransport";

// The WDIO guest plugin is only compiled into the dedicated E2E frontend build.
if ((import.meta.env as Record<string, string | undefined>).VITE_TAURI_E2E === "1") {
  await import("@wdio/tauri-plugin");
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

const AppRouterProvider = APP_RUNTIME === "desktop" ? HashRouter : BrowserRouter;

const startRoute = (window as Window & { __AI_NOVEL_START_ROUTE__?: unknown }).__AI_NOVEL_START_ROUTE__;
if (typeof startRoute === "string" && startRoute.startsWith("/") && window.location.hash !== `#${startRoute}`) {
  window.location.hash = `#${startRoute}`;
}

if (hasDesktopShell()) {
  const runtimeConfig = (window as Window & { __AI_NOVEL_RUNTIME__?: unknown }).__AI_NOVEL_RUNTIME__;
  void reportClientRuntime({
    config: runtimeConfig ?? null,
    locationHash: window.location.hash,
    runtimeMode: APP_RUNTIME,
  }).catch(() => undefined);
  const reportError = (message: string) => {
    void reportClientRuntime({
      config: runtimeConfig ?? null,
      locationHash: window.location.hash,
      runtimeMode: APP_RUNTIME,
      message,
    }).catch(() => undefined);
  };
  window.addEventListener("error", (event) => {
    reportError(`${event.message} @ ${event.filename}:${event.lineno}:${event.colno}`);
  });
  window.addEventListener("unhandledrejection", (event) => {
    reportError(`unhandled rejection: ${String(event.reason)}`);
  });
  void getDesktopShellBootstrap()
    .then((snapshot) => console.info("[desktop] local kernel ready", snapshot.stage))
    .catch((error: unknown) => reportError(`bootstrap failed: ${String(error)}`));

  if ((window as Window & { __AI_NOVEL_SMOKE_SAVE__?: unknown }).__AI_NOVEL_SMOKE_SAVE__ === true) {
    void (async () => {
      const novel = await createNovelWithSetup({ title: "桌面保存烟测", description: "" });
      const chapter = await createChapter({ novelId: novel.id, title: "第一章", sortOrder: 1 });
      const saved = await saveChapter({
        novelId: novel.id,
        chapterId: chapter.id,
        expectedVersion: 0,
        title: chapter.title,
        content: "桌面保存烟测正文。",
      });
      const reopened = await getChapter({ novelId: novel.id, chapterId: chapter.id });
      reportClientRuntime({
        config: runtimeConfig ?? null,
        locationHash: window.location.hash,
        runtimeMode: APP_RUNTIME,
        message: `save smoke ok version=${saved.officialVersion} chars=${reopened.content.length}`,
      }).catch(() => undefined);
    })().catch((error: unknown) => reportError(`save smoke failed: ${String(error)}`));
  }

  if ((window as Window & { __AI_NOVEL_SMOKE_CHANNEL__?: unknown }).__AI_NOVEL_SMOKE_CHANNEL__ === true) {
    void smokeChannelMessage((event) => {
      reportClientRuntime({
        config: runtimeConfig ?? null,
        locationHash: window.location.hash,
        runtimeMode: APP_RUNTIME,
        message: `channel smoke ok kind=${event.kind} seq=${"seq" in event ? event.seq : "n/a"}`,
      }).catch(() => undefined);
    }).catch((error: unknown) => reportError(`channel smoke failed: ${String(error)}`));
  }

  if ((window as Window & { __AI_NOVEL_SMOKE_DIRECTOR__?: unknown }).__AI_NOVEL_SMOKE_DIRECTOR__ === true) {
    void (async () => {
      const novel = await createNovelWithSetup({ title: "桌面导演烟测", description: "" });
      await createChapter({ novelId: novel.id, title: "第一章", sortOrder: 1 });
      window.location.hash = `#/lab/director/${encodeURIComponent(novel.id)}`;
      await new Promise(resolve => setTimeout(resolve, 2500));
      const page = document.querySelector("[data-tauri-director-page]");
      const start = document.querySelector("[data-tauri-director-start]");
      reportClientRuntime({
        config: runtimeConfig ?? null,
        locationHash: window.location.hash,
        runtimeMode: APP_RUNTIME,
        message: `director ui smoke ok page=${Boolean(page)} start=${Boolean(start)}`,
      }).catch(() => undefined);
    })().catch((error: unknown) => reportError(`director ui smoke failed: ${String(error)}`));
  }

  if ((window as Window & { __AI_NOVEL_SMOKE_ADAPTER__?: unknown }).__AI_NOVEL_SMOKE_ADAPTER__ === true) {
    void import("./api/client").then(async ({ apiClient }) => {
      const created = await apiClient.post<{ data: { id: string } }>("/novels", {
        title: "适配器烟测",
        description: "",
      });
      const novelId = created.data.data.id;
      const chapter = await apiClient.post<{ data: { id: string; title: string } }>(
        `/novels/${novelId}/chapters`,
        { title: "第一章", order: 1 },
      );
      const chapterId = chapter.data.data.id;
      const saved = await apiClient.put<{ data: { officialVersion: number; content: string } }>(
        `/novels/${novelId}/chapters/${chapterId}`,
        { expectedVersion: 0, title: "第一章", content: "适配器保存正文。" },
      );
      const reopened = await apiClient.get<{ data: { content: string } }>(
        `/novels/${novelId}/chapters/${chapterId}`,
      );
      const updated = await apiClient.put<{ data: { description: string; postGenerationStyleReviewEnabled: boolean } }>(
        `/novels/${novelId}`,
        { description: "适配器更新后的简介", postGenerationStyleReviewEnabled: true },
      );
      const exported = await apiClient.get<Blob>(`/novels/${novelId}/export`, {
        params: { format: "txt", scope: "full" },
        responseType: "blob",
      });
      const exportText = await exported.data.text();
      reportClientRuntime({
        config: runtimeConfig ?? null,
        locationHash: window.location.hash,
        runtimeMode: APP_RUNTIME,
        message: `adapter smoke ok version=${saved.data.data.officialVersion} chars=${reopened.data.data.content.length} updated=${updated.data.data.description}/${updated.data.data.postGenerationStyleReviewEnabled} export=${exportText.length}`,
      }).catch(() => undefined);
    }).catch((error: unknown) => reportError(`adapter smoke failed: ${String(error)}`));
  }
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>
      <QueryClientProvider client={queryClient}>
        <AppRouterProvider>
          <DesktopBootstrapBoundary>
            <ServerStartupGate>
              <AccessGate>
                <AppRouter />
              </AccessGate>
            </ServerStartupGate>
          </DesktopBootstrapBoundary>
          <Toaster />
        </AppRouterProvider>
      </QueryClientProvider>
    </ThemeProvider>
  </React.StrictMode>,
);
