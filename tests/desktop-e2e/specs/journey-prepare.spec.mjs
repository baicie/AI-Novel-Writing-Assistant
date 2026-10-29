import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Shared fixture journey: prepare", () => {
  it("creates one novel with a world, a cast, two volumes and ten planned chapters", async () => {
    const novel = await invoke("create_novel", { title: "旅程基准书", description: "P15.8 共享 Fixture 作者旅程" });
    const world = await invoke("create_world", {
      name: "山门外门",
      description: "青石阶、夜雨与长明灯。",
      overviewSummary: "宗门以剑为尊，祖祠封着旧事。",
    });
    const characters = [];
    for (const [name, role] of [["林动", "主角"], ["苏晚", "同伴"], ["陆沉", "对立者"]]) {
      characters.push(await invoke("create_character", {
        request: { id: `journey-char-${name}`, novelId: novel.id, name, role },
      }));
    }
    const volumes = [];
    for (let index = 1; index <= 2; index += 1) {
      volumes.push(await invoke("create_volume", {
        novelId: novel.id,
        title: index === 1 ? "山门卷" : "祖祠卷",
        sortOrder: index,
        summary: index === 1 ? "入门与试探" : "真相与代价",
      }));
    }
    const chapters = [];
    for (let index = 1; index <= 10; index += 1) {
      chapters.push(await invoke("create_chapter", {
        novelId: novel.id,
        title: `第${index}章 山门夜行`,
        sortOrder: index,
      }));
    }
    const fixture = {
      novelId: novel.id,
      worldId: world.id,
      characterIds: characters.map((item) => item.id),
      characterNames: characters.map((item) => item.name),
      volumeIds: volumes.map((item) => item.id),
      chapterIds: chapters.map((item) => item.id),
    };
    fs.writeFileSync(`${dataDir}/journey-fixture.json`, JSON.stringify(fixture, null, 2));
    expect(chapters).toHaveLength(10);
    expect(volumes).toHaveLength(2);
  });
});
