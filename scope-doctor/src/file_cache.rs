use crate::error::FileCacheError;
use anyhow::Result;
use async_trait::async_trait;
use mockall::automock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Eq, PartialEq)]
pub enum FileCacheStatus {
    FileMatches,
    FileChanged,
}

#[automock]
#[async_trait]
pub trait FileCache: Sync {
    async fn check_file(&self, check_name: String, path: &Path) -> Result<FileCacheStatus>;
    async fn update_cache_entry(&self, check_name: String, path: &Path) -> Result<()>;
    async fn persist(&self) -> Result<(), FileCacheError>;
}

#[derive(Debug)]
pub enum CacheStorage {
    NoCache(NoOpCache),
    File(FileBasedCache),
}

impl Deref for CacheStorage {
    type Target = dyn FileCache;

    fn deref(&self) -> &Self::Target {
        match self {
            CacheStorage::NoCache(d) => d,
            CacheStorage::File(f) => f,
        }
    }
}

#[derive(Default, Debug)]
pub struct NoOpCache {}

#[async_trait]
impl FileCache for NoOpCache {
    async fn check_file(&self, _check_name: String, _path: &Path) -> Result<FileCacheStatus> {
        Ok(FileCacheStatus::FileChanged)
    }

    async fn update_cache_entry(&self, _check_name: String, _path: &Path) -> Result<()> {
        Ok(())
    }

    async fn persist(&self) -> Result<(), FileCacheError> {
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct FileCacheData {
    #[serde(default)]
    checksums: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, Default)]
pub struct FileBasedCache {
    data: Arc<RwLock<FileCacheData>>,
    path: String,
}

impl FileBasedCache {
    pub fn new(cache_path: &Path) -> Result<Self> {
        if cache_path.exists() {
            let file = File::open(cache_path)?;
            match serde_json::from_reader(file) {
                Err(e) => {
                    warn!("Error when parsing file cache {:?}", e);
                    warn!(target: "user", "Unable to load cache, the file was not valid. Using empty cache.");
                    Ok(Self {
                        path: cache_path.display().to_string(),
                        ..Default::default()
                    })
                }
                Ok(r) => Ok(Self {
                    path: cache_path.display().to_string(),
                    data: Arc::new(RwLock::new(r)),
                }),
            }
        } else {
            Ok(Self {
                path: cache_path.display().to_string(),
                ..Default::default()
            })
        }
    }
}

#[async_trait]
impl FileCache for FileBasedCache {
    #[tracing::instrument(skip_all, fields(check.name = %check_name))]
    async fn check_file(&self, check_name: String, path: &Path) -> Result<FileCacheStatus> {
        match make_checksum(path).await {
            Ok(checksum) => {
                let data = self.data.read().await;
                let check_cache = data.checksums.get(&check_name).cloned().unwrap_or_default();
                if check_cache.get(&path.display().to_string()) == Some(&checksum) {
                    Ok(FileCacheStatus::FileMatches)
                } else {
                    Ok(FileCacheStatus::FileChanged)
                }
            }
            Err(e) => {
                info!("Unable to make checksum of file. {:?}", e);
                Ok(FileCacheStatus::FileChanged)
            }
        }
    }

    #[tracing::instrument(skip_all, fields(check.name = %check_name))]
    async fn update_cache_entry(&self, check_name: String, path: &Path) -> Result<()> {
        match make_checksum(path).await {
            Ok(checksum) => {
                let mut data = self.data.write().await;
                let check_cache = data.checksums.entry(check_name).or_default();
                check_cache.insert(path.display().to_string(), checksum);
            }
            Err(e) => {
                info!("Unable to make checksum of file. {:?}", e);
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn persist(&self) -> Result<(), FileCacheError> {
        let file_path = PathBuf::from(&self.path);
        let parent = match file_path.parent() {
            Some(parent) => parent,
            None => {
                return Err(FileCacheError::FsError);
            }
        };
        std::fs::create_dir_all(parent)?;
        let cache_data = self.data.read().await;
        match serde_json::to_string(cache_data.deref()) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&self.path, text.as_bytes()) {
                    warn!(target: "user", "Failed to write updated cache to disk, next run will show incorrect results");
                    return Err(FileCacheError::WriteIoError(e));
                }
            }
            Err(e) => {
                warn!(target: "user", "Unable to update cached value, next run will show incorrect results");
                return Err(FileCacheError::SerializationError(e));
            }
        }

        Ok(())
    }
}

async fn make_checksum(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok("<not exist>".to_string());
    } else if path.is_dir() {
        return Ok("<dir>".to_string());
    }

    Ok(sha256::try_async_digest(path).await?)
}
