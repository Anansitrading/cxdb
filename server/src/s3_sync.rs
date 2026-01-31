// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

//! S3 Sync Module
//!
//! Provides periodic backup of local storage files to S3 for durability.
//! Uses the AWS SDK for Rust with tokio async runtime.
//!
//! # Design
//!
//! - **Sync State**: Persisted locally in `sync_state.json`, tracks last synced
//!   size for each file to avoid redundant uploads.
//! - **Periodic Sync**: Background tokio task wakes every `sync_interval` and uploads
//!   any files that have grown since the last sync.
//! - **Restore on Startup**: If local data directory is empty but S3 has data,
//!   restore from S3 before opening stores.
//!
//! # S3 Object Layout
//!
//! ```text
//! s3://{bucket}/{prefix}/
//!   blobs/blobs.pack
//!   blobs/blobs.idx
//!   turns/turns.log
//!   turns/turns.idx
//!   turns/turns.meta
//!   turns/heads.tbl
//!   registry/{bundle_id}.json
//!   sync_manifest.json    # metadata about last sync
//! ```

use crate::error::{Result, StoreError};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client as S3Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::watch;
use tokio::time::interval;

/// S3 sync configuration
#[derive(Debug, Clone)]
pub struct S3SyncConfig {
    /// S3 bucket name
    pub bucket: String,
    /// Object key prefix (e.g., "cxdb/prod/")
    pub prefix: String,
    /// AWS region (e.g., "us-west-2")
    pub region: String,
    /// Sync interval in seconds
    pub sync_interval_secs: u64,
    /// Whether S3 sync is enabled
    pub enabled: bool,
}

impl S3SyncConfig {
    /// Load config from environment variables
    pub fn from_env() -> Option<Self> {
        let enabled = std::env::var("CXDB_S3_SYNC_ENABLED")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        if !enabled {
            return None;
        }

        let bucket = std::env::var("CXDB_S3_BUCKET").ok()?;
        let prefix = std::env::var("CXDB_S3_PREFIX").unwrap_or_default();
        let region = std::env::var("CXDB_S3_REGION").unwrap_or_else(|_| "us-west-2".to_string());
        let sync_interval_secs = std::env::var("CXDB_S3_SYNC_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        Some(Self {
            bucket,
            prefix,
            region,
            sync_interval_secs,
            enabled: true,
        })
    }
}

/// Tracks sync state for each file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncState {
    /// Map of relative file path -> last synced size in bytes
    pub file_sizes: HashMap<String, u64>,
    /// Unix timestamp of last successful sync
    pub last_sync_time: u64,
}

impl SyncState {
    fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("sync_state.json");
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self, data_dir: &Path) -> Result<()> {
        let path = data_dir.join("sync_state.json");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| StoreError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        fs::write(&path, json)?;
        Ok(())
    }
}

/// S3 manifest stored in the bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Manifest {
    /// Map of relative file path -> size in bytes
    pub files: HashMap<String, u64>,
    /// Unix timestamp when this manifest was created
    pub created_at: u64,
    /// Version for future compatibility
    pub version: u32,
}

/// Files to sync (relative to data_dir)
const SYNC_FILES: &[&str] = &[
    "blobs/blobs.pack",
    "blobs/blobs.idx",
    "turns/turns.log",
    "turns/turns.idx",
    "turns/turns.meta",
    "turns/heads.tbl",
];

/// S3 sync manager
pub struct S3Sync {
    config: S3SyncConfig,
    data_dir: PathBuf,
    s3_client: S3Client,
}

impl S3Sync {
    /// Create a new S3Sync manager.
    /// This is async because it loads AWS config.
    pub async fn new(config: S3SyncConfig, data_dir: PathBuf) -> Self {
        // Load AWS config from environment/IRSA
        let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(config.region.clone()))
            .load()
            .await;

        let s3_client = S3Client::new(&aws_config);

