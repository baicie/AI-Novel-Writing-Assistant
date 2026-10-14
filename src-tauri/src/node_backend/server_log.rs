use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[derive(Clone)]
pub struct LogSink {
    path: PathBuf,
    file: Arc<Mutex<Option<File>>>,
}

impl LogSink {
    pub fn create(logs_dir: &Path) -> Self {
        let now = OffsetDateTime::now_utc();
        let path = logs_dir.join(format!(
            "desktop-server-{:04}{:02}{:02}-{:02}{:02}{:02}.log",
            now.year(),
            u8::from(now.month()),
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        ));
        let file = fs::create_dir_all(logs_dir)
            .and_then(|()| OpenOptions::new().create(true).append(true).open(&path));
        if let Err(error) = &file {
            tracing::warn!(%error, path = %path.display(), "无法创建本地服务日志");
        }
        Self {
            path,
            file: Arc::new(Mutex::new(file.ok())),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn line(&self, source: &str, message: &str) {
        let stamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default();
        let mut guard = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(file) = guard.as_mut() {
            let _ = writeln!(file, "{stamp} [{source}] {message}");
        }
    }

    pub fn pipe(&self, source: &'static str, stream: impl Read + Send + 'static) {
        let sink = self.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stream);
            let mut buffer = Vec::new();
            loop {
                buffer.clear();
                match reader.read_until(b'\n', &mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let text = String::from_utf8_lossy(&buffer);
                        let text = text.trim_end();
                        if !text.is_empty() {
                            sink.line(source, text);
                        }
                    }
                }
            }
        });
    }
}
