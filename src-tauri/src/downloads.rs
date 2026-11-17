use crate::db::{Database, DownloadRecord};
use crate::error::{VortexError, VortexResult};
use chrono::Utc;
use futures_util::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressEvent {
    pub id: String,
    pub received_bytes: i64,
    pub total_bytes: i64,
    pub speed_bps: u64,
    pub status: String,
}

pub struct DownloadManager {
    db: Arc<Database>,
    default_dir: PathBuf,
    cancel_signals: Mutex<HashMap<String, watch::Sender<String>>>, // "run" | "pause" | "cancel"
}

impl DownloadManager {
    pub fn new(db: Arc<Database>, default_dir: PathBuf) -> VortexResult<Self> {
        std::fs::create_dir_all(&default_dir)?;
        Ok(Self {
            db,
            default_dir,
            cancel_signals: Mutex::new(HashMap::new()),
        })
    }

    pub fn has_active_downloads(&self) -> bool {
        !self.cancel_signals.lock().unwrap().is_empty()
    }

    pub fn list(&self) -> VortexResult<Vec<DownloadRecord>> {
        self.db.list_downloads()
    }

    pub fn start_download(
        self: &Arc<Self>,
        app: AppHandle,
        url: String,
        filename: Option<String>,
        save_path: Option<String>,
    ) -> VortexResult<DownloadRecord> {
        let parsed = url::Url::parse(&url).map_err(|e| VortexError::InvalidUrl(e.to_string()))?;
        let resolved_name = filename.unwrap_or_else(|| {
            parsed
                .path_segments()
                .and_then(|s| s.last())
                .filter(|s| !s.is_empty())
                .unwrap_or("vortex-download.bin")
                .to_string()
        });
        let target_path = save_path
            .map(PathBuf::from)
            .unwrap_or_else(|| self.default_dir.join(&resolved_name));

        let record = DownloadRecord {
            id: uuid::Uuid::new_v4().to_string(),
            filename: resolved_name,
            url,
            save_path: target_path.to_string_lossy().to_string(),
            total_bytes: 0,
            received_bytes: 0,
            status: "downloading".to_string(),
            mime_type: None,
            supports_resume: false,
            started_at: Utc::now().timestamp_millis(),
            completed_at: None,
        };

        self.db.upsert_download(&record)?;
        self.spawn_worker(app, record.clone(), 0);
        Ok(record)
    }

    pub fn pause(&self, id: &str) -> VortexResult<()> {
        if let Some(tx) = self.cancel_signals.lock().unwrap().get(id) {
            let _ = tx.send("pause".to_string());
        }
        Ok(())
    }

