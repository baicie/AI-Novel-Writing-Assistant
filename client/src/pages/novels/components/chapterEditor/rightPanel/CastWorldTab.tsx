import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { queryKeys } from "@/api/queryKeys";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import {
  confirmMemoryCandidate,
  confirmStructuredFact,
  deferMemoryCandidate,
  getChapterContinuity,
  rejectMemoryCandidate,
  resolveMemoryConflict,
  type ContinuityCandidate,
  type ContinuityConflict,
  type ContinuityFact,
  type ContinuityWorld,
} from "@/platform/desktopTransport";
import { canReconfirm, factValueText, sourceLabel } from "./continuityModel";

interface CastWorldTabProps {
  novelId: string;
  chapterId: string;
  /** 跳转到事实来源章节；由 Shell 负责未保存正文的确认。 */
  onOpenChapter?: (chapterId: string) => void;
}

const FACT_TYPE_LABELS: Record<string, string> = {
  location: "地点",
  life_state: "生死",
  relationship: "关系",
  alias: "身份",
  timeline_event: "事件",
  plot_thread: "伏笔",
  character_state: "人物状态",
};

const CANDIDATE_TYPE_LABELS: Record<string, string> = {
  character_state: "人物状态",
  timeline_event: "时间线事件",
  plot_thread: "伏笔",
};

function newCommandId(prefix: string): string {
  return `${prefix}-${crypto.randomUUID()}`;
}

/**
 * 「人物与世界观」：把截至本章的已确认事实、待处理候选与冲突放在同一个可操作面板里。
 * 所有写操作都走既有记忆命令与版本约束；这里不直接改数据库，也不把候选当正式事实。
 */
