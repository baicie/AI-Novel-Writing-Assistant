use std::path::Path;

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BootstrapState {
    Launching,
    StartingServer,
    Ready,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BootstrapStage {
    Launching,
    ServerStarting,
    ServerHealthy,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapSnapshot {
    pub state: BootstrapState,
    pub stage: BootstrapStage,
    pub title: String,
    pub detail: String,
    pub log_dir: String,
    pub log_file: String,
    pub updated_at_ms: i64,
    pub can_retry: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapStatus {
    Launching,
    StartingBundled,
    ConnectingExternal { port: u16 },
    Ready,
    BundleMissing,
    SpawnFailed,
    ExitedDuringStartup,
    StartupTimedOut,
    ExternalUnreachable { port: u16 },
    ExitedAfterReady,
}

const LOG_HINT: &str = "可以打开日志目录查看原因，或点击“重新启动”再试一次。";

impl BootstrapStatus {
    fn state(self) -> BootstrapState {
        match self {
            Self::Launching => BootstrapState::Launching,
            Self::StartingBundled | Self::ConnectingExternal { .. } => BootstrapState::StartingServer,
            Self::Ready => BootstrapState::Ready,
            _ => BootstrapState::Error,
        }
    }

    fn stage(self) -> BootstrapStage {
        match self.state() {
            BootstrapState::Launching => BootstrapStage::Launching,
            BootstrapState::StartingServer => BootstrapStage::ServerStarting,
            BootstrapState::Ready => BootstrapStage::ServerHealthy,
            BootstrapState::Error => BootstrapStage::Error,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Launching => "正在准备创作工作台",
            Self::StartingBundled => "正在启动本地创作服务",
            Self::ConnectingExternal { .. } => "正在连接本地创作服务",
            Self::Ready => "创作工作台已就绪",
            Self::BundleMissing => "安装包缺少本地创作服务",
            Self::SpawnFailed => "本地创作服务没有运行起来",
            Self::ExitedDuringStartup => "本地创作服务启动失败",
            Self::StartupTimedOut => "本地创作服务启动超时",
            Self::ExternalUnreachable { .. } => "没有连接到本地创作服务",
            Self::ExitedAfterReady => "本地创作服务停止运行",
        }
    }

    fn detail(self) -> String {
        match self {
            Self::Launching => "马上开始启动本地创作服务。".into(),
            Self::StartingBundled => "首次打开会先准备本机资料库，通常几秒内完成。".into(),
            Self::ConnectingExternal { port } => {
                format!("开发模式会连接本机 {port} 端口上的创作服务，请确认开发服务正在运行。")
            }
            Self::Ready => "本地创作服务运行正常。".into(),
            Self::BundleMissing => "这个安装包没有带上本地创作服务，请重新下载完整的安装包后再打开。".into(),
            Self::SpawnFailed => format!("系统没能运行本地创作服务。{LOG_HINT}"),
            Self::ExitedDuringStartup => format!("本地创作服务在启动过程中退出。{LOG_HINT}"),
            Self::StartupTimedOut => format!("等待 60 秒后服务仍没有响应。{LOG_HINT}"),
            Self::ExternalUnreachable { port } => {
                format!("开发模式需要本机 {port} 端口上的创作服务。启动开发服务后点击“重新启动”。")
            }
            Self::ExitedAfterReady => {
                "点击“重新启动”恢复创作工作台；如果反复出现，可以打开日志目录查看原因。".into()
            }
        }
    }

    fn can_retry(self) -> bool {
        !matches!(self, Self::BundleMissing)
    }

    pub fn snapshot(self, log_dir: &Path, log_file: &Path) -> BootstrapSnapshot {
        BootstrapSnapshot {
            state: self.state(),
            stage: self.stage(),
            title: self.title().into(),
            detail: self.detail(),
            log_dir: log_dir.display().to_string(),
            log_file: log_file.display().to_string(),
            updated_at_ms: novel_platform::now_ms(),
            can_retry: self.can_retry(),
        }
    }
}
