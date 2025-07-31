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
/// Returns the appropriate configuration directory for the current platform:
/// - macOS: `~/Library/Application Support`
/// - Linux: `~/.config` (following XDG Base Directory Specification)
/// - Windows: `%APPDATA%`
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
        #[cfg(target_os = "macos")]
        {
            Some(home.join("Library").join("Application Support"))
        }
        #[cfg(target_os = "linux")]
        {
            // Check for XDG_CONFIG_HOME first, fallback to ~/.config
            if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
                Some(PathBuf::from(xdg_config))
            } else {
                Some(home.join(".config"))
            }
        }
        #[cfg(target_os = "windows")]
        {
            // On Windows, use APPDATA
            std::env::var("APPDATA")
                .map(PathBuf::from)
                .ok()
                .or_else(|| Some(home.join("AppData").join("Roaming")))
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            // Fallback for other platforms
            Some(home.join(".config"))
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_returns_some() {
        // This test assumes that the home directory is available
        // in the test environment
        let home_dir = home();
        assert!(home_dir.is_some(), "Expected home directory to be available");
        
        if let Some(home) = home_dir {
            assert!(home.is_absolute(), "Home directory should be an absolute path");
        }
    }

    #[test]
    fn test_config_returns_some() {
        let config_dir = config();
        assert!(config_dir.is_some(), "Expected config directory to be available");
        
        if let Some(config) = config_dir {
            assert!(config.is_absolute(), "Config directory should be an absolute path");
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_config_macos_path() {
        if let Some(config_dir) = config() {
            let path_str = config_dir.to_string_lossy();
            assert!(
                path_str.contains("Library/Application Support"),
                "macOS config directory should contain 'Library/Application Support', got: {}",
                path_str
            );
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_config_linux_path() {
        use std::env;
        // Test without XDG_CONFIG_HOME set
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        env::remove_var("XDG_CONFIG_HOME");
        
        if let Some(config_dir) = config() {
            let path_str = config_dir.to_string_lossy();
            assert!(
                path_str.ends_with("/.config"),
                "Linux config directory should end with '/.config', got: {}",
                path_str
            );
        }
        
        // Restore original XDG_CONFIG_HOME if it existed
        if let Some(xdg_config) = original_xdg {
            env::set_var("XDG_CONFIG_HOME", xdg_config);
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_config_linux_xdg_config_home() {
        use std::env;
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let test_xdg_path = "/tmp/test_xdg_config";
        
        env::set_var("XDG_CONFIG_HOME", test_xdg_path);
        
        if let Some(config_dir) = config() {
            assert_eq!(
                config_dir,
                PathBuf::from(test_xdg_path),
                "Should use XDG_CONFIG_HOME when set"
            );
        }
        
        // Restore original XDG_CONFIG_HOME
        match original_xdg {
            Some(xdg_config) => env::set_var("XDG_CONFIG_HOME", xdg_config),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_config_windows_path() {
        if let Some(config_dir) = config() {
            let path_str = config_dir.to_string_lossy();
            assert!(
                path_str.contains("AppData") || path_str.contains("APPDATA"),
                "Windows config directory should contain 'AppData', got: {}",
                path_str
            );
        }
    }

    #[test]
    fn test_config_dir_consistency() {
        // Test that multiple calls return the same result
        let config1 = config();
        let config2 = config();
        assert_eq!(config1, config2, "config should return consistent results");
    }

    #[test]
    fn test_home_dir_consistency() {
        // Test that multiple calls return the same result
        let home1 = home();
        let home2 = home();
        assert_eq!(home1, home2, "home should return consistent results");
    }
}
