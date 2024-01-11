use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};
use anyhow::Result;
use tokio::sync::RwLock;

#[derive(Debug, Eq, PartialEq)]
pub enum FileCacheStatus {
    FileMatches,
    FileChanged,
}

#[async_trait]
pub trait FileCache: Sync {
    async fn check_file(&self, path: &Path) -> Result<FileCacheStatus>;
    async fn update_cache_entry(&self, path: &Path) -> Result<()>;
}

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
    async fn check_file(&self, _path: &Path) -> Result<FileCacheStatus> {
        Ok(FileCacheStatus::FileChanged)
    }

    async fn update_cache_entry(&self, _path: &Path) -> Result<()> {
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct FileCacheData {
    #[serde(default)]
    checksums: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
pub struct FileBasedCache {
    data: Arc<RwLock<FileCacheData>>,
    path: String
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
                },
                Ok(r) => {
                    Ok(Self {
                        path: cache_path.display().to_string(),
                        data: Arc::new(RwLock::new(r))
                    })
                }
            }
        } else {
            Ok(Self {
                path: cache_path.display().to_string(),
                ..Default::default()
            })
        }
    }
    pub async fn write_storage(&self) -> Result<()>{
        let cache_data = self.data.read().await;
        match serde_json::to_string(cache_data.deref()) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&self.path, text.as_bytes()) {
                    warn!("Error writing cache data {:?}", e);
                    warn!(target: "user", "Failed to write updated cache to disk, next run will show incorrect results")
                }
            },
            Err(e) => {
                warn!("Error deserializing cache data {:?}", e);
                warn!(target: "user", "Unable to update cached value, next run will show incorrect results");
            }
        }

        Ok(())
    }
}

#[async_trait]
impl FileCache for FileBasedCache {
    async fn check_file(&self, path: &Path) -> Result<FileCacheStatus> {
        match make_checksum(path).await {
            Ok(checksum) => {
                if self.data.read().await.checksums.get(&path.display().to_string()) == Some(&checksum) {
                    Ok(FileCacheStatus::FileMatches)
                } else {
                    Ok(FileCacheStatus::FileChanged)
                }
            },
            Err(e) => {
                info!("Unable to make checksum of file. {:?}", e);
                Ok(FileCacheStatus::FileChanged)
            }
        }
    }

    async fn update_cache_entry(&self, path: &Path) -> Result<()> {
        match make_checksum(path).await {
            Ok(checksum) => {
                self.data.write().await.checksums.insert(path.display().to_string(), checksum);
            },
            Err(e) => {
                info!("Unable to make checksum of file. {:?}", e);
            }
        }

        Ok(())
    }
}

async fn make_checksum(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok("<not exist>".to_string())
    } else if path.is_dir() {
        return Ok("<dir>".to_string())
    }

    Ok(sha256::try_async_digest(path).await?)
}