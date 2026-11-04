/** 人物与世界观面板的纯展示逻辑：不发请求，便于确定性测试。 */

export interface FactValueInput {
  factType: string;
  value: string;
  valueJson?: string | null;
}

/** 把结构化事实的 JSON 值转成作者能读的一句话；无法识别时原样展示。 */
export function factValueText(fact: FactValueInput): string {
  let parsed: Record<string, unknown> | null = null;
  if (fact.valueJson) {
    try {
      const value: unknown = JSON.parse(fact.valueJson);
      parsed = value && typeof value === "object" ? (value as Record<string, unknown>) : null;
    } catch {
      parsed = null;
    }
  }
  if (!parsed) {
    return fact.value;
  }
  switch (fact.factType) {
    case "life_state": {
      const state = parsed.state;
      return state === "alive" ? "存活" : state === "dead" ? "死亡" : String(state ?? fact.value);
    }
    case "location":
      return String(parsed.location ?? parsed.name ?? fact.value);
    case "relationship": {
      const relation = String(parsed.relation ?? "");
      const target = String(parsed.targetEntityId ?? parsed.target ?? "");
      const active = parsed.active !== false;
      return `${relation || "关系"}${target ? ` · ${target}` : ""}${active ? "" : "（已结束）"}`;
    }
    case "alias":
      return String(parsed.alias ?? parsed.name ?? fact.value);
    default:
      return fact.value;
  }
}

export function sourceLabel(fact: {
  sourceChapterOrder?: number | null;
  sourceChapterTitle?: string | null;
}): string {
  if (fact.sourceChapterOrder == null) {
    return "来源不可用";
  }
  return `第${fact.sourceChapterOrder}章${fact.sourceChapterTitle ? ` ${fact.sourceChapterTitle}` : ""}`;
}

/** 只有来源版本已知且事实来自当前章节版本时，才允许一键重新确认。 */
export function canReconfirm(fact: {
  status: string;
  valueJson?: string | null;
  sourceChapterId?: string | null;
  sourceCurrentVersion?: number | null;
  entityId?: string | null;
}): boolean {
  return (
    fact.status !== "unavailable"
    && Boolean(fact.valueJson)
    && Boolean(fact.sourceChapterId)
    && fact.sourceCurrentVersion != null
    && Boolean(fact.entityId)
  );
}
