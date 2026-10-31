import { summarizeCast, type CastItem } from "./panelModel";

interface CastWorldTabProps {
  world?: { name: string; summary: string | null } | null;
  characters?: CastItem[];
}

/**
 * 「人物与世界观」：只展示已经存在于小说资料里的信息，不生成任何新的记忆事实。
 */
export default function CastWorldTab(props: CastWorldTabProps) {
  const { world = null, characters = [] } = props;
  const summary = summarizeCast(characters);

  return (
    <div className="flex flex-col gap-3 px-1 py-2" data-testid="cast-world-tab">
      <div className="space-y-1">
        <div className="text-sm font-medium text-foreground">{world?.name?.trim() || "还没有设定世界观名称"}</div>
        <div className="text-xs leading-5 text-muted-foreground">
          {world?.summary?.trim() || "世界观简介为空，可在小说基础信息里补充。"}
        </div>
      </div>

      <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
        <span className="rounded-full bg-muted px-2 py-1">人物 {summary.total}</span>
        <span className="rounded-full bg-muted px-2 py-1">主角 {summary.protagonists}</span>
        <span className="rounded-full bg-muted px-2 py-1">同伴 {summary.allies}</span>
        <span className="rounded-full bg-muted px-2 py-1">对手 {summary.antagonists}</span>
      </div>

      {characters.length === 0 ? (
        <div className="text-xs text-muted-foreground">还没有人物资料。</div>
      ) : (
        <ul className="space-y-2">
          {characters.map((character) => (
            <li key={character.id} className="rounded-xl bg-muted/40 px-3 py-2">
              <div className="text-sm text-foreground">
                {character.name}
                <span className="ml-2 text-xs text-muted-foreground">{character.role}</span>
              </div>
              {character.storyFunction || character.currentGoal ? (
                <div className="mt-1 text-xs leading-5 text-muted-foreground">
                  {[character.storyFunction, character.currentGoal].filter(Boolean).join(" · ")}
                </div>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
