import fs from "node:fs";
import { execSync } from "node:child_process";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const binary = process.env.BIZ_NOVEL_E2E_BINARY;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native index crash claim", () => {
  it("persists the new claim epoch and then dies with SIGKILL", async () => {
    const state = JSON.parse(fs.readFileSync(`${dataDir}/index-crash-state.json`, "utf8"));
    await browser.execute((args) => {
      window.__indexRetryPending = window.__TAURI__.core.invoke("retry_knowledge_document_with_job", args);
      return null;
    }, { novelId: state.novelId, documentId: state.documentId, sourceRevision: "v2", indexVersion: 2 });

    let claimed = null;
    const deadline = Date.now() + 60_000;
    while (Date.now() < deadline) {
      const jobs = await invoke("list_index_jobs", { novelId: state.novelId });
      const running = jobs.find((job) => job.sourceRevision === "v2" && job.status === "running");
      if (running && running.claimEpoch > state.firstEpoch) {
        claimed = running;
        break;
      }
      await browser.pause(300);
    }
    expect(claimed).not.toBeNull();
    expect(claimed.claimEpoch).toBe(state.firstEpoch + 1);

    fs.writeFileSync(`${dataDir}/index-crash-claim.json`, JSON.stringify({
      ...state,
      claimedJobId: claimed.id,
      claimedEpoch: claimed.claimEpoch,
    }));

    const pids = execSync(`pgrep -f ${JSON.stringify(binary)}`, { encoding: "utf8" })
      .trim().split(/\s+/).filter(Boolean);
    if (!pids.length) throw new Error("test Tauri app pid not found");
    for (const pid of pids) process.kill(Number(pid), "SIGKILL");
    try { await browser.pause(1000); } catch { /* expected SIGKILL */ }
  });
});