export default function CastWorldTab(props: CastWorldTabProps) {
  const { novelId, chapterId, onOpenChapter } = props;
  const queryClient = useQueryClient();
  const [expandedConflictId, setExpandedConflictId] = useState<string | null>(null);
  const [conflictReason, setConflictReason] = useState("作者按正文证据裁决");

  const continuityQuery = useQuery({
    queryKey: queryKeys.novels.chapterContinuity(novelId, chapterId),
    queryFn: () => getChapterContinuity({ novelId, chapterId }),
    enabled: Boolean(novelId && chapterId),
  });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.novels.chapterContinuity(novelId, chapterId) });
    void queryClient.invalidateQueries({ queryKey: queryKeys.novels.chapterEditorWorkspace(novelId, chapterId) });
    void queryClient.invalidateQueries({ queryKey: queryKeys.novels.chapterContextPreview(novelId, chapterId) });
  };

  const candidateMutation = useMutation({
    mutationFn: async (input: { action: "confirm" | "reject" | "defer"; candidate: ContinuityCandidate }) => {
      if (input.action === "confirm") {
        return confirmMemoryCandidate({ candidateId: input.candidate.id, expectedVersion: input.candidate.version });
      }
      if (input.action === "defer") {
        return deferMemoryCandidate({ candidateId: input.candidate.id, expectedVersion: input.candidate.version });
      }
      return rejectMemoryCandidate({
        candidateId: input.candidate.id,
        expectedVersion: input.candidate.version,
        reason: "作者判断这条候选与正文证据不符",
      });
    },
    onSuccess: (_result, input) => {
      toast.success(input.action === "confirm" ? "已确认这条事实。" : input.action === "defer" ? "已延后处理。" : "已拒绝这条候选。");
      invalidate();
    },
    onError: (error: unknown) => toast.error(error instanceof Error ? error.message : "候选处理没有完成。"),
  });

  const conflictMutation = useMutation({
    mutationFn: async (input: {
      conflict: ContinuityConflict;
      decision: string;
      chosenCandidateId?: string | null;
    }) =>
      resolveMemoryConflict({
        conflictId: input.conflict.id,
        expectedVersion: input.conflict.version,
        commandId: newCommandId("chapter-continuity-conflict"),
        decision: input.decision,
        chosenCandidateId: input.chosenCandidateId ?? null,
        reason: conflictReason.trim() || "作者按正文证据裁决",
        operator: "author",
      }),
    onSuccess: () => {
      toast.success("冲突已裁决。");
      invalidate();
    },
    onError: (error: unknown) => toast.error(error instanceof Error ? error.message : "冲突裁决没有完成。"),
  });

  const reconfirmMutation = useMutation({
    mutationFn: (fact: ContinuityFact) => {
      if (!fact.entityId || !fact.valueJson || !fact.sourceChapterId || fact.sourceCurrentVersion == null) {
        throw new Error("这条事实缺少可重新确认的来源信息。");
      }
      return confirmStructuredFact({
        fact: {
          id: `reconfirm-${crypto.randomUUID()}`,
          novelId,
          entityId: fact.entityId,
          factType: fact.factType,
          valueJson: fact.valueJson,
          sourceChapterId: fact.sourceChapterId,
          sourceRevision: fact.sourceCurrentVersion,
          evidenceQuote: fact.evidenceQuote?.trim() || "作者在人物与世界观面板重新确认",
          effectiveChapterOrder: fact.effectiveChapterOrder,
          nowMs: Date.now(),
        },
      });
    },
    onSuccess: () => {
      toast.success("已按当前章节版本重新确认这条事实。");
      invalidate();
    },
    onError: (error: unknown) => toast.error(error instanceof Error ? error.message : "重新确认没有完成。"),
  });

  if (continuityQuery.isLoading) {
    return <div className="px-1 py-4 text-sm text-muted-foreground" data-testid="continuity-loading">正在整理截至本章的故事事实...</div>;
  }
  if (continuityQuery.isError) {
    return (
      <div className="space-y-2 px-1 py-4 text-sm text-muted-foreground" data-testid="continuity-error">
        <div>故事事实读取失败，不能当成「没有资料」继续。</div>
        <Button size="sm" variant="outline" onClick={() => void continuityQuery.refetch()}>重试</Button>
      </div>
    );
  }
  const projection = continuityQuery.data;
  if (!projection) {
    return <div className="px-1 py-4 text-sm text-muted-foreground">暂无可展示的故事事实。</div>;
  }

  const keyFacts = projection.facts.filter(
    (fact) => fact.kind === "structured" || fact.factType === "timeline_event" || fact.factType === "plot_thread",
  );
  const visibleFacts = keyFacts.slice(0, 80);
  const hiddenFactCount = keyFacts.length - visibleFacts.length;

  return (
    <div className="flex flex-col gap-4 px-1 py-2" data-testid="cast-world-tab">
      <div className="text-xs text-muted-foreground" data-testid="continuity-scope">
        截至第 {projection.chapterOrder} 章（正式版本 {projection.chapterVersion}）· 已确认事实 {projection.facts.length} 条
        {projection.staleFacts.length > 0 ? ` · 待重新确认 ${projection.staleFacts.length} 条` : ""}
        {projection.candidates.length > 0 ? ` · 待处理候选 ${projection.candidates.length} 条` : ""}
      </div>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-foreground">世界设定</h3>
        {projection.worlds.length === 0 ? (
          <div className="text-xs text-muted-foreground">这本书还没有绑定世界设定。</div>
        ) : (
          <WorldCard world={projection.worlds[0]!} />
        )}
        {projection.worlds.slice(1).map((world) => (
          <WorldCard key={world.id} world={world} secondary />
        ))}
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-foreground">人物状态</h3>
        {projection.characters.length === 0 ? (
          <div className="text-xs text-muted-foreground">还没有人物资料。</div>
        ) : (
          <ul className="space-y-2">
            {projection.characters.map((character) => (
              <li key={character.id} className="rounded-xl bg-muted/40 px-3 py-2" data-testid="continuity-character">
                <div className="flex items-center justify-between gap-2">
                  <div className="text-sm text-foreground">
                    {character.name}
                    <span className="ml-2 text-xs text-muted-foreground">{character.role}</span>
                  </div>
                  {character.pendingCandidateCount > 0 ? (
                    <span className="rounded-full bg-amber-100 px-2 py-0.5 text-[11px] text-amber-900">
                      待确认 {character.pendingCandidateCount}
                    </span>
                  ) : null}
                </div>
                {character.currentState ? (
                  <div className="mt-1 space-y-1" data-testid="continuity-character-state">
                    <div className="text-xs leading-5 text-foreground">{character.currentState.body}</div>
                    <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
                      <SourceLink
                        chapterId={character.currentState.sourceChapterId}
                        label={
                          character.currentState.chapterOrder == null
                            ? "来源不可用"
                            : `第${character.currentState.chapterOrder}章`
                        }
                        onOpenChapter={onOpenChapter}
                      />
                      {character.currentState.evidenceQuote ? (
                        <span className="truncate" data-testid="continuity-evidence">证据：{character.currentState.evidenceQuote}</span>
                      ) : null}
                    </div>
                  </div>
                ) : (
                  <div className="mt-1 text-xs text-muted-foreground">还没有已确认的人物状态。</div>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-foreground">关键事实</h3>
        {visibleFacts.length === 0 ? (
          <div className="text-xs text-muted-foreground">截至本章还没有已确认的关键事实。</div>
        ) : (
          <ul className="space-y-2">
            {visibleFacts.map((fact) => (
              <li key={fact.id} className="rounded-xl bg-muted/30 px-3 py-2" data-testid="continuity-fact">
                <div className="flex flex-wrap items-center gap-2 text-xs">
                  <span className="rounded-full bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
                    {FACT_TYPE_LABELS[fact.factType] ?? fact.factType}
                  </span>
                  {fact.entityName ? <span className="text-foreground">{fact.entityName}</span> : null}
                  <SourceLink chapterId={fact.sourceChapterId} label={sourceLabel(fact)} onOpenChapter={onOpenChapter} />
                </div>
                <div className="mt-1 text-xs leading-5 text-foreground">{factValueText(fact)}</div>
                {fact.evidenceQuote ? (
                  <div className="mt-1 text-[11px] leading-5 text-muted-foreground">证据：{fact.evidenceQuote}</div>
                ) : null}
              </li>
            ))}
          </ul>
        )}
        {hiddenFactCount > 0 ? (
          <div className="text-[11px] text-muted-foreground">还有 {hiddenFactCount} 条事实未展开；先处理上面的条目以保持列表可读。</div>
        ) : null}
      </section>

      {projection.staleFacts.length > 0 ? (
        <section className="space-y-2">
          <h3 className="text-sm font-medium text-amber-900">需要重新确认</h3>
          {projection.staleFacts.map((fact) => (
            <div key={fact.id} className="rounded-xl bg-amber-50 px-3 py-2" data-testid="continuity-stale">
              <div className="text-xs text-amber-950">
                {FACT_TYPE_LABELS[fact.factType] ?? fact.factType}
                {fact.entityName ? ` · ${fact.entityName}` : ""} · {sourceLabel(fact)}
              </div>
              <div className="mt-1 text-xs leading-5 text-amber-950">{factValueText(fact)}</div>
              <div className="mt-1 text-[11px] text-amber-800">
                来源章节已是版本 {fact.sourceCurrentVersion ?? "未知"}，原事实来自版本 {fact.sourceRevision ?? "未知"}，不能继续作为有效证据。
              </div>
              <Button
                size="sm"
                variant="outline"
                className="mt-2"
                data-testid="continuity-reconfirm"
                disabled={reconfirmMutation.isPending || !canReconfirm(fact)}
                onClick={() => reconfirmMutation.mutate(fact)}
              >
                按当前版本重新确认
              </Button>
            </div>
          ))}
        </section>
      ) : null}

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-foreground">待处理候选</h3>
        {projection.candidates.length === 0 ? (
          <div className="text-xs text-muted-foreground">截至本章没有待处理的事实候选。</div>
        ) : (
          projection.candidates.slice(0, 20).map((candidate) => (
            <div key={candidate.id} className="rounded-xl bg-muted/30 px-3 py-2" data-testid="continuity-candidate">
              <div className="flex flex-wrap items-center gap-2 text-xs">
                <span className="rounded-full bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
                  {CANDIDATE_TYPE_LABELS[candidate.factType] ?? candidate.factType}
                </span>
                {candidate.entityName ? <span className="text-foreground">{candidate.entityName}</span> : null}
                <span className="text-[11px] text-muted-foreground">
                  {candidate.chapterOrder == null ? "来源不可用" : `第${candidate.chapterOrder}章`}
                  {candidate.expired ? " · 来源已更新" : ""}
                  {candidate.ambiguous ? " · 人物身份待指定" : ""}
                </span>
              </div>
              <div className="mt-1 text-xs leading-5 text-foreground">{candidate.value}</div>
              <div className="mt-1 text-[11px] leading-5 text-muted-foreground">证据：{candidate.evidenceQuote}</div>
              <div className="mt-2 flex flex-wrap gap-2">
                <Button
                  size="sm"
                  data-testid="continuity-candidate-confirm"
                  disabled={!candidate.canConfirm || candidateMutation.isPending}
                  onClick={() => candidateMutation.mutate({ action: "confirm", candidate })}
                >
                  确认事实
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  data-testid="continuity-candidate-defer"
                  disabled={candidate.status !== "proposed" || candidateMutation.isPending}
                  onClick={() => candidateMutation.mutate({ action: "defer", candidate })}
                >
                  稍后处理
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  data-testid="continuity-candidate-reject"
                  disabled={candidateMutation.isPending}
                  onClick={() => candidateMutation.mutate({ action: "reject", candidate })}
                >
                  拒绝
                </Button>
              </div>
              {candidate.expired ? (
                <div className="mt-3 text-[11px] text-amber-800">
                  来源章节已经产生新版本，这条候选不能直接确认；请重新抽取或按新正文确认。
                </div>
              ) : null}
            </div>
          ))
        )}
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-foreground">事实冲突</h3>
        {projection.conflicts.length > 0 ? (
          <label className="block text-[11px] text-muted-foreground">
            裁决原因（会写入冲突记录）
            <input
              className="mt-1 w-full rounded-lg border border-border bg-background px-2 py-1 text-xs text-foreground"
              data-testid="continuity-conflict-reason"
              value={conflictReason}
              onChange={(event) => setConflictReason(event.target.value)}
            />
          </label>
        ) : null}
        {projection.conflicts.length === 0 ? (
          <div className="text-xs text-muted-foreground">没有待裁决的事实冲突。</div>
        ) : (
          projection.conflicts.map((conflict) => (
            <div key={conflict.id} className="rounded-xl bg-rose-50 px-3 py-2" data-testid="continuity-conflict">
              <div className="text-xs font-medium text-rose-950">{conflict.description}</div>
              <div className="mt-1 text-[11px] text-rose-800">
                第 {conflict.chapterOrder} 章 · 状态 {conflict.status}
              </div>
              <ul className="mt-2 space-y-1">
                {conflict.sides.map((side) => (
                  <li key={`${side.kind}-${side.id}`} className="rounded-lg bg-background/70 px-2 py-1" data-testid="continuity-conflict-side">
                    <div className="text-[11px] text-muted-foreground">
                      {side.kind === "candidate" ? "候选" : "事实"} · {side.status}
                      {side.sourceChapterOrder != null ? ` · 第${side.sourceChapterOrder}章` : ""}
                    </div>
                    <div className="text-xs leading-5 text-foreground">{side.value ?? "来源不可用"}</div>
                    {side.evidenceQuote ? (
                      <div className="text-[11px] leading-5 text-muted-foreground">证据：{side.evidenceQuote}</div>
                    ) : null}
                  </li>
                ))}
              </ul>
              <div className="mt-2 flex flex-wrap gap-2">
                {conflict.candidateIds.length > 0 ? (
                  conflict.candidateIds.map((candidateId) => (
                    <Button
                      key={candidateId}
                      size="sm"
                      variant="outline"
                      data-testid="continuity-conflict-confirm"
                      disabled={conflictMutation.isPending}
                      onClick={() => conflictMutation.mutate({ conflict, decision: "confirm_new", chosenCandidateId: candidateId })}
                    >
                      采用这条候选
                    </Button>
                  ))
                ) : (
                  <Button
                    size="sm"
                    data-testid="continuity-conflict-confirm"
                    disabled={conflictMutation.isPending}
                    onClick={() => conflictMutation.mutate({ conflict, decision: "confirm_new" })}
                  >
                    采用新事实
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="outline"
                  data-testid="continuity-conflict-keep"
                  disabled={conflictMutation.isPending}
                  onClick={() => conflictMutation.mutate({ conflict, decision: "keep_existing" })}
                >
                  保留原事实
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  data-testid="continuity-conflict-dispute"
                  disabled={conflictMutation.isPending}
                  onClick={() => conflictMutation.mutate({ conflict, decision: "mark_disputed" })}
                >
                  标记待定
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setExpandedConflictId(expandedConflictId === conflict.id ? null : conflict.id)}
                >
                  {expandedConflictId === conflict.id ? "收起说明" : "裁决说明"}
                </Button>
              </div>
              {expandedConflictId === conflict.id ? (
                <div className="mt-2 text-[11px] leading-5 text-rose-900">
                  裁决会写入冲突记录并保留双方来源；系统不会自动覆盖已确认事实。采用新事实后，旧事实会被标记为已被替代。
                </div>
              ) : null}
            </div>
          ))
        )}
      </section>

      {projection.limits.factsTruncated || projection.limits.candidatesTruncated || projection.limits.conflictsTruncated ? (
        <div className="text-[11px] text-muted-foreground">
          事实、候选或冲突超过单次展示上限，已优先显示最近章节；请先处理当前条目再刷新。
        </div>
      ) : null}
    </div>
  );
}

function WorldCard({ world, secondary = false }: { world: ContinuityWorld; secondary?: boolean }) {
  return (
    <div className={secondary ? "rounded-xl bg-muted/20 px-3 py-2" : "rounded-xl bg-muted/40 px-3 py-2"} data-testid="continuity-world">
      <div className="text-sm text-foreground">{world.name?.trim() || "未命名世界"}</div>
      <div className="mt-1 text-xs leading-5 text-muted-foreground">
        {world.overviewSummary?.trim() || world.description?.trim() || "这个世界还没有可展示的设定说明。"}
      </div>
      <div className="mt-1 text-[11px] text-muted-foreground">状态：{world.status || "未知"}</div>
    </div>
  );
}

function SourceLink(props: {
  chapterId?: string | null;
  label: string;
  onOpenChapter?: (chapterId: string) => void;
}) {
  const { chapterId, label, onOpenChapter } = props;
  if (!chapterId || !onOpenChapter) {
    return <span className="text-[11px] text-muted-foreground">{label}</span>;
  }
  return (
    <button
      type="button"
      className="text-[11px] text-muted-foreground underline underline-offset-2 hover:text-foreground"
      data-testid="continuity-source-link"
      onClick={() => onOpenChapter(chapterId)}
    >
      {label}
    </button>
  );
}
