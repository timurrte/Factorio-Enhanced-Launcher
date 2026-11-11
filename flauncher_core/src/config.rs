use std::{fs, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProfileConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin_path: Option<PathBuf>,
}

pub fn get_profile_config_in_dir(profiles_dir: &Path, profile_name: &str) -> ProfileConfig {
    let path = profiles_dir.join(profile_name).join("profile.json");
    if let Ok(content) = fs::read_to_string(path)
        && let Ok(cfg) = serde_json::from_str::<ProfileConfig>(&content)
    {
        return cfg;
    }
    ProfileConfig::default()
}

pub fn save_profile_config_in_dir(profiles_dir: &Path, profile_name: &str, config: &ProfileConfig) -> Result<(), String> {
    let profile_dir = profiles_dir.join(profile_name);
    if !profile_dir.is_dir() {
        return Err(format!("Profile '{}' does not exist.", profile_name));
    }
    let config_path = profile_dir.join("profile.json");
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(config_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_profile_bin_path_in_dir(profiles_dir: &Path, profile_name: &str) -> Option<PathBuf> {
    get_profile_config_in_dir(profiles_dir, profile_name).bin_path
}

pub fn set_profile_bin_path_in_dir(profiles_dir: &Path, profile_name: &str, bin_path: Option<PathBuf>) -> Result<(), String> {
    let mut cfg = get_profile_config_in_dir(profiles_dir, profile_name);
    cfg.bin_path = bin_path;
    save_profile_config_in_dir(profiles_dir, profile_name, &cfg)
}

pub fn get_profile_config(profile_name: &str) -> ProfileConfig {
    get_profile_config_in_dir(&crate::get_profiles_dir(), profile_name)
}

pub fn save_profile_config(profile_name: &str, config: &ProfileConfig) -> Result<(), String> {
    save_profile_config_in_dir(&crate::get_profiles_dir(), profile_name, config)
}

pub fn get_profile_bin_path(profile_name: &str) -> Option<PathBuf> {
    get_profile_bin_path_in_dir(&crate::get_profiles_dir(), profile_name)
}

pub fn set_profile_bin_path(profile_name: &str, bin_path: Option<PathBuf>) -> Result<(), String> {
    set_profile_bin_path_in_dir(&crate::get_profiles_dir(), profile_name, bin_path)
}

/// Infer the Factorio data directory given an executable path
pub fn infer_data_path(bin_path: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        if let Some(macos_dir) = bin_path.parent()
            && let Some(contents_dir) = macos_dir.parent()
        {
            let candidate = contents_dir.join("data");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    if let Some(bin_dir) = bin_path.parent() {
        if let Some(bin_parent) = bin_dir.parent()
            && let Some(root) = bin_parent.parent()
        {
            let candidate = root.join("data");
            if candidate.exists() {
                return Some(candidate);
            }
        }
        let candidate = bin_dir.join("data");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

/// Resolve path to the actual Factorio executable, automatically extracting inside .app or directories if applicable
pub fn resolve_executable_path(path: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if path.extension().and_then(|e| e.to_str()) == Some("app") {
            let app_bin = path.join("Contents/MacOS/factorio");
            if app_bin.exists() {
                return app_bin;
            }
        }
        if path.is_dir() {
            let app_bin = path.join("Factorio.app/Contents/MacOS/factorio");
            if app_bin.exists() {
                return app_bin;
            }
            let contents_bin = path.join("Contents/MacOS/factorio");
            if contents_bin.exists() {
                return contents_bin;
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if path.is_dir() {
            let win_bin = path.join("bin/x64/factorio.exe");
            if win_bin.exists() {
                return win_bin;
            }
            let win_bin_direct = path.join("bin\\x64\\factorio.exe");
            if win_bin_direct.exists() {
                return win_bin_direct;
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if path.is_dir() {
            let lin_bin = path.join("bin/x64/factorio");
            if lin_bin.exists() {
                return lin_bin;
            }
        }
    }

    path.to_path_buf()
}

/// Query the version of Factorio for a given binary
pub fn get_version_for_binary(bin_path: &Path) -> Option<String> {
    if let Some(data_dir) = infer_data_path(bin_path) {
        let info_path = data_dir.join("base").join("info.json");
        if let Ok(content) = fs::read_to_string(&info_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(v) = val.get("version").and_then(|v| v.as_str()) {
                    return Some(v.to_string());
                }
            }
        }
    }

    if let Ok(output) = std::process::Command::new(bin_path).arg("--version").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix("Factorio ") {
                if let Some(ver) = rest.split_whitespace().next() {
                    return Some(ver.to_string());
                }
            }
        }
    }

    None
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FactorioConfig {
    pub bin_path: PathBuf,
    pub data_path: PathBuf,
    pub username: Option<String>,
    pub token: Option<String>,
    pub preferred_branch: Option<String>, // "stable" or "experimental"
    pub preferred_build: Option<String>,  // "alpha" or "expansion"
}

impl FactorioConfig {
    pub fn install_dir(&self) -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            if let Some(contents) = self.data_path.parent() {
                if let Some(app_bundle) = contents.parent() {
                    if app_bundle.extension().and_then(|e| e.to_str()) == Some("app") {
                        return app_bundle.to_path_buf();
                    }
                }
            }
            if let Some(macos) = self.bin_path.parent() {
                if let Some(contents) = macos.parent() {
                    if let Some(app_bundle) = contents.parent() {
                        if app_bundle.extension().and_then(|e| e.to_str()) == Some("app") {
                            return app_bundle.to_path_buf();
                        }
                    }
                }
            }
            self.bin_path.parent().unwrap_or(&self.bin_path).to_path_buf()
        }
        #[cfg(not(target_os = "macos"))]
        {
            if let Some(data_parent) = self.data_path.parent() {
                if data_parent.join("bin").exists() {
                    return data_parent.to_path_buf();
                }
            }
            if let Some(bin_dir) = self.bin_path.parent() {
                if let Some(bin_parent) = bin_dir.parent() {
                    if let Some(root) = bin_parent.parent() {
                        if root.join("data").exists() {
                            return root.to_path_buf();
                        }
                    }
                }
            }
            self.data_path.parent().unwrap_or(&self.data_path).to_path_buf()
        }
    }
}

pub fn get_app_config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());

    #[cfg(target_os = "macos")]
    let base = std::env::var("HOME")
        .map(|h| format!("{}/Library/Application Support", h))
        .unwrap_or_else(|_| ".".to_string());

    #[cfg(target_os = "linux")]
    let base = std::env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", std::env::var("HOME").unwrap_or_else(|_| ".".to_string())));

    PathBuf::from(base).join("FactorioProfileManager")
}

pub fn save_config(config: &FactorioConfig) -> Result<(), String> {
    let config_dir = get_app_config_dir();
    fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;

    let config_file = config_dir.join("paths.toml");
    let mut content = format!(
        "bin_path = \"{}\"\ndata_path = \"{}\"\n",
        config.bin_path.to_string_lossy(),
        config.data_path.to_string_lossy()
    );

    if let Some(ref u) = config.username {
        content.push_str(&format!("username = \"{}\"\n", u));
    }
    if let Some(ref t) = config.token {
        content.push_str(&format!("token = \"{}\"\n", t));
    }
    if let Some(ref b) = config.preferred_branch {
        content.push_str(&format!("preferred_branch = \"{}\"\n", b));
    }
    if let Some(ref b) = config.preferred_build {
        content.push_str(&format!("preferred_build = \"{}\"\n", b));
    }

    fs::write(config_file, content).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_config() -> Option<FactorioConfig> {
    let config_file = get_app_config_dir().join("paths.toml");
    let content = fs::read_to_string(config_file).ok()?;

    let mut bin_path = None;
    let mut data_path = None;
    let mut username = None;
    let mut token = None;
    let mut preferred_branch = None;
    let mut preferred_build = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("bin_path = ") {
            bin_path = Some(PathBuf::from(val.trim_matches('"')));
        } else if let Some(val) = trimmed.strip_prefix("data_path = ") {
            data_path = Some(PathBuf::from(val.trim_matches('"')));
        } else if let Some(val) = trimmed.strip_prefix("username = ") {
            let u = val.trim_matches('"');
            if !u.is_empty() {
                username = Some(u.to_string());
            }
        } else if let Some(val) = trimmed.strip_prefix("token = ") {
            let t = val.trim_matches('"');
            if !t.is_empty() {
                token = Some(t.to_string());
            }
        } else if let Some(val) = trimmed.strip_prefix("preferred_branch = ") {
            let b = val.trim_matches('"');
            if !b.is_empty() {
                preferred_branch = Some(b.to_string());
            }
        } else if let Some(val) = trimmed.strip_prefix("preferred_build = ") {
            let b = val.trim_matches('"');
            if !b.is_empty() {
                preferred_build = Some(b.to_string());
            }
        }
    }

    Some(FactorioConfig {
        bin_path: bin_path?,
        data_path: data_path?,
        username,
        token,
        preferred_branch,
        preferred_build,
    })
}

pub fn get_installed_version(config: &FactorioConfig) -> Option<String> {
    let info_path = config.data_path.join("base").join("info.json");
    if let Ok(content) = fs::read_to_string(&info_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(v) = val.get("version").and_then(|v| v.as_str()) {
                return Some(v.to_string());
            }
        }
    }

    if let Ok(output) = std::process::Command::new(&config.bin_path).arg("--version").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix("Factorio ") {
                if let Some(ver) = rest.split_whitespace().next() {
                    return Some(ver.to_string());
                }
            }
        }
    }

    None
}

