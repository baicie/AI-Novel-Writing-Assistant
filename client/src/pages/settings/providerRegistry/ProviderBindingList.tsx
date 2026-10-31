import type { ModelBindingView, ProviderView } from "@/platform/providerTransport";
import { BINDING_TASK_TYPES } from "../providerRegistryModel";

interface ProviderBindingListProps {
  providers: ProviderView[];
  bindings: ModelBindingView[];
  busy: boolean;
  onSelectProvider: (taskType: string, providerId: string, currentModelId: string | undefined) => void;
  onSelectEffort: (taskType: string, binding: ModelBindingView, effort: string | null) => void;
}

/** 任务绑定：给每个创作环节指定来源、模型与推理强度。 */
export default function ProviderBindingList(props: ProviderBindingListProps) {
  return (
    <div className="space-y-2">
      <h3 className="text-sm font-semibold">任务绑定</h3>
      <p className="text-xs text-muted-foreground">给每个创作环节指定模型，未绑定的环节使用默认模型。</p>
      <ul className="space-y-2">
        {BINDING_TASK_TYPES.map((task) => {
          const binding = props.bindings.find((item) => item.taskType === task.value);
          const provider = props.providers.find((item) => item.id === binding?.providerId);
          const supportsEffort = provider ? provider.protocol !== "ollama" : true;
          return (
            <li key={task.value} className="flex flex-wrap items-center gap-2 text-sm">
              <span className="w-24 text-muted-foreground">{task.label}</span>
              <select
                className="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
                value={binding?.providerId ?? ""}
                onChange={(event) => {
                  const providerId = event.target.value;
                  if (!providerId) return;
                  props.onSelectProvider(task.value, providerId, binding?.modelId);
                }}
              >
                <option value="">未绑定</option>
                {props.providers.map((provider) => (
                  <option key={provider.id} value={provider.id}>
                    {provider.displayName ?? provider.id}
                  </option>
                ))}
              </select>
              <span className="text-xs text-muted-foreground">{binding?.modelId ?? "默认模型"}</span>
              <select
                aria-label={task.label + " 推理强度"}
                className="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
                value={binding?.reasoningEffort ?? ""}
                disabled={!binding || props.busy}
                onChange={(event) => {
                  if (!binding) return;
                  props.onSelectEffort(task.value, binding, event.target.value || null);
                }}
              >
                <option value="">推理强度：默认</option>
                {["low", "medium", "high"].map((item) => (
                  <option key={item} value={item}>
                    {item}
                  </option>
                ))}
              </select>
              {supportsEffort ? null : (
                <span className="text-xs text-muted-foreground">该协议不支持强度</span>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
