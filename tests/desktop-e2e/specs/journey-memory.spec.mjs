import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Shared fixture journey: structured memory", () => {
  it("keeps alias, location, life state and relationship timeline straight on one novel", async function () {
    this.timeout(180_000);
    const fixture = JSON.parse(fs.readFileSync(`${dataDir}/journey-fixture.json`, "utf8"));
    const [hero, ally] = fixture.characterIds;
    const chapters = fixture.chapterIds;
    const fact = (extra) => ({
      id: crypto.randomUUID(),
      novelId: fixture.novelId,
      nowMs: Date.now(),
      sourceRevision: 1,
      ...extra,
    });

    await invoke("confirm_structured_fact", { fact: fact({ entityId: hero, factType: "alias", valueJson: JSON.stringify({ alias: "林公子" }), sourceChapterId: chapters[0], effectiveChapterOrder: 1, evidenceQuote: "同伴称他林公子。" }) });
    await invoke("confirm_structured_fact", { fact: fact({ entityId: hero, factType: "life_state", valueJson: JSON.stringify({ state: "alive" }), sourceChapterId: chapters[1], effectiveChapterOrder: 2, evidenceQuote: "林动仍在山中行走。" }) });
    await invoke("confirm_structured_fact", { fact: fact({ entityId: hero, factType: "relationship", valueJson: JSON.stringify({ relation: "ally_of", targetEntityId: ally, targetName: "苏晚" }), sourceChapterId: chapters[1], effectiveChapterOrder: 2, evidenceQuote: "两人结盟。" }) });
    await invoke("confirm_structured_fact", { fact: fact({ entityId: hero, factType: "location", valueJson: JSON.stringify({ location: "山门" }), sourceChapterId: chapters[0], effectiveChapterOrder: 2, evidenceQuote: "林动踏上山门石阶。" }) });

    const early = await invoke("list_structured_facts", { novelId: fixture.novelId, entityId: hero, asOfChapterOrder: 2 });
    expect(early.some((item) => item.factType === "location" && item.valueJson.includes("山门"))).toBe(true);
    expect(early.some((item) => item.factType === "alias" && item.valueJson.includes("林公子"))).toBe(true);

    await invoke("confirm_structured_fact", { fact: fact({ entityId: hero, factType: "location", valueJson: JSON.stringify({ location: "祖祠" }), sourceChapterId: chapters[4], effectiveChapterOrder: 5, evidenceQuote: "林动改在祖祠守夜。" }) });
    const later = await invoke("list_structured_facts", { novelId: fixture.novelId, entityId: hero });
    const activeLocations = later.filter((item) => item.factType === "location" && item.status === "active");
    expect(activeLocations).toHaveLength(1);
    expect(activeLocations[0].valueJson).toContain("祖祠");

    const asOfThree = await invoke("list_structured_facts", { novelId: fixture.novelId, entityId: hero, asOfChapterOrder: 3 });
    const asOfLocations = asOfThree.filter((item) => item.factType === "location");
    expect(asOfLocations.some((item) => item.valueJson.includes("山门"))).toBe(true);
    expect(asOfLocations.some((item) => item.valueJson.includes("祖祠"))).toBe(false);

    await invoke("confirm_structured_fact", { fact: fact({ entityId: hero, factType: "relationship", valueJson: JSON.stringify({ relation: "ally_of", targetEntityId: ally, active: false }), sourceChapterId: chapters[2], effectiveChapterOrder: 3, evidenceQuote: "两人在雨里决裂。" }) });
    const afterTermination = await invoke("list_structured_facts", { novelId: fixture.novelId, entityId: hero });
    const relationships = afterTermination.filter((item) => item.factType === "relationship");
    expect(relationships.some((item) => item.status === "active" && item.valueJson.includes('"active":false'))).toBe(true);

    await invoke("confirm_structured_fact", { fact: fact({ entityId: hero, factType: "location", valueJson: JSON.stringify({ location: "山门" }), sourceChapterId: chapters[4], effectiveChapterOrder: 5, evidenceQuote: "有人说他回了山门。" }) });
    await invoke("sync_memory_conflicts", { novelId: fixture.novelId });
    const conflicts = await invoke("list_memory_conflicts", { novelId: fixture.novelId });
    const open = conflicts.find((item) => item.status === "open");
    expect(open).toBeTruthy();
    const resolved = await invoke("resolve_memory_conflict", {
      request: {
        conflictId: open.id,
        expectedVersion: open.version,
        commandId: `journey-conflict-${Date.now()}`,
        decision: "keep_existing",
        chosenCandidateId: null,
        reason: "旅程：以祖祠为准。",
        operator: "journey",
      },
    });
    expect(resolved.status).not.toBe("open");

    const final = await invoke("list_structured_facts", { novelId: fixture.novelId, entityId: hero });
    const finalLocations = final.filter((item) => item.factType === "location" && item.status === "active");
    expect(finalLocations).toHaveLength(1);
    expect(finalLocations[0].valueJson).toContain("祖祠");
    expect(finalLocations[0].sourceChapterId).toBe(chapters[4]);
    expect(finalLocations[0].truthfulness).toBe("confirmed");
  });
});
