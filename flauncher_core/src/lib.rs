use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod config;
pub use crate::config::*;

mod update;
pub use crate::update::*;

pub fn get_profiles_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());

    #[cfg(not(target_os = "windows"))]
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    PathBuf::from(home).join("Documents").join("FactorioProfiles")
}

pub fn get_profiles() -> Vec<String> {
    let dir = get_profiles_dir();
    if !dir.exists() {
        return vec![];
    }
    let entries = fs::read_dir(dir);
    match entries {
        Ok(iter) => {
            let mut list: Vec<String> = iter
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    if entry.file_type().ok()?.is_dir() {
                        let name = entry.file_name().into_string().ok()?;
                        if !name.starts_with('.') {
                            Some(name)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            list.sort_by_key(|a| a.to_lowercase());
            list
        }
        Err(_) => vec![],
    }
}

pub fn create_profile_dir(name: &str, config: &FactorioConfig) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Profile name cannot be empty.".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err("Profile name contains invalid characters.".into());
    }
    if name == "." || name == ".." {
        return Err("Invalid profile name.".into());
    }

    let profiles_dir = get_profiles_dir();
    let profile_path = profiles_dir.join(name);
    let config_dir = profile_path.join("config");
    let mods_dir = profile_path.join("mods");
    let saves_dir = profile_path.join("saves");

    fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&saves_dir).map_err(|e| e.to_string())?;

    let config_ini_path = config_dir.join("config.ini");
    let config_content = format!(
        r#"; version=13
; This is a generated configuration file for Factorio isolated profiles

[path]
read-data={}
write-data={}

[general]
locale=auto

[other]
[interface]
[input]
[controls]
[controller]
[sound]
[map-view]
[debug]
[multiplayer-lobby]
[graphics]
"#,
        config.data_path.to_string_lossy(),
        profile_path.to_string_lossy()
    );

    fs::write(config_ini_path, config_content).map_err(|e| e.to_string())?;

    Ok(())
}

/// Rename an existing profile folder in a specific directory and update internal configuration files
pub fn rename_profile_in_dir(profiles_dir: &Path, old_name: &str, new_name: &str) -> Result<(), String> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();

    if new_name.is_empty() {
        return Err("Profile name cannot be empty.".into());
    }
    if new_name.contains('/') || new_name.contains('\\') || new_name.contains('\0') {
        return Err("Profile name contains invalid characters.".into());
    }
    if new_name == "." || new_name == ".." {
        return Err("Invalid profile name.".into());
    }
    if old_name == new_name {
        return Ok(());
    }

    let old_path = profiles_dir.join(old_name);
    let new_path = profiles_dir.join(new_name);

    if !old_path.is_dir() {
        return Err(format!("Profile '{}' does not exist.", old_name));
    }
    if new_path.exists() {
        return Err(format!("A profile named '{}' already exists.", new_name));
    }

    fs::rename(&old_path, &new_path).map_err(|e| {
        format!(
            "Failed to rename profile directory from '{}' to '{}': {}",
            old_name, new_name, e
        )
    })?;

    // Synchronize write-data in config.ini with the new path
    let config_ini_path = new_path.join("config").join("config.ini");
    if config_ini_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_ini_path) {
            let mut new_lines = Vec::new();
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("write-data=") || trimmed.starts_with("write-data =") {
                    new_lines.push(format!("write-data={}", new_path.to_string_lossy()));
                } else {
                    new_lines.push(line.to_string());
                }
            }
            let _ = fs::write(&config_ini_path, new_lines.join("\n"));
        }
    }

    Ok(())
}

/// Rename an existing profile folder and update internal configuration files
pub fn rename_profile(old_name: &str, new_name: &str) -> Result<(), String> {
    rename_profile_in_dir(&get_profiles_dir(), old_name, new_name)
}

/// Synchronize read-data and write-data paths in profile config.ini within a specific profiles directory
pub fn sync_profile_config_ini_in_dir(
    profiles_dir: &Path,
    profile_name: &str,
    data_path: &Path,
    profile_path: &Path,
) -> Result<(), String> {
    let config_ini_path = profiles_dir.join(profile_name).join("config").join("config.ini");
    if !config_ini_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&config_ini_path).map_err(|e| e.to_string())?;
    let mut new_lines = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("read-data=") || trimmed.starts_with("read-data =") {
            new_lines.push(format!("read-data={}", data_path.to_string_lossy()));
        } else if trimmed.starts_with("write-data=") || trimmed.starts_with("write-data =") {
            new_lines.push(format!("write-data={}", profile_path.to_string_lossy()));
        } else {
            new_lines.push(line.to_string());
        }
    }

    fs::write(&config_ini_path, new_lines.join("\n")).map_err(|e| e.to_string())?;
    Ok(())
}