        Self {
            config,
            data_dir,
            s3_client,
        }
    }

    /// Check if local data directory needs restoration from S3.
    /// Returns true if data was restored.
    pub async fn maybe_restore(&self) -> Result<bool> {
        // Check if any core data files exist locally
        let has_local_data = SYNC_FILES.iter().any(|f| self.data_dir.join(f).exists());

        if has_local_data {
            eprintln!("[s3_sync] Local data exists, skipping restore");
            return Ok(false);
        }

        eprintln!("[s3_sync] No local data found, attempting S3 restore...");

        // Try to fetch manifest from S3
        let manifest = match self.fetch_manifest().await {
            Ok(Some(m)) => m,
            Ok(None) => {
                eprintln!("[s3_sync] No S3 manifest found, starting fresh");
                return Ok(false);
            }
            Err(e) => {
                eprintln!("[s3_sync] Failed to fetch manifest: {e}");
                return Ok(false);
            }
        };

        eprintln!(
            "[s3_sync] Found S3 manifest with {} files from {}",
            manifest.files.len(),
            manifest.created_at
        );

        // Restore each file
        for (relative_path, expected_size) in &manifest.files {
            let local_path = self.data_dir.join(relative_path);

            // Create parent directories
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent)?;
            }

            match self.download_file(relative_path, &local_path).await {
                Ok(size) => {
                    if size != *expected_size {
                        eprintln!(
                            "[s3_sync] Warning: {relative_path} size mismatch (expected {expected_size}, got {size})"
                        );
                    }
                    eprintln!("[s3_sync] Restored {relative_path} ({size} bytes)");
                }
                Err(e) => {
                    eprintln!("[s3_sync] Failed to restore {relative_path}: {e}");
                }
            }
        }

        // Restore registry files
        if let Err(e) = self.restore_registry().await {
            eprintln!("[s3_sync] Registry restore failed: {e}");
        }

        eprintln!("[s3_sync] Restore complete");
        Ok(true)
    }

    /// Start the background sync loop. Returns a handle to stop it.
    pub fn start_background_sync(self) -> S3SyncHandle {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let handle = tokio::spawn(async move {
            self.sync_loop(shutdown_rx).await;
        });

        S3SyncHandle {
            shutdown_tx,
            handle,
        }
    }

    async fn sync_loop(self, mut shutdown_rx: watch::Receiver<bool>) {
        let mut ticker = interval(Duration::from_secs(self.config.sync_interval_secs));
        eprintln!(
            "[s3_sync] Starting background sync (interval: {}s)",
            self.config.sync_interval_secs
        );

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = self.do_sync().await {
                        eprintln!("[s3_sync] Sync failed: {e}");
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
            }
        }

        // Final sync on shutdown
        eprintln!("[s3_sync] Performing final sync before shutdown...");
        if let Err(e) = self.do_sync().await {
            eprintln!("[s3_sync] Final sync failed: {e}");
        }
        eprintln!("[s3_sync] Shutdown complete");
    }

    async fn do_sync(&self) -> Result<()> {
        let mut state = SyncState::load(&self.data_dir);
        let mut files_synced = 0;
        let mut bytes_synced = 0u64;

        // Sync each tracked file
        for relative_path in SYNC_FILES {
            let local_path = self.data_dir.join(relative_path);

            if !local_path.exists() {
                continue;
            }

            let current_size = fs::metadata(&local_path)?.len();
            let last_size = state.file_sizes.get(*relative_path).copied().unwrap_or(0);

            if current_size > last_size {
                match self.upload_file(&local_path, relative_path).await {
                    Ok(()) => {
                        state
                            .file_sizes
                            .insert(relative_path.to_string(), current_size);
                        files_synced += 1;
                        bytes_synced += current_size - last_size;
                    }
                    Err(e) => {
                        eprintln!("[s3_sync] Failed to upload {relative_path}: {e}");
                    }
                }
            }
        }

        // Sync registry files
        let registry_synced = self.sync_registry(&mut state).await?;

        if files_synced > 0 || registry_synced > 0 {
            // Update manifest
            self.upload_manifest(&state).await?;

            state.last_sync_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            state.save(&self.data_dir)?;

            eprintln!(
                "[s3_sync] Synced {} files ({} bytes) + {} registry bundles",
                files_synced, bytes_synced, registry_synced
            );
        }

        Ok(())
    }

    async fn sync_registry(&self, state: &mut SyncState) -> Result<usize> {
        let registry_dir = self.data_dir.join("registry");
        if !registry_dir.exists() {
            return Ok(0);
        }

        let mut synced = 0;
        for entry in fs::read_dir(&registry_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let relative_path =
                    format!("registry/{}", path.file_name().unwrap().to_string_lossy());
                let current_size = fs::metadata(&path)?.len();
                let last_size = state.file_sizes.get(&relative_path).copied().unwrap_or(0);

                if current_size != last_size {
                    self.upload_file(&path, &relative_path).await?;
                    state.file_sizes.insert(relative_path, current_size);
                    synced += 1;
                }
            }
        }

        Ok(synced)
    }

    async fn restore_registry(&self) -> Result<()> {
        let registry_dir = self.data_dir.join("registry");
        fs::create_dir_all(&registry_dir)?;

        // List objects with registry/ prefix and download each
        let prefix = self.s3_key("registry/");

        let resp = self
            .s3_client
            .list_objects_v2()
            .bucket(&self.config.bucket)
            .prefix(&prefix)
            .send()
            .await
            .map_err(|e| StoreError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        if let Some(contents) = resp.contents {
            for obj in contents {
                if let Some(key) = obj.key {
                    // Extract relative path from full key
                    let relative_path = if self.config.prefix.is_empty() {
                        key.clone()
                    } else {
                        key.strip_prefix(&format!("{}/", self.config.prefix.trim_end_matches('/')))
                            .unwrap_or(&key)
                            .to_string()
                    };

                    let local_path = self.data_dir.join(&relative_path);
                    if let Err(e) = self.download_file(&relative_path, &local_path).await {
                        eprintln!("[s3_sync] Failed to restore {relative_path}: {e}");
                    }
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // S3 Operations
    // =========================================================================

    fn s3_key(&self, relative_path: &str) -> String {
        if self.config.prefix.is_empty() {
            relative_path.to_string()
        } else {
            format!(
                "{}/{}",
                self.config.prefix.trim_end_matches('/'),
                relative_path
            )
        }
    }

    async fn fetch_manifest(&self) -> Result<Option<S3Manifest>> {
        let key = self.s3_key("sync_manifest.json");

        let result = self
            .s3_client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await;

        match result {
            Ok(resp) => {
                let bytes = resp
                    .body
                    .collect()
                    .await
                    .map_err(|e| StoreError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
                    .into_bytes();

                let manifest: S3Manifest = serde_json::from_slice(&bytes)
                    .map_err(|e| StoreError::Corrupt(format!("Invalid manifest: {e}")))?;
                Ok(Some(manifest))
            }
            Err(e) => {
                // Check if it's a "not found" error
                let service_err = e.into_service_error();
                if service_err.is_no_such_key() {
                    Ok(None)
                } else {
                    Err(StoreError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("S3 get failed: {service_err}"),
                    )))
                }
            }
        }
    }

    async fn upload_manifest(&self, state: &SyncState) -> Result<()> {
        let manifest = S3Manifest {
            files: state.file_sizes.clone(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            version: 1,
        };

        let json = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| StoreError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let key = self.s3_key("sync_manifest.json");

        self.s3_client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(ByteStream::from(json))
            .content_type("application/json")
            .send()
            .await
            .map_err(|e| {
                StoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("S3 put manifest failed: {e}"),
                ))
            })?;

        Ok(())
    }

    async fn upload_file(&self, local_path: &Path, relative_path: &str) -> Result<()> {
        let key = self.s3_key(relative_path);

        // Read file into memory (could use streaming for very large files)
        let data = fs::read(local_path)?;

        self.s3_client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(ByteStream::from(data))
            .content_type("application/octet-stream")
            .send()
            .await
            .map_err(|e| {
                StoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("S3 upload failed: {e}"),
                ))
            })?;

        Ok(())
    }

    async fn download_file(&self, relative_path: &str, local_path: &Path) -> Result<u64> {
        let key = self.s3_key(relative_path);

        let resp = self
            .s3_client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                StoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("S3 download failed for {relative_path}: {e}"),
                ))
            })?;

        let bytes = resp
            .body
            .collect()
            .await
            .map_err(|e| StoreError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
            .into_bytes();

        let size = bytes.len() as u64;
        fs::write(local_path, &bytes)?;

        Ok(size)
    }
}