    pub fn resume(self: &Arc<Self>, app: AppHandle, id: &str) -> VortexResult<DownloadRecord> {
        let records = self.db.list_downloads()?;
        let mut record = records
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| VortexError::Validation("Завантаження не знайдено".into()))?;
        let existing_len = std::fs::metadata(&record.save_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);
        record.status = "downloading".to_string();
        record.received_bytes = existing_len;
        self.db.upsert_download(&record)?;
        self.spawn_worker(app, record.clone(), existing_len as u64);
        Ok(record)
    }

    fn spawn_worker(self: &Arc<Self>, app: AppHandle, mut record: DownloadRecord, offset: u64) {
        let (tx, rx) = watch::channel("run".to_string());
        self.cancel_signals
            .lock()
            .unwrap()
            .insert(record.id.clone(), tx);

        let manager = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let client = reqwest::Client::builder()
                .user_agent("VortexBrowser/1.0")
                .build()
                .unwrap_or_default();

            let mut req = client.get(&record.url);
            if offset > 0 {
                req = req.header(reqwest::header::RANGE, format!("bytes={}-", offset));
            }

            let resp = match req.send().await {
                Ok(r) if r.status().is_success() || r.status().as_u16() == 206 => r,
                _ => {
                    record.status = "failed".to_string();
                    let _ = manager.db.upsert_download(&record);
                    let _ = app.emit(
                        "vortex://download-progress",
                        DownloadProgressEvent {
                            id: record.id.clone(),
                            received_bytes: record.received_bytes,
                            total_bytes: record.total_bytes,
                            speed_bps: 0,
                            status: "failed".to_string(),
                        },
                    );
                    manager.cancel_signals.lock().unwrap().remove(&record.id);
                    return;
                }
            };

            let supports_resume = resp
                .headers()
                .get(reqwest::header::ACCEPT_RANGES)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("bytes"))
                .unwrap_or(false);
            record.supports_resume = supports_resume;
            if let Some(ct) = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
            {
                record.mime_type = Some(ct.to_string());
            }

            let content_len = resp.content_length().unwrap_or(0) as i64;
            if offset == 0 {
                record.total_bytes = content_len;
            } else if record.total_bytes == 0 && content_len > 0 {
                record.total_bytes = offset as i64 + content_len;
            }

            let mut file = match OpenOptions::new()
                .create(true)
                .write(true)
                .append(offset > 0)
                .truncate(offset == 0)
                .open(&record.save_path)
                .await
            {
                Ok(f) => f,
                Err(_) => {
                    record.status = "failed".to_string();
                    let _ = manager.db.upsert_download(&record);
                    manager.cancel_signals.lock().unwrap().remove(&record.id);
                    return;
                }
            };

            let mut stream = resp.bytes_stream();
            let mut received = offset as i64;
            let mut window_bytes = 0u64;
            let mut last_tick = std::time::Instant::now();

            while let Some(chunk_res) = stream.next().await {
                let signal = rx.borrow().clone();
                if signal == "pause" {
                    record.status = "paused".to_string();
                    record.received_bytes = received;
                    let _ = manager.db.upsert_download(&record);
                    let _ = app.emit(
                        "vortex://download-progress",
                        DownloadProgressEvent {
                            id: record.id.clone(),
                            received_bytes: received,
                            total_bytes: record.total_bytes,
                            speed_bps: 0,
                            status: "paused".to_string(),
                        },
                    );
                    manager.cancel_signals.lock().unwrap().remove(&record.id);
                    return;
                }

                match chunk_res {
                    Ok(chunk) => {
                        if file.write_all(&chunk).await.is_err() {
                            record.status = "failed".to_string();
                            break;
                        }
                        received += chunk.len() as i64;
                        window_bytes += chunk.len() as u64;

                        let elapsed = last_tick.elapsed();
                        if elapsed.as_millis() >= 250 {
                            let speed_bps =
                                ((window_bytes as f64) / elapsed.as_secs_f64()) as u64;
                            record.received_bytes = received;
                            let _ = app.emit(
                                "vortex://download-progress",
                                DownloadProgressEvent {
                                    id: record.id.clone(),
                                    received_bytes: received,
                                    total_bytes: record.total_bytes,
                                    speed_bps,
                                    status: "downloading".to_string(),
                                },
                            );
                            window_bytes = 0;
                            last_tick = std::time::Instant::now();
                        }
                    }
                    Err(_) => {
                        record.status = "failed".to_string();
                        break;
                    }
                }
            }

            let _ = file.flush().await;
            if record.status == "downloading" {
                record.status = "completed".to_string();
                record.received_bytes = received;
                if record.total_bytes == 0 {
                    record.total_bytes = received;
                }
                record.completed_at = Some(Utc::now().timestamp_millis());
            }

            let _ = manager.db.upsert_download(&record);
            let _ = app.emit(
                "vortex://download-progress",
                DownloadProgressEvent {
                    id: record.id.clone(),
                    received_bytes: record.received_bytes,
                    total_bytes: record.total_bytes,
                    speed_bps: 0,
                    status: record.status.clone(),
                },
            );
            manager.cancel_signals.lock().unwrap().remove(&record.id);
        });
    }

    pub fn open_folder(&self, save_path: &str) -> VortexResult<()> {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(format!("/select,{}", save_path))
                .spawn()?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = save_path;
        }
        Ok(())
    }
}