/// Synchronize read-data and write-data paths in profile config.ini
pub fn sync_profile_config_ini(
    profile_name: &str,
    data_path: &Path,
    profile_path: &Path,
) -> Result<(), String> {
    sync_profile_config_ini_in_dir(&get_profiles_dir(), profile_name, data_path, profile_path)
}

pub fn launch_game(name: &str, config: &FactorioConfig) -> Result<(), String> {
    let profiles_dir = get_profiles_dir();
    let profile_path = profiles_dir.join(name);
    let config_ini_path = profile_path.join("config").join("config.ini");

    if !config_ini_path.exists() {
        create_profile_dir(name, config)?;
    }

    // Check if the profile overrides the global bin_path
    let profile_cfg = get_profile_config(name);
    let (bin_to_launch, data_to_use) = if let Some(ref custom_bin) = profile_cfg.bin_path {
        let data = infer_data_path(custom_bin).unwrap_or_else(|| config.data_path.clone());
        (custom_bin.clone(), data)
    } else {
        (config.bin_path.clone(), config.data_path.clone())
    };

    if !bin_to_launch.exists() {
        return Err(format!(
            "Factorio executable not found at '{}'",
            bin_to_launch.display()
        ));
    }

    // Keep config.ini in sync with the active data path and profile location
    let _ = sync_profile_config_ini(name, &data_to_use, &profile_path);

    Command::new(&bin_to_launch)
        .arg("--config")
        .arg(&config_ini_path)
        .spawn()
        .map_err(|e| {
            format!(
                "Failed to launch Factorio at '{}': {} (os error {})",
                bin_to_launch.display(),
                e,
                e.raw_os_error().unwrap_or(0)
            )
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rename_profile_validation() {
        assert!(rename_profile("old", "").is_err());
        assert!(rename_profile("old", "   ").is_err());
        assert!(rename_profile("old", "invalid/name").is_err());
        assert!(rename_profile("old", "invalid\\name").is_err());
        assert!(rename_profile("old", "..").is_err());
        assert!(rename_profile("old", ".").is_err());
        assert!(rename_profile("same", "same").is_ok());
    }

    #[test]
    fn test_profile_config_serialization() {
        let tmp_dir = std::env::temp_dir().join(format!("test_prof_cfg_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let cfg = ProfileConfig {
            bin_path: Some(PathBuf::from("/custom/path/factorio")),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let loaded: ProfileConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.bin_path, Some(PathBuf::from("/custom/path/factorio")));

        let empty_cfg: ProfileConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(empty_cfg.bin_path, None);

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_infer_data_path() {
        #[cfg(target_os = "macos")]
        {
            let bin = PathBuf::from("/Applications/Factorio.app/Contents/MacOS/factorio");
            // If the directory doesn't actually exist on disk, infer_data_path returns None
            let _ = infer_data_path(&bin);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let bin = PathBuf::from("/opt/factorio/bin/x64/factorio");
            let _ = infer_data_path(&bin);
        }
    }

    #[test]
    fn test_rename_profile_and_custom_bin_persistence() {
        let tmp_root = std::env::temp_dir().join(format!("test_profiles_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_root);
        fs::create_dir_all(&tmp_root).unwrap();

        // 1. Create a dummy profile directory
        let old_name = "OriginalProfile";
        let old_dir = tmp_root.join(old_name);
        let config_dir = old_dir.join("config");
        fs::create_dir_all(&config_dir).unwrap();

        let initial_config_ini = format!(
            "[path]\nread-data=/games/factorio/data\nwrite-data={}\n",
            old_dir.to_string_lossy()
        );
        fs::write(config_dir.join("config.ini"), initial_config_ini).unwrap();

        // 2. Set custom bin_path for old_name
        let custom_bin = PathBuf::from("/custom/bin/factorio");
        set_profile_bin_path_in_dir(&tmp_root, old_name, Some(custom_bin.clone())).unwrap();
        assert_eq!(
            get_profile_bin_path_in_dir(&tmp_root, old_name),
            Some(custom_bin.clone())
        );

        // 3. Rename profile
        let new_name = "RenamedProfile";
        rename_profile_in_dir(&tmp_root, old_name, new_name).unwrap();

        let new_dir = tmp_root.join(new_name);
        assert!(!old_dir.exists());
        assert!(new_dir.exists());

        // 4. Verify config.ini write-data updated
        let updated_ini = fs::read_to_string(new_dir.join("config").join("config.ini")).unwrap();
        assert!(updated_ini.contains(&format!("write-data={}", new_dir.to_string_lossy())));

        // 5. Verify custom bin_path persisted in renamed profile
        assert_eq!(
            get_profile_bin_path_in_dir(&tmp_root, new_name),
            Some(custom_bin)
        );

        // 6. Reset to global default
        set_profile_bin_path_in_dir(&tmp_root, new_name, None).unwrap();
        assert_eq!(get_profile_bin_path_in_dir(&tmp_root, new_name), None);

        let _ = fs::remove_dir_all(&tmp_root);
    }
}