import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native index crash restart", () => {
  it("takes over the expired claim with a new epoch and serves only the latest revision", async () => {
    const state = JSON.parse(fs.readFileSync(`${dataDir}/index-crash-claim.json`, "utf8"));
    let completed = null;
    const deadline = Date.now() + 120_000;
    while (Date.now() < deadline) {
      const jobs = await invoke("list_index_jobs", { novelId: state.novelId });
      const finished = jobs.find((job) => job.id === state.claimedJobId);
      if (finished && finished.status === "succeeded" && finished.claimEpoch > state.claimedEpoch) {
        completed = finished;
        break;
      }
      await browser.pause(500);
    }
    expect(completed).not.toBeNull();
    expect(completed.claimEpoch).toBeGreaterThan(state.claimedEpoch);
    expect(completed.sourceRevision).toBe("v2");

    const document = await invoke("get_knowledge_document", { documentId: state.documentId });
    expect(document.sourceRevision).toBe("v2");

    const fresh = await invoke("search_knowledge", { novelId: state.novelId, query: "图谱", limit: 5 });
    expect(fresh.some((hit) => hit.body.includes("新稿"))).toBe(true);
    const stale = await invoke("search_knowledge", { novelId: state.novelId, query: "剑谱", limit: 5 });
    expect(stale.some((hit) => hit.body.includes("旧稿"))).toBe(false);

    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, state.novelId);
    const panel = await $("[data-testid=index-health-panel]");
    await panel.waitForDisplayed({ timeout: 20_000 });
    await browser.waitUntil(async () => (await panel.getText()).includes("succeeded"), {
      timeout: 20_000,
      timeoutMsg: "native index panel did not show the finished job",
    });
  });
});
