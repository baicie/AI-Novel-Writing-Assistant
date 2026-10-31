import DesktopModelSettingsCard from "./DesktopModelSettingsCard";
import ProviderRegistrySection from "./ProviderRegistrySection";
import UpdateCheckCard from "./UpdateCheckCard";
import { LocalCreationKeyManager } from "@/components/localCreation";

// 桌面版只保留本地内核真正支持的设置项，避免出现「暂未接入」的入口。
export default function DesktopSettingsView() {
  return (
    <div className="space-y-6">
      <section className="space-y-3">
        <h2 className="text-base font-semibold">模型来源</h2>
        <p className="text-sm text-muted-foreground">
          在这里添加模型服务、取回可用模型，并给每个创作环节指定模型。密钥保存在系统凭据里，不会显示在页面上。
        </p>
        <ProviderRegistrySection />
      </section>

      <section className="space-y-2">
        <h2 className="text-base font-semibold">默认模型</h2>
        <p className="text-sm text-muted-foreground">
          没有单独指定模型的创作环节使用这里配置的地址与模型。
        </p>
        <DesktopModelSettingsCard />
      </section>

      <section className="space-y-2">
        <h2 className="text-base font-semibold">本地创作</h2>
        <p className="text-sm text-muted-foreground">本地生成使用的密钥，保存在系统凭据里。</p>
        <div className="max-w-md">
          <LocalCreationKeyManager />
        </div>
      </section>

      <section className="space-y-2">
        <h2 className="text-base font-semibold">版本更新</h2>
        <div className="max-w-md">
          <UpdateCheckCard />
        </div>
      </section>
    </div>
  );
}