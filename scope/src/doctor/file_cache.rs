use super::error::FileCacheError;
use anyhow::Result;
use async_trait::async_trait;
use mockall::automock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;
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
pub trait FileCache: Sync + Send + Debug {
    async fn check_file(&self, check_name: String, path: &Path) -> Result<FileCacheStatus>;
    async fn update_cache_entry(&self, check_name: String, path: &Path) -> Result<()>;
    async fn persist(&self) -> Result<(), FileCacheError>;
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
    // group-name: { action-name: { file-path: checksum } }
    // checksums: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>,
    #[serde(default)]
    checksums: BTreeMap<String, GroupCache>, // group-name -> file-path -> checksum
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct GroupCache {
    #[serde(flatten)]
    files: BTreeMap<String, String>, // file-path -> checksum
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
                if check_cache.files.get(&path.display().to_string()) == Some(&checksum) {
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
                check_cache.files.insert(path.display().to_string(), checksum);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{tempdir, TempDir};

    /// Helper function to create a temporary directory and file for testing
    fn setup_temp_cache() -> (TempDir, PathBuf) {
        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");
        (temp_dir, cache_path)
    }

    /// Helper function to create a test file with content
    fn create_test_file(temp_dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let file_path = temp_dir.path().join(name);
        fs::write(&file_path, content).unwrap();
        file_path
    }

    mod file_based_cache {
        use super::*;

        #[tokio::test]
        async fn test_new_cache_with_nonexistent_file() {
            let (_temp_dir, cache_path) = setup_temp_cache();

            let cache = FileBasedCache::new(&cache_path).unwrap();

            assert_eq!(cache.path, cache_path.display().to_string());

            // Should return FileChanged for any file since cache is empty
            let status = cache
                .check_file("test-group".to_string(), &cache_path)
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileChanged);
        }

        #[tokio::test]
        async fn test_new_cache_with_valid_existing_file() {
            let (_temp_dir, cache_path) = setup_temp_cache();

            // Create a valid cache file
            let cache_data = r#"{"checksums":{"test-group":{"/test/file":"abcd1234"}}}"#;
            fs::write(&cache_path, cache_data).unwrap();

            let cache = FileBasedCache::new(&cache_path).unwrap();
            assert_eq!(cache.path, cache_path.display().to_string());
        }

        #[tokio::test]
        async fn test_new_cache_with_invalid_json() {
            let (_temp_dir, cache_path) = setup_temp_cache();

            // Create an invalid JSON file
            fs::write(&cache_path, "invalid json").unwrap();

            let cache = FileBasedCache::new(&cache_path).unwrap();

            // Should still create cache but with empty data
            let status = cache
                .check_file("test-group".to_string(), &cache_path)
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileChanged);
        }

        #[tokio::test]
        async fn test_check_file_not_in_cache() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file = create_test_file(&temp_dir, "test.txt", "content");

            let cache = FileBasedCache::new(&cache_path).unwrap();

            let status = cache
                .check_file("test-group".to_string(), &test_file)
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileChanged);
        }

        #[tokio::test]
        async fn test_check_file_matches_cache() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file = create_test_file(&temp_dir, "test.txt", "content");

            let cache = FileBasedCache::new(&cache_path).unwrap();

            // First update the cache
            cache
                .update_cache_entry("test-group".to_string(), &test_file)
                .await
                .unwrap();

            // Then check - should match
            let status = cache
                .check_file("test-group".to_string(), &test_file)
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileMatches);
        }

        #[tokio::test]
        async fn test_check_file_changed_content() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file = create_test_file(&temp_dir, "test.txt", "original content");

            let cache = FileBasedCache::new(&cache_path).unwrap();

            // Update cache with original content
            cache
                .update_cache_entry("test-group".to_string(), &test_file)
                .await
                .unwrap();

            // Modify the file
            fs::write(&test_file, "modified content").unwrap();

            // Should detect change
            let status = cache
                .check_file("test-group".to_string(), &test_file)
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileChanged);
        }

        #[tokio::test]
        async fn test_check_nonexistent_file() {
            let (_temp_dir, cache_path) = setup_temp_cache();
            let nonexistent_file = PathBuf::from("/nonexistent/file.txt");

            let cache = FileBasedCache::new(&cache_path).unwrap();

            let status = cache
                .check_file("test-group".to_string(), &nonexistent_file)
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileChanged);
        }

        #[tokio::test]
        async fn test_check_directory() {
            let (temp_dir, cache_path) = setup_temp_cache();

            let cache = FileBasedCache::new(&cache_path).unwrap();

            let status = cache
                .check_file("test-group".to_string(), temp_dir.path())
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileChanged);
        }

        #[tokio::test]
        async fn test_update_cache_entry() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file = create_test_file(&temp_dir, "test.txt", "content");

            let cache = FileBasedCache::new(&cache_path).unwrap();

            // Update cache entry
            cache
                .update_cache_entry("test-group".to_string(), &test_file)
                .await
                .unwrap();

            // Verify it was added
            let status = cache
                .check_file("test-group".to_string(), &test_file)
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileMatches);
        }

        #[tokio::test]
        async fn test_update_cache_entry_nonexistent_file() {
            let (_temp_dir, cache_path) = setup_temp_cache();
            let nonexistent_file = PathBuf::from("/nonexistent/file.txt");

            let cache = FileBasedCache::new(&cache_path).unwrap();

            // Should not panic or error
            cache
                .update_cache_entry("test-group".to_string(), &nonexistent_file)
                .await
                .unwrap();

            // Check should return FileMatches since the file consistently doesn't exist
            let status = cache
                .check_file("test-group".to_string(), &nonexistent_file)
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileMatches);
        }

        #[tokio::test]
        async fn test_multiple_groups_cache_separately() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file1 = create_test_file(&temp_dir, "test1.txt", "content1");
            let test_file2 = create_test_file(&temp_dir, "test2.txt", "content2");

            let cache = FileBasedCache::new(&cache_path).unwrap();

            // Update different groups
            cache
                .update_cache_entry("group1".to_string(), &test_file1)
                .await
                .unwrap();
            cache
                .update_cache_entry("group2".to_string(), &test_file2)
                .await
                .unwrap();

            // Both should match their respective groups
            let status1 = cache
                .check_file("group1".to_string(), &test_file1)
                .await
                .unwrap();
            let status2 = cache
                .check_file("group2".to_string(), &test_file2)
                .await
                .unwrap();

            assert_eq!(status1, FileCacheStatus::FileMatches);
            assert_eq!(status2, FileCacheStatus::FileMatches);
        }

        #[tokio::test]
        async fn test_cross_group_checks_fail() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file1 = create_test_file(&temp_dir, "test1.txt", "content1");
            let test_file2 = create_test_file(&temp_dir, "test2.txt", "content2");

            let cache = FileBasedCache::new(&cache_path).unwrap();

            // Update different groups
            cache
                .update_cache_entry("group1".to_string(), &test_file1)
                .await
                .unwrap();
            cache
                .update_cache_entry("group2".to_string(), &test_file2)
                .await
                .unwrap();

            // Cross-group checks should fail
            let status_cross = cache
                .check_file("group1".to_string(), &test_file2)
                .await
                .unwrap();
            assert_eq!(status_cross, FileCacheStatus::FileChanged);
        }

        #[tokio::test]
        async fn test_same_file_different_groups() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file = create_test_file(&temp_dir, "test.txt", "content");

            let cache = FileBasedCache::new(&cache_path).unwrap();

            // Update same file in different groups
            cache
                .update_cache_entry("group1".to_string(), &test_file)
                .await
                .unwrap();
            cache
                .update_cache_entry("group2".to_string(), &test_file)
                .await
                .unwrap();

            // Both groups should match
            let status1 = cache
                .check_file("group1".to_string(), &test_file)
                .await
                .unwrap();
            let status2 = cache
                .check_file("group2".to_string(), &test_file)
                .await
                .unwrap();

            assert_eq!(status1, FileCacheStatus::FileMatches);
            assert_eq!(status2, FileCacheStatus::FileMatches);
        }

        #[tokio::test]
        async fn test_persist_creates_cache_file() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file = create_test_file(&temp_dir, "test.txt", "content");

            let cache = FileBasedCache::new(&cache_path).unwrap();
            cache
                .update_cache_entry("test-group".to_string(), &test_file)
                .await
                .unwrap();

            cache.persist().await.unwrap();

            assert!(cache_path.exists());
        }

        #[tokio::test]
        async fn test_persist_creates_valid_json() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file = create_test_file(&temp_dir, "test.txt", "content");

            let cache = FileBasedCache::new(&cache_path).unwrap();
            cache
                .update_cache_entry("test-group".to_string(), &test_file)
                .await
                .unwrap();

            cache.persist().await.unwrap();

            let content = fs::read_to_string(&cache_path).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

            assert!(parsed["checksums"].is_object());
            assert!(parsed["checksums"]["test-group"].is_object());
        }

        #[tokio::test]
        async fn test_persist_creates_parent_directory() {
            let temp_dir = tempdir().unwrap();
            let nested_path = temp_dir
                .path()
                .join("nested")
                .join("dir")
                .join("cache.json");

            let cache = FileBasedCache::new(&nested_path).unwrap();
            let test_file = create_test_file(&temp_dir, "test.txt", "content");
            cache
                .update_cache_entry("test-group".to_string(), &test_file)
                .await
                .unwrap();

            cache.persist().await.unwrap();

            assert!(nested_path.parent().unwrap().exists());
        }

        #[tokio::test]
        async fn test_persist_creates_nested_cache_file() {
            let temp_dir = tempdir().unwrap();
            let nested_path = temp_dir
                .path()
                .join("nested")
                .join("dir")
                .join("cache.json");

            let cache = FileBasedCache::new(&nested_path).unwrap();
            let test_file = create_test_file(&temp_dir, "test.txt", "content");
            cache
                .update_cache_entry("test-group".to_string(), &test_file)
                .await
                .unwrap();

            cache.persist().await.unwrap();

            assert!(nested_path.exists());
        }

        #[tokio::test]
        async fn test_persist_invalid_path() {
            // Try to create cache at root (should fail on most systems)
            let invalid_path = PathBuf::from("/");

            let cache = FileBasedCache::new(&invalid_path).unwrap();
            let result = cache.persist().await;

            // Should return an error
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_roundtrip_persistence() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file1 = create_test_file(&temp_dir, "test1.txt", "content1");
            let test_file2 = create_test_file(&temp_dir, "test2.txt", "content2");

            // Create first cache and add data
            {
                let cache = FileBasedCache::new(&cache_path).unwrap();
                cache
                    .update_cache_entry("group1".to_string(), &test_file1)
                    .await
                    .unwrap();
                cache
                    .update_cache_entry("group2".to_string(), &test_file2)
                    .await
                    .unwrap();
                cache.persist().await.unwrap();
            }

            // Create new cache from same file
            let cache2 = FileBasedCache::new(&cache_path).unwrap();

            // Should load previous data
            let status1 = cache2
                .check_file("group1".to_string(), &test_file1)
                .await
                .unwrap();
            let status2 = cache2
                .check_file("group2".to_string(), &test_file2)
                .await
                .unwrap();

            assert_eq!(status1, FileCacheStatus::FileMatches);
            assert_eq!(status2, FileCacheStatus::FileMatches);
        }

        #[tokio::test]
        async fn test_concurrent_access() {
            let (temp_dir, cache_path) = setup_temp_cache();
            let test_file = create_test_file(&temp_dir, "test.txt", "content");

            let cache = FileBasedCache::new(&cache_path).unwrap();

            // Simulate concurrent access
            let cache_clone1 = Arc::clone(&cache.data);
            let cache_clone2 = Arc::clone(&cache.data);
            let file_path1 = test_file.clone();
            let file_path2 = test_file.clone();

            let handle1 = tokio::spawn(async move {
                let checksum = make_checksum(&file_path1).await.unwrap();
                let mut data = cache_clone1.write().await;
                data.checksums
                    .entry("group1".to_string())
                    .or_default()
                    .files
                    .insert(file_path1.display().to_string(), checksum);
            });

            let handle2 = tokio::spawn(async move {
                let checksum = make_checksum(&file_path2).await.unwrap();
                let mut data = cache_clone2.write().await;
                data.checksums
                    .entry("group2".to_string())
                    .or_default()
                    .files
                    .insert(file_path2.display().to_string(), checksum);
            });

            // Wait for both to complete
            handle1.await.unwrap();
            handle2.await.unwrap();

            // Both groups should have the file
            let status1 = cache
                .check_file("group1".to_string(), &test_file)
                .await
                .unwrap();
            let status2 = cache
                .check_file("group2".to_string(), &test_file)
                .await
                .unwrap();

            assert_eq!(status1, FileCacheStatus::FileMatches);
            assert_eq!(status2, FileCacheStatus::FileMatches);
        }

        #[tokio::test]
        async fn test_make_checksum_normal_file() {
            let temp_dir = tempdir().unwrap();
            let file_path = create_test_file(&temp_dir, "test.txt", "test content");

            let checksum = make_checksum(&file_path).await.unwrap();

            assert!(!checksum.is_empty());
            assert_ne!(checksum, "<not exist>");
            assert_ne!(checksum, "<dir>");
        }

        #[tokio::test]
        async fn test_make_checksum_nonexistent_file() {
            let temp_dir = tempdir().unwrap();
            let nonexistent = temp_dir.path().join("nonexistent.txt");

            let checksum = make_checksum(&nonexistent).await.unwrap();

            assert_eq!(checksum, "<not exist>");
        }

        #[tokio::test]
        async fn test_make_checksum_directory() {
            let temp_dir = tempdir().unwrap();

            let checksum = make_checksum(temp_dir.path()).await.unwrap();

            assert_eq!(checksum, "<dir>");
        }

        #[tokio::test]
        async fn test_checksum_consistency() {
            let temp_dir = tempdir().unwrap();
            let file_path = create_test_file(&temp_dir, "test.txt", "consistent content");

            // Multiple checksum calls should return the same result
            let checksum1 = make_checksum(&file_path).await.unwrap();
            let checksum2 = make_checksum(&file_path).await.unwrap();

            assert_eq!(checksum1, checksum2);

            // Different content should produce different checksums
            fs::write(&file_path, "different content").unwrap();
            let checksum3 = make_checksum(&file_path).await.unwrap();

            assert_ne!(checksum1, checksum3);
        }
    }

    mod noop_cache {
        use super::*;

        #[tokio::test]
        async fn test_noop_cache_always_returns_file_changed() {
            let cache = NoOpCache::default();
            let temp_path = PathBuf::from("/tmp/test");

            // Should always return FileChanged
            let status = cache
                .check_file("test".to_string(), &temp_path)
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileChanged);
        }

        #[tokio::test]
        async fn test_noop_cache_update_and_persist_succeed() {
            let cache = NoOpCache::default();
            let temp_path = PathBuf::from("/tmp/test");

            // Should succeed but do nothing
            cache
                .update_cache_entry("test".to_string(), &temp_path)
                .await
                .unwrap();
            cache.persist().await.unwrap();
        }

        #[tokio::test]
        async fn test_noop_cache_still_returns_file_changed_after_update() {
            let cache = NoOpCache::default();
            let temp_path = PathBuf::from("/tmp/test");

            // Update cache entry (should do nothing)
            cache
                .update_cache_entry("test".to_string(), &temp_path)
                .await
                .unwrap();

            // Still returns FileChanged after update
            let status = cache
                .check_file("test".to_string(), &temp_path)
                .await
                .unwrap();
            assert_eq!(status, FileCacheStatus::FileChanged);
        }
    }
}
