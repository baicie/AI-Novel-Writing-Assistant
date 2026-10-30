import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { providerClient } from "@/platform/providerTransport";

/**
 * 桌面版的模型状态：只读本地内核里的模型来源与任务绑定，
 * 不调用网页版厂商接口（那些接口在桌面端没有实现）。
 */
export default function DesktopModelStatus() {
  const navigate = useNavigate();
  const [label, setLabel] = useState("正在读取模型…");
  const [tone, setTone] = useState<"ok" | "warn">("ok");

  useEffect(() => {
    let alive = true;
    void (async () => {
      try {
        const [providers, route] = await Promise.all([
          providerClient.listProfiles(),
          providerClient.getEffectiveRoute("writer"),
        ]);
        if (!alive) return;
        if (!providers.length) {
          setTone("warn");
          setLabel("尚未添加模型来源");
          return;
        }
        if (route) {
          const name = providers.find((item) => item.id === route.providerId)?.displayName
            ?? route.providerId;
          setTone("ok");
          setLabel(`${name} · ${route.modelId}`);
          return;
        }
        const withKey = providers.filter((item) => item.credentialPresent);
        setTone(withKey.length ? "ok" : "warn");
        setLabel(withKey.length ? `${withKey.length} 个来源可用，未绑定任务模型` : "来源已添加，还没有可用密钥");
      } catch {
        if (alive) {
          setTone("warn");
          setLabel("读取模型状态失败");
        }
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  return (
    <div className="hidden items-center gap-2 sm:flex">
      <span
        className={
          tone === "ok" ? "text-xs text-muted-foreground" : "text-xs text-amber-600"
        }
      >
        {label}
      </span>
      <Button size="sm" variant="ghost" onClick={() => navigate("/settings")}>
        打开模型设置
      </Button>
    </div>
  );
}
