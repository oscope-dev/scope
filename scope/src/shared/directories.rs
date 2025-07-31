//! Cross-platform directory utilities for the scope application.
//!
//! This module provides functions for discovering standard directories
//! across different operating systems in a consistent way.

use std::path::PathBuf;

/// Get the user's home directory.
///
/// # Returns
///
/// Returns `Some(PathBuf)` if the home directory can be determined,
/// `None` otherwise.
///
/// # Examples
///
/// ```
/// # use dev_scope::shared::directories::home;
/// if let Some(home) = home() {
///     println!("Home directory: {}", home.display());
/// }
/// ```
pub fn home() -> Option<PathBuf> {
    std::env::home_dir()
}

/// Get the user's configuration directory.
///
/// Returns the configuration directory following the XDG Base Directory Specification:
/// - `$XDG_CONFIG_HOME` if set, otherwise `~/.config`
///
/// This follows the XDG Base Directory Specification for Unix-like systems,
/// which provides a consistent location across different Unix variants.
///
/// # Returns
///
/// Returns `Some(PathBuf)` if the configuration directory can be determined,
/// `None` otherwise.
///
/// # Examples
///
/// ```
/// # use dev_scope::shared::directories::config;
/// if let Some(config_dir) = config() {
///     println!("Config directory: {}", config_dir.display());
/// }
/// ```
pub fn config() -> Option<PathBuf> {
    if let Some(home) = home() {
        // Use XDG Base Directory Specification
        // Check for XDG_CONFIG_HOME first, fallback to ~/.config
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            Some(PathBuf::from(xdg_config))
        } else {
            Some(home.join(".config"))
        }
    } else {
        None
    }
}

/// Get the user's cache directory.
///
/// Returns the cache directory following the XDG Base Directory Specification:
/// - `$XDG_CACHE_HOME` if set, otherwise `~/.cache`
///
/// This follows the XDG Base Directory Specification for Unix-like systems.
/// The cache directory is intended for user-specific non-essential data files.
///
/// # Returns
///
/// Returns `Some(PathBuf)` if the cache directory can be determined,
/// `None` otherwise.
///
/// # Examples
///
/// ```
/// # use dev_scope::shared::directories::cache;
/// if let Some(cache_dir) = cache() {
///     println!("Cache directory: {}", cache_dir.display());
/// }
/// ```
pub fn cache() -> Option<PathBuf> {
    if let Some(home) = home() {
        // Use XDG Base Directory Specification
        // Check for XDG_CACHE_HOME first, fallback to ~/.cache
        if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
            Some(PathBuf::from(xdg_cache))
        } else {
            Some(home.join(".cache"))
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod home {
        use super::*;

        #[test]
        fn consistency() {
            // Test that multiple calls return the same result
            let home1 = home();
            let home2 = home();
            assert_eq!(home1, home2, "home should return consistent results");
        }
    }

    mod config {
        use super::*;

        #[test]
        fn unix_path() {
            use std::env;
            // Test without XDG_CONFIG_HOME set
            let original_xdg = env::var("XDG_CONFIG_HOME").ok();
            env::remove_var("XDG_CONFIG_HOME");

            let config_dir = config().unwrap();
            let path_str = config_dir.to_string_lossy();
            assert!(
                path_str.ends_with("/.config"),
                "Config directory should end with '/.config', got: {}",
                path_str
            );

            // Restore original XDG_CONFIG_HOME if it existed
            if let Some(xdg_config) = original_xdg {
                env::set_var("XDG_CONFIG_HOME", xdg_config);
            }
        }

        #[test]
        fn xdg_config_home() {
            use std::env;
            let original_xdg = env::var("XDG_CONFIG_HOME").ok();
            let test_xdg_path = "/tmp/test_xdg_config";

            env::set_var("XDG_CONFIG_HOME", test_xdg_path);

            let config_dir = config().unwrap();
            assert_eq!(
                config_dir,
                PathBuf::from(test_xdg_path),
                "Should use XDG_CONFIG_HOME when set"
            );

            // Restore original XDG_CONFIG_HOME
            match original_xdg {
                Some(xdg_config) => env::set_var("XDG_CONFIG_HOME", xdg_config),
                None => env::remove_var("XDG_CONFIG_HOME"),
            }
        }

        #[test]
        fn consistency() {
            // Test that multiple calls return the same result
            let config1 = config();
            let config2 = config();
            assert_eq!(config1, config2, "config should return consistent results");
        }
    }

    mod cache {
        use super::*;

        #[test]
        fn unix_path() {
            use std::env;
            // Test without XDG_CACHE_HOME set
            let original_xdg = env::var("XDG_CACHE_HOME").ok();
            env::remove_var("XDG_CACHE_HOME");

            let cache_dir = cache().unwrap();
            let path_str = cache_dir.to_string_lossy();
            assert!(
                path_str.ends_with("/.cache"),
                "Cache directory should end with '/.cache', got: {}",
                path_str
            );

            // Restore original XDG_CACHE_HOME if it existed
            if let Some(xdg_cache) = original_xdg {
                env::set_var("XDG_CACHE_HOME", xdg_cache);
            }
        }

        #[test]
        fn xdg_cache_home() {
            use std::env;
            let original_xdg = env::var("XDG_CACHE_HOME").ok();
            let test_xdg_path = "/tmp/test_xdg_cache";

            env::set_var("XDG_CACHE_HOME", test_xdg_path);

            let cache_dir = cache().unwrap();
            assert_eq!(
                cache_dir,
                PathBuf::from(test_xdg_path),
                "Should use XDG_CACHE_HOME when set"
            );

            // Restore original XDG_CACHE_HOME
            match original_xdg {
                Some(xdg_cache) => env::set_var("XDG_CACHE_HOME", xdg_cache),
                None => env::remove_var("XDG_CACHE_HOME"),
            }
        }

        #[test]
        fn consistency() {
            use std::env;
            // Save and clean any XDG environment variables that might affect the test
            let original_xdg_cache = env::var("XDG_CACHE_HOME").ok();
            env::remove_var("XDG_CACHE_HOME");

            // Test that multiple calls return the same result
            let cache1 = cache();
            let cache2 = cache();
            assert_eq!(cache1, cache2, "cache should return consistent results");

            // Restore original environment
            if let Some(xdg_cache) = original_xdg_cache {
                env::set_var("XDG_CACHE_HOME", xdg_cache);
            }
        }
    }
}
