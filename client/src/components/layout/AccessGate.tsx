import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { LockKeyhole } from "lucide-react";
import { API_BASE_URL, APP_RUNTIME } from "@/lib/constants";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface AccessStatus {
  required: boolean;
  authenticated: boolean;
}

interface AccessSessionValue {
  required: boolean;
  logout: () => Promise<void>;
}

const AccessSessionContext = createContext<AccessSessionValue>({
  required: false,
  logout: async () => undefined,
});

export function useAccessSession(): AccessSessionValue {
  return useContext(AccessSessionContext);
}

export const ACCESS_REQUIRED_EVENT = "ai-novel-access-required";

export function notifyAccessRequired(): void {
  window.dispatchEvent(new Event(ACCESS_REQUIRED_EVENT));
}

async function readAccessStatus(signal?: AbortSignal): Promise<AccessStatus> {
  const response = await fetch(`${API_BASE_URL}/access/status`, {
    cache: "no-store",
    credentials: "include",
    signal,
  });
  if (!response.ok) {
    throw new Error("无法确认访问状态。");
  }
  const payload = await response.json() as { data?: AccessStatus };
  return payload.data ?? { required: false, authenticated: true };
}

function LoginScreen(props: { onSuccess: () => void }) {
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);

  async function submit() {
    setPending(true);
    setError("");
    try {
      const response = await fetch(`${API_BASE_URL}/access/login`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ password }),
      });
      const payload = await response.json() as { error?: string };
      if (!response.ok) {
        setError(payload.error || "暂时无法进入，请稍后再试。");
        return;
      }
      props.onSuccess();
    } catch {
      setError("暂时无法连接服务器，请稍后再试。");
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-background px-6">
      <form
        className="w-full max-w-sm"
        onSubmit={(event) => {
          event.preventDefault();
          if (!pending && password.trim()) {
            void submit();
          }
        }}
      >
        <div className="flex size-12 items-center justify-center rounded-full bg-muted">
          <LockKeyhole className="size-5 text-primary" aria-hidden="true" />
        </div>
        <h1 className="mt-5 text-xl font-semibold text-foreground">输入访问密码</h1>
        <p className="mt-3 text-sm leading-6 text-muted-foreground">
          这台服务器上的小说、设定和模型密钥需要密码才能打开。密码由部署服务器的人设置。
        </p>
        <label className="mt-6 block text-sm font-medium text-foreground" htmlFor="access-password">
          访问密码
        </label>
        <Input
          id="access-password"
          className="mt-2"
          type="password"
          autoComplete="current-password"
          autoFocus
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
        {error ? <p className="mt-3 text-sm text-destructive">{error}</p> : null}
        <Button className="mt-6 w-full" type="submit" disabled={pending || !password.trim()}>
          {pending ? "正在进入…" : "进入工作台"}
        </Button>
      </form>
    </div>
  );
}

export default function AccessGate({ children }: { children: ReactNode }) {
  // Desktop 应用使用本地 Runtime，无需访问控制
  if (APP_RUNTIME === "desktop") {
    return children;
  }

  const [required, setRequired] = useState(false);
  const [phase, setPhase] = useState<"checking" | "open" | "locked" | "unreachable">("checking");
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    let timer = 0;
    readAccessStatus(controller.signal)
      .then((status) => {
        setRequired(status.required);
        setPhase(status.required && !status.authenticated ? "locked" : "open");
      })
      .catch((error: unknown) => {
        if (error instanceof DOMException && error.name === "AbortError") {
          return;
        }
        if (attempt < 20) {
          timer = window.setTimeout(() => setAttempt((current) => current + 1), 1000);
          return;
        }
        setPhase("unreachable");
      });
    return () => {
      controller.abort();
      window.clearTimeout(timer);
    };
  }, [attempt]);

  useEffect(() => {
    function lock() {
      setPhase("locked");
    }
    window.addEventListener(ACCESS_REQUIRED_EVENT, lock);
    return () => window.removeEventListener(ACCESS_REQUIRED_EVENT, lock);
  }, []);

  async function logout() {
    await fetch(`${API_BASE_URL}/access/logout`, { method: "POST", credentials: "include" });
    setPhase("locked");
  }

  if (phase === "checking") {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background text-sm text-muted-foreground">
        正在打开工作台…
      </div>
    );
  }

  if (phase === "unreachable") {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background px-6 text-center">
        <div>
          <h1 className="text-xl font-semibold text-foreground">暂时无法连接创作服务</h1>
          <p className="mt-3 text-sm leading-6 text-muted-foreground">请确认服务器已经启动，然后重新检查。</p>
          <Button className="mt-6" type="button" variant="outline" onClick={() => { setAttempt(0); setPhase("checking"); }}>
            重新检查
          </Button>
        </div>
      </div>
    );
  }

  return (
    <AccessSessionContext.Provider value={{ required, logout }}>
      {phase === "locked" ? <LoginScreen onSuccess={() => setPhase("open")} /> : children}
    </AccessSessionContext.Provider>
  );
}
