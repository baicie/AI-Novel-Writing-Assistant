import DesktopModelSettingsCard from "./DesktopModelSettingsCard";
import UpdateCheckCard from "./UpdateCheckCard";
import { LocalCreationKeyManager } from "@/components/localCreation";

// 桌面版只保留本地内核真正支持的设置项，避免出现「暂未接入」的入口。
export default function DesktopSettingsView() {
  return (
    <div className="space-y-6">
      <section className="space-y-2">
        <h2 className="text-base font-semibold">模型</h2>
        <p className="text-sm text-muted-foreground">
          桌面版直接使用你填写的接口地址和模型，写作时不会经过任何中间服务。
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
