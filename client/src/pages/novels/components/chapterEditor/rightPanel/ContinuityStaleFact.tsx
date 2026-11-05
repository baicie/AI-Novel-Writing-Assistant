import { useState } from "react";
import { Button } from "@/components/ui/button";
import type { ContinuityFact } from "@/platform/desktopTransport";
import { factValueText, reconfirmMode, sourceLabel } from "./continuityModel";

interface ContinuityStaleFactProps {
  fact: ContinuityFact;
  isPending: boolean;
  onReconfirm: (input: {
    fact: ContinuityFact;
    evidenceQuote: string;
    effectiveChapterOrder: number;
  }) => void;
}

const TYPE_LABELS: Record<string, string> = {
  location: "地点",
  life_state: "生死",
  relationship: "关系",
  alias: "身份",
};

/**
 * 一条需要重新确认的结构化事实。
 *
 * 旧证据不存在时必须让作者从新正文里重新选取；来源顺序失效时必须重新选择生效章节。
 * 界面只负责收集输入，最终校验仍由内核在同一事务内完成。
 */
export default function ContinuityStaleFact(props: ContinuityStaleFactProps) {
  const { fact, isPending, onReconfirm } = props;
  const mode = reconfirmMode(fact);
  const [evidence, setEvidence] = useState(fact.evidenceStillPresent ? (fact.evidenceQuote ?? "") : "");
  const [effectiveOrder, setEffectiveOrder] = useState(
    fact.sourceOrderValid === false
      ? (fact.sourceChapterOrder ?? fact.effectiveChapterOrder)
      : fact.effectiveChapterOrder,
  );

  const reason =
    mode === "blocked"
      ? "来源章节不可用，无法重新确认；请先恢复来源章节或重新抽取事实。"
      : mode === "needs_order"
        ? `来源章节在第 ${fact.sourceChapterOrder ?? "?"} 章，晚于当前生效章节 ${fact.effectiveChapterOrder}；请把生效章节调整到来源章节之后。`
        : mode === "needs_evidence"
          ? "当前正文中已经找不到旧证据；请从最新正文中重新选取一段作为新证据，再确认。"
          : `来源章节已是版本 ${fact.sourceCurrentVersion ?? "未知"}，旧证据仍然存在，可以直接确认。`;

  const canSubmit =
    mode !== "blocked"
    && evidence.trim().length > 0
    && effectiveOrder > 0
    && (fact.sourceOrderValid !== false || effectiveOrder >= (fact.sourceChapterOrder ?? 0));

  return (
    <div className="rounded-xl bg-amber-50 px-3 py-2" data-testid="continuity-stale">
      <div className="text-xs text-amber-950">
        {TYPE_LABELS[fact.factType] ?? fact.factType}
        {fact.entityName ? ` · ${fact.entityName}` : ""} · {sourceLabel(fact)}
      </div>
      <div className="mt-1 text-xs leading-5 text-amber-950">{factValueText(fact)}</div>
      <div className="mt-1 text-[11px] leading-5 text-amber-800">{reason}</div>

      {mode === "needs_evidence" || mode === "needs_order" ? (
        <label className="mt-2 block text-[11px] text-amber-900">
          当前正文证据
          <textarea
            className="mt-1 h-16 w-full rounded-lg border border-amber-300 bg-background px-2 py-1 text-xs text-foreground"
            data-testid="continuity-new-evidence"
            value={evidence}
            onChange={(event) => setEvidence(event.target.value)}
            placeholder="从当前正式正文中复制一段原文"
          />
        </label>
      ) : null}

      {mode === "needs_order" ? (
        <label className="mt-2 block text-[11px] text-amber-900">
          生效章节
          <input
            type="number"
            min={Math.max(1, fact.sourceChapterOrder ?? 1)}
            className="mt-1 w-24 rounded-lg border border-amber-300 bg-background px-2 py-1 text-xs text-foreground"
            data-testid="continuity-effective-order"
            value={effectiveOrder}
            onChange={(event) => setEffectiveOrder(Number(event.target.value))}
          />
        </label>
      ) : null}

      <Button
        size="sm"
        variant="outline"
        className="mt-2"
        data-testid="continuity-reconfirm"
        disabled={isPending || !canSubmit}
        onClick={() =>
          onReconfirm({
            fact,
            evidenceQuote: evidence.trim(),
            effectiveChapterOrder: effectiveOrder,
          })
        }
      >
        {mode === "needs_evidence"
          ? "用新证据确认"
          : mode === "needs_order"
            ? "按新顺序确认"
            : "按当前版本确认"}
      </Button>
      {mode === "blocked" ? (
        <div className="mt-1 text-[11px] text-rose-800">来源不可用时不会自动补造证据。</div>
      ) : null}
    </div>
  );
}
