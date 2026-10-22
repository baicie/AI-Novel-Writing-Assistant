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
import { getDesktopShellBootstrap, hasDesktopShell, reportClientRuntime } from "./platform/desktopTransport";

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
