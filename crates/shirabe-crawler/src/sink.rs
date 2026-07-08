//! Storage sinks — where extracted records go.
//!
//! A sink is just an async `write` of a JSON record. The crawler ships an
//! in-memory sink (for tests and small crawls) and an NDJSON file sink (the
//! standard "one JSON object per line" format that streams well and needs no
//! extra dependency). Anything heavier (SQLite, S3, a queue) is one impl away.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::error::CrawlError;

/// A place to land extracted records.
#[async_trait]
pub trait RecordSink: Send + Sync {
    async fn write(&self, record: Value) -> Result<(), CrawlError>;
}

/// In-memory sink — collects every record. Mainly for tests and tiny crawls.
#[derive(Default)]
pub struct MemorySink {
    inner: Arc<Mutex<Vec<Value>>>,
}

impl MemorySink {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn records(&self) -> Vec<Value> {
        self.inner.lock().await.clone()
    }

    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

#[async_trait]
impl RecordSink for MemorySink {
    async fn write(&self, record: Value) -> Result<(), CrawlError> {
        self.inner.lock().await.push(record);
        Ok(())
    }
}

/// Append-only NDJSON file sink. One JSON object per line, flushed per record.
pub struct NdjsonSink {
    path: PathBuf,
    file: Arc<Mutex<Option<tokio::fs::File>>>,
}

impl NdjsonSink {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            file: Arc::new(Mutex::new(None)),
        }
    }

    async fn ensure_open(&self) -> Result<(), CrawlError> {
        let mut guard = self.file.lock().await;
        if guard.is_none() {
            let f = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .await
                .map_err(|e| CrawlError::Sink(format!("open {}: {e}", self.path.display())))?;
            *guard = Some(f);
        }
        Ok(())
    }
}

#[async_trait]
impl RecordSink for NdjsonSink {
    async fn write(&self, record: Value) -> Result<(), CrawlError> {
        self.ensure_open().await?;
        let mut guard = self.file.lock().await;
        let Some(f) = guard.as_mut() else {
            return Err(CrawlError::Sink("file not open".into()));
        };
        let mut line = serde_json::to_string(&record)
            .map_err(|e| CrawlError::Sink(format!("serialize: {e}")))?;
        line.push('\n');
        use tokio::io::AsyncWriteExt;
        f.write_all(line.as_bytes())
            .await
            .map_err(|e| CrawlError::Sink(format!("write: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn memory_sink_collects() {
        let s = MemorySink::new();
        s.write(json!({"a": 1})).await.unwrap();
        s.write(json!({"b": 2})).await.unwrap();
        assert_eq!(s.len().await, 2);
        assert_eq!(s.records().await[0]["a"], 1);
    }

    #[tokio::test]
    async fn ndjson_sink_appends_lines() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("shirabe-crawler-ndjson-{}.jsonl", uuidish()));
        let s = NdjsonSink::new(&path);
        s.write(json!({"k": "v1"})).await.unwrap();
        s.write(json!({"k": "v2"})).await.unwrap();

        let text = tokio::fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = text.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("v1"));
        assert!(lines[1].contains("v2"));

        let _ = tokio::fs::remove_file(&path).await;
    }

    /// A poor man's unique id without pulling `uuid`. Process id + a static
    /// counter is unique enough within a test run.
    fn uuidish() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        format!(
            "{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        )
    }
}
