import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  confirmStructuredFact,
  listChapters,
  listCharacters,
  listStructuredFacts,
  type ChapterSnapshot,
  type CharacterSnapshot,
  type StructuredFactView,
} from "@/platform/desktopTransport";

const factTypeLabel: Record<string, string> = {
  location: "所在地点",
  life_state: "生死状态",
  alias: "别名",
  relationship: "人物关系",
};

function displayValue(fact: StructuredFactView): string {
  try {
    const value = JSON.parse(fact.valueJson) as Record<string, unknown>;
    return String(value.location ?? value.state ?? value.alias ?? value.relationship ?? fact.valueJson);
  } catch {
    return fact.valueJson;
  }
}

export default function StructuredMemoryPanel({ novelId }: { novelId: string }) {
  const [characters, setCharacters] = useState<CharacterSnapshot[]>([]);
  const [chapters, setChapters] = useState<ChapterSnapshot[]>([]);
  const [entityId, setEntityId] = useState("");
  const [facts, setFacts] = useState<StructuredFactView[]>([]);
  const [factType, setFactType] = useState("location");
  const [value, setValue] = useState("");
  const [chapterId, setChapterId] = useState("");
  const [evidence, setEvidence] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refreshFacts = async (id: string) => {
    if (!id) { setFacts([]); return; }
    setFacts(await listStructuredFacts({ novelId, entityId: id }));
  };

  useEffect(() => {
    Promise.all([listCharacters(novelId), listChapters(novelId)]).then(([characterRows, chapterRows]) => {
      setCharacters(characterRows);
      setChapters(chapterRows);
      setEntityId((current) => current || characterRows[0]?.id || "");
      setChapterId((current) => current || chapterRows[0]?.id || "");
    }).catch(() => {});
  }, [novelId]);

  useEffect(() => {
    void refreshFacts(entityId).catch(() => setFacts([]));
  }, [entityId, novelId]);

  const addFact = async () => {
    const chapter = chapters.find((row) => row.id === chapterId);
    if (!entityId || !chapter || !value.trim() || !evidence.trim()) {
      setMessage("请选择人物、来源章节，并填写状态和来源证据。");
      return;
    }
    if (chapter.officialVersion <= 0) {
      setMessage("来源章节还没有正式正文，请先完成并确认章节。");
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      const structuredValue = factType === "life_state" ? { state: value.trim() }
        : factType === "alias" ? { alias: value.trim() }
          : factType === "relationship" ? { relationship: value.trim() }
            : { location: value.trim() };
      await confirmStructuredFact({
        fact: {
          id: crypto.randomUUID(),
          novelId,
          entityId,
          factType,
          valueJson: JSON.stringify(structuredValue),
          sourceChapterId: chapter.id,
          sourceRevision: chapter.officialVersion,
          evidenceQuote: evidence.trim(),
          effectiveChapterOrder: chapter.sortOrder,
          nowMs: Date.now(),
        },
      });
      setValue("");
      setEvidence("");
      await refreshFacts(entityId);
      setMessage("人物状态已确认并写入长篇记忆。");
    } catch (cause: unknown) {
      setMessage(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="space-y-3 rounded-xl bg-muted/15 p-3" data-testid="structured-memory-panel">
      <div>
        <h3 className="text-sm font-semibold">人物长期状态</h3>
        <p className="mt-1 text-xs text-muted-foreground">记录人物所在位置与生死状态，确认后按来源章节参与后续创作上下文。</p>
      </div>
      <div className="flex flex-wrap items-center gap-2 text-xs">
        <select className="rounded-md border border-input bg-background px-2 py-1.5" value={entityId} onChange={(event) => setEntityId(event.target.value)} data-testid="memory-character">
          {characters.map((character) => <option key={character.id} value={character.id}>{character.name}</option>)}
        </select>
        <span className="text-muted-foreground">共 {facts.length} 条已确认事实</span>
      </div>
      <ul className="space-y-1">
        {facts.map((fact) => (
          <li key={fact.id} className="flex flex-wrap items-center gap-2 rounded-lg bg-background/60 px-2 py-1.5 text-xs">
            <span className="font-medium">{factTypeLabel[fact.factType] ?? fact.factType}</span>
            <span>{displayValue(fact)}</span>
            <span className="text-muted-foreground">第{fact.effectiveChapterOrder}章 · 来源 v{fact.sourceRevision} · {fact.status}</span>
            <span className="line-clamp-1 text-muted-foreground">证据：{fact.evidenceQuote}</span>
          </li>
        ))}
        {!facts.length ? <li className="text-xs text-muted-foreground">还没有已确认的人物状态。</li> : null}
      </ul>
      <div className="grid gap-2 sm:grid-cols-2">
        <label className="text-xs">状态类型
          <select className="mt-1 w-full rounded-md border border-input bg-background px-2 py-1.5" value={factType} onChange={(event) => setFactType(event.target.value)} data-testid="memory-type">
            <option value="location">所在地点</option>
            <option value="life_state">生死状态</option>
          </select>
        </label>
        <label className="text-xs">状态值
          <input className="mt-1 w-full rounded-md border border-input bg-background px-2 py-1.5" value={value} placeholder={factType === "life_state" ? "alive / dead / unknown" : "山门"} onChange={(event) => setValue(event.target.value)} data-testid="memory-value" />
        </label>
        <label className="text-xs">来源章节
          <select className="mt-1 w-full rounded-md border border-input bg-background px-2 py-1.5" value={chapterId} onChange={(event) => setChapterId(event.target.value)} data-testid="memory-chapter">
            {chapters.map((chapter) => <option key={chapter.id} value={chapter.id}>第{chapter.sortOrder}章 {chapter.title}（正式 v{chapter.officialVersion}）</option>)}
          </select>
        </label>
        <label className="text-xs">来源证据
          <input className="mt-1 w-full rounded-md border border-input bg-background px-2 py-1.5" value={evidence} onChange={(event) => setEvidence(event.target.value)} data-testid="memory-evidence" />
        </label>
      </div>
      {message ? <p className="text-xs text-muted-foreground" role="status">{message}</p> : null}
      <Button size="sm" onClick={() => void addFact()} disabled={busy} data-testid="memory-confirm">{busy ? "保存中..." : "确认人物状态"}</Button>
    </section>
  );
}