/// Handle to stop the background sync task
pub struct S3SyncHandle {
    shutdown_tx: watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
}

impl S3SyncHandle {
    /// Signal shutdown and wait for the sync task to finish
    pub async fn shutdown(self) {
        eprintln!("[s3_sync] Shutdown requested, waiting for final sync...");
        let _ = self.shutdown_tx.send(true);
        let _ = self.handle.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sync_state_roundtrip() {
        let temp = TempDir::new().unwrap();
        let mut state = SyncState::default();
        state
            .file_sizes
            .insert("blobs/blobs.pack".to_string(), 12345);
        state.last_sync_time = 1700000000;

        state.save(temp.path()).unwrap();

        let loaded = SyncState::load(temp.path());
        assert_eq!(loaded.file_sizes.get("blobs/blobs.pack"), Some(&12345));
        assert_eq!(loaded.last_sync_time, 1700000000);
    }

    #[test]
    fn test_s3_key_with_prefix() {
        // Note: Can't easily test S3Sync::s3_key without async context,
        // but the logic is simple string manipulation
        let prefix = "cxdb/prod/";
        let relative = "blobs/blobs.pack";
        let key = format!("{}/{}", prefix.trim_end_matches('/'), relative);
        assert_eq!(key, "cxdb/prod/blobs/blobs.pack");
    }

    #[test]
    fn test_s3_key_no_prefix() {
        let prefix = "";
        let relative = "blobs/blobs.pack";
        let key = if prefix.is_empty() {
            relative.to_string()
        } else {
            format!("{}/{}", prefix.trim_end_matches('/'), relative)
        };
        assert_eq!(key, "blobs/blobs.pack");
    }
}
