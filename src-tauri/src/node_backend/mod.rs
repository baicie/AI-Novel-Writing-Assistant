mod process;
mod server_log;
mod snapshot;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use process::{ManagedServer, ServerLaunch};
use server_log::LogSink;
pub use snapshot::BootstrapSnapshot;
use snapshot::BootstrapStatus;

const DEV_SERVER_PORT: u16 = 3000;
const FALLBACK_SERVER_PORT: u16 = 39_210;
const API_TIMEOUT_MS: u64 = 600_000;
const BUNDLED_STARTUP_TIMEOUT: Duration = Duration::from_secs(60);
const EXTERNAL_STARTUP_TIMEOUT: Duration = Duration::from_secs(120);
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(250);
const EXIT_WATCH_INTERVAL: Duration = Duration::from_secs(1);
const STOP_GRACE_PERIOD: Duration = Duration::from_secs(5);

enum LaunchPlan {
    /// Tauri 进程自身就是本地创作服务（novel-http crate 在 3000 起 axum）。
    /// 这种情况下不生成 Node sidecar，也不等待外部 server。
    KernelIntegrated,
    Bundled { node: PathBuf, server_dir: PathBuf },
    External,
    BundleMissing,
}

enum ExitPoll {
    Running,
    Exited(Option<i32>),
    Detached,
}

pub struct NodeBackend {
    inner: Arc<Inner>,
}

struct Inner {
    plan: LaunchPlan,
    port: u16,
    data_dir: PathBuf,
    logs_dir: PathBuf,
    log: LogSink,
    snapshot: Mutex<BootstrapSnapshot>,
    server: Mutex<Option<ManagedServer>>,
}

impl NodeBackend {
    pub fn prepare(app: &AppHandle, data_dir: &Path) -> Self {
        let plan = resolve_launch_plan(app);
        let port = configured_port().unwrap_or_else(|| match plan {
            LaunchPlan::External => DEV_SERVER_PORT,
            LaunchPlan::KernelIntegrated => DEV_SERVER_PORT,
            _ => process::reserve_loopback_port().unwrap_or(FALLBACK_SERVER_PORT),
        });
        let logs_dir = data_dir.join("logs");
        let log = LogSink::create(&logs_dir);
        let snapshot = BootstrapStatus::Launching.snapshot(&logs_dir, log.path());
        Self {
            inner: Arc::new(Inner {
                plan,
                port,
                data_dir: data_dir.to_path_buf(),
                logs_dir,
                log,
                snapshot: Mutex::new(snapshot),
                server: Mutex::new(None),
            }),
        }
    }

    pub fn runtime_script(&self, app_version: &str) -> String {
        let runtime = serde_json::json!({
            "mode": "desktop",
            "apiBaseUrl": format!("http://127.0.0.1:{}/api", self.inner.port),
            "apiTimeoutMs": API_TIMEOUT_MS,
            "isPackaged": !matches!(self.inner.plan, LaunchPlan::External),
            "appVersion": app_version,
        });
        format!("window.__AI_NOVEL_RUNTIME__ = {runtime};")
    }

    pub fn start(&self) {
        let inner = Arc::clone(&self.inner);
        thread::spawn(move || inner.run());
    }

    pub fn snapshot(&self) -> BootstrapSnapshot {
        lock(&self.inner.snapshot).clone()
    }

    pub fn reveal_logs_directory(&self) -> std::io::Result<()> {
        process::reveal_directory(&self.inner.logs_dir)
    }

    pub fn stop(&self) {
        self.inner.stop();
    }
}

impl Inner {
    fn run(&self) {
        match &self.plan {
            LaunchPlan::KernelIntegrated => self.run_kernel_integrated(),
            LaunchPlan::Bundled { node, server_dir } => self.run_bundled(node, server_dir),
            LaunchPlan::External => self.run_external(),
            LaunchPlan::BundleMissing => {
                self.log.line("desktop", "bundled server resources are missing");
                self.publish(BootstrapStatus::BundleMissing);
            }
        }
    }

    fn run_kernel_integrated(&self) {
        // 内核就是 Tauri 进程本体，HTTP server 已经在 lib.rs 里起好，这里只把状态打到 Ready。
        self.log
            .line("desktop", "kernel-integrated mode: 不生成 sidecar");
        self.publish(BootstrapStatus::Ready);
    }

