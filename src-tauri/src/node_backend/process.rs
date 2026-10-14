use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time::Duration;

use super::server_log::LogSink;

pub const NODE_BINARY_NAME: &str = if cfg!(windows) { "node.exe" } else { "node" };

const HEALTH_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const HEALTH_IO_TIMEOUT: Duration = Duration::from_secs(2);
const DESKTOP_ORIGINS: &str = "tauri://localhost,http://tauri.localhost,https://tauri.localhost";

pub fn sidecar_entry(server_dir: &Path) -> PathBuf {
    server_dir
        .join("dist")
        .join("app")
        .join("desktop")
        .join("sidecarEntry.js")
}

pub struct ServerLaunch<'a> {
    pub node: &'a Path,
    pub server_dir: &'a Path,
    pub data_dir: &'a Path,
    pub port: u16,
}

pub struct ManagedServer {
    pub child: Child,
    // The sidecar stops itself when this pipe closes, including when the shell process dies.
    pub control: Option<ChildStdin>,
}

pub fn spawn_server(launch: &ServerLaunch<'_>, log: &LogSink) -> io::Result<ManagedServer> {
    let mut command = Command::new(launch.node);
    command
        .arg(sidecar_entry(launch.server_dir))
        .current_dir(launch.server_dir)
        .env_remove("NODE_OPTIONS")
        .env_remove("ACCESS_PASSWORD")
        .env("NODE_ENV", "production")
        .env("AI_NOVEL_RUNTIME", "desktop")
        .env("AI_NOVEL_DESKTOP_PACKAGED", "true")
        .env("AI_NOVEL_APP_DATA_DIR", launch.data_dir)
        .env("AI_NOVEL_DATABASE_MODE", "sqlite")
        .env("DATABASE_URL", "file:./dev.db")
        .env("PORT", launch.port.to_string())
        .env("HOST", "127.0.0.1")
        .env("ALLOW_LAN", "false")
        .env("CORS_ORIGIN", DESKTOP_ORIGINS)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if std::env::var_os("DIRECTOR_NEXT_ENABLED").is_none() {
        command.env("DIRECTOR_NEXT_ENABLED", "true");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command.spawn()?;
    if let Some(stdout) = child.stdout.take() {
        log.pipe("server", stdout);
    }
    if let Some(stderr) = child.stderr.take() {
        log.pipe("server:stderr", stderr);
    }
    let control = child.stdin.take();
    Ok(ManagedServer { child, control })
}

pub fn reserve_loopback_port() -> io::Result<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    Ok(listener.local_addr()?.port())
}

pub fn is_server_healthy(port: u16) -> bool {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, HEALTH_CONNECT_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(HEALTH_IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(HEALTH_IO_TIMEOUT));
    let request =
        format!("GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut status_line = [0_u8; 12];
    let mut filled = 0;
    while filled < status_line.len() {
        match stream.read(&mut status_line[filled..]) {
            Ok(0) | Err(_) => return false,
            Ok(read) => filled += read,
        }
    }
    matches!(&status_line, b"HTTP/1.1 200" | b"HTTP/1.0 200")
}

pub fn reveal_directory(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)?;
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let mut child = Command::new(program).arg(path).spawn()?;
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