pub fn has_space_age(config: &FactorioConfig) -> bool {
    config.data_path.join("space-age").is_dir()
}

pub fn find_factorio_installation<P: AsRef<Path>>(install_root: P) -> Result<FactorioConfig, String> {
    let root = install_root.as_ref();

    #[cfg(target_os = "macos")]
    let (bin_rel, data_rel) = (
        PathBuf::from("Contents/MacOS/factorio"),
        PathBuf::from("Contents/data"),
    );

    #[cfg(target_os = "windows")]
    let (bin_rel, data_rel) = (
        PathBuf::from("bin\\x64\\factorio.exe"),
        PathBuf::from("data"),
    );

    #[cfg(target_os = "linux")]
    let (bin_rel, data_rel) = (
        PathBuf::from("bin/x64/factorio"),
        PathBuf::from("data"),
    );

    let bin_path = root.join(&bin_rel);
    let data_path = root.join(&data_rel);

    if !bin_path.exists() {
        return Err(format!(
            "Could not find Factorio executable at expected location: {}",
            bin_path.display()
        ));
    }

    if !data_path.exists() {
        return Err(format!(
            "Could not find Factorio data directory at expected location: {}",
            data_path.display()
        ));
    }

    Ok(FactorioConfig {
        bin_path,
        data_path,
        username: None,
        token: None,
        preferred_branch: None,
        preferred_build: None,
    })
}