    fn run_bundled(&self, node: &Path, server_dir: &Path) {
        self.publish(BootstrapStatus::StartingBundled);
        self.log.line(
            "desktop",
            &format!(
                "starting bundled server on port {} with {} in {}",
                self.port,
                node.display(),
                server_dir.display()
            ),
        );
        let launch = ServerLaunch {
            node,
            server_dir,
            data_dir: &self.data_dir,
            port: self.port,
        };
        match process::spawn_server(&launch, &self.log) {
            Ok(server) => *lock(&self.server) = Some(server),
            Err(error) => {
                self.log.line("desktop", &format!("failed to spawn the server: {error}"));
                self.publish(BootstrapStatus::SpawnFailed);
                return;
            }
        }

        let deadline = Instant::now() + BUNDLED_STARTUP_TIMEOUT;
        loop {
            match self.poll_exit() {
                ExitPoll::Running => {}
                ExitPoll::Exited(code) => {
                    self.log_exit("server exited during startup", code);
                    self.publish(BootstrapStatus::ExitedDuringStartup);
                    return;
                }
                ExitPoll::Detached => return,
            }
            if process::is_server_healthy(self.port) {
                break;
            }
            if Instant::now() >= deadline {
                self.log.line("desktop", "server did not become healthy before the startup timeout");
                self.publish(BootstrapStatus::StartupTimedOut);
                self.stop();
                return;
            }
            thread::sleep(STARTUP_POLL_INTERVAL);
        }

        self.log.line("desktop", "server is healthy");
        self.publish(BootstrapStatus::Ready);
        loop {
            thread::sleep(EXIT_WATCH_INTERVAL);
            match self.poll_exit() {
                ExitPoll::Running => {}
                ExitPoll::Exited(code) => {
                    self.log_exit("server exited unexpectedly", code);
                    self.publish(BootstrapStatus::ExitedAfterReady);
                    return;
                }
                ExitPoll::Detached => return,
            }
        }
    }

    fn run_external(&self) {
        self.publish(BootstrapStatus::ConnectingExternal { port: self.port });
        self.log.line(
            "desktop",
            &format!("waiting for the development server on port {}", self.port),
        );
        let deadline = Instant::now() + EXTERNAL_STARTUP_TIMEOUT;
        while !process::is_server_healthy(self.port) {
            if Instant::now() >= deadline {
                self.log.line("desktop", "development server is unreachable");
                self.publish(BootstrapStatus::ExternalUnreachable { port: self.port });
                return;
            }
            thread::sleep(STARTUP_POLL_INTERVAL);
        }
        self.log.line("desktop", "development server is healthy");
        self.publish(BootstrapStatus::Ready);
    }

    fn poll_exit(&self) -> ExitPoll {
        let mut guard = lock(&self.server);
        let Some(server) = guard.as_mut() else {
            return ExitPoll::Detached;
        };
        match server.child.try_wait() {
            Ok(Some(status)) => {
                guard.take();
                ExitPoll::Exited(status.code())
            }
            Ok(None) | Err(_) => ExitPoll::Running,
        }
    }

    fn stop(&self) {
        let Some(mut server) = lock(&self.server).take() else {
            return;
        };
        self.log.line("desktop", "stopping the server");
        drop(server.control.take());
        let deadline = Instant::now() + STOP_GRACE_PERIOD;
        loop {
            match server.child.try_wait() {
                Ok(Some(status)) => {
                    self.log_exit("server stopped", status.code());
                    return;
                }
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
                _ => break,
            }
        }
        self.log.line("desktop", "server did not stop in time; terminating it");
        let _ = server.child.kill();
        let _ = server.child.wait();
    }

    fn publish(&self, status: BootstrapStatus) {
        *lock(&self.snapshot) = status.snapshot(&self.logs_dir, self.log.path());
    }

    fn log_exit(&self, message: &str, code: Option<i32>) {
        let code = code.map_or_else(|| "none".to_string(), |code| code.to_string());
        self.log.line("desktop", &format!("{message} (exit code {code})"));
    }
}

fn resolve_launch_plan(app: &AppHandle) -> LaunchPlan {
    // 优先级：`AI_NOVEL_KERNEL_BACKEND=1` 直接走内核集成模式，跳过 Node sidecar。
    // 这个开关在 stage 1 切流期间用于 Tauri 进程内起 novel-http 的 axum server；
    // 见 `docs/wiki/architecture/backend-rustization-roadmap.md`。
    if std::env::var("AI_NOVEL_KERNEL_BACKEND")
        .ok()
        .filter(|value| !value.is_empty() && value != "0" && value != "false")
        .is_some()
    {
        return LaunchPlan::KernelIntegrated;
    }
    let requested = std::env::var("AI_NOVEL_DESKTOP_SERVER_MODE").unwrap_or_default();
    let use_external = match requested.trim() {
        "external" => true,
        "bundled" => false,
        _ => cfg!(debug_assertions),
    };
    if use_external {
        return LaunchPlan::External;
    }
    let server_dir = app.path().resource_dir().ok().map(|dir| dir.join("server"));
    let node = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(process::NODE_BINARY_NAME)));
    match (server_dir, node) {
        (Some(server_dir), Some(node))
            if process::sidecar_entry(&server_dir).is_file() && node.is_file() =>
        {
            LaunchPlan::Bundled { node, server_dir }
        }
        _ => LaunchPlan::BundleMissing,
    }
}

fn configured_port() -> Option<u16> {
    std::env::var("AI_NOVEL_SERVER_PORT")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .filter(|port| *port != 0)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
