import { startServer, type StartedServer } from "../../app";

// The supervising desktop shell keeps stdin piped open for the lifetime of this process.
// End-of-input means the shell is gone, so the server stops instead of outliving it.
const FORCED_EXIT_DELAY_MS = 5_000;

let runningServer: StartedServer | null = null;
let stopRequested = false;

function closeRunningServer(server: StartedServer): void {
  setTimeout(() => process.exit(0), FORCED_EXIT_DELAY_MS).unref();
  const closing = server.close();
  server.server.closeAllConnections();
  void closing
    .catch((error) => {
      console.warn("[desktop-sidecar] graceful stop failed.", error);
    })
    .finally(() => process.exit(0));
}

function requestStop(reason: string): void {
  if (stopRequested) {
    return;
  }
  stopRequested = true;
  console.log(`[desktop-sidecar] stopping: ${reason}.`);
  if (runningServer) {
    closeRunningServer(runningServer);
  }
}

process.stdin.on("end", () => requestStop("desktop shell closed the control pipe"));
process.stdin.on("error", () => requestStop("desktop shell control pipe failed"));
process.stdin.resume();
process.on("SIGTERM", () => requestStop("SIGTERM"));
process.on("SIGINT", () => requestStop("SIGINT"));

startServer()
  .then((server) => {
    runningServer = server;
    console.log(`[desktop-sidecar] ready at ${server.url}.`);
    if (stopRequested) {
      closeRunningServer(server);
    }
  })
  .catch((error) => {
    console.error("[desktop-sidecar] server failed to start.", error);
    process.exit(1);
  });
