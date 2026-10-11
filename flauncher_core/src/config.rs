use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Clone)]
pub struct FactorioConfig {
    pub bin_path: PathBuf,
    pub data_path: PathBuf
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
    let content = format!(
        "bin_path = \"{}\"\ndata_path = \"{}\"\n",
        config.bin_path.to_string_lossy(),
        config.data_path.to_string_lossy()
    );

    fs::write(config_file, content).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_config() -> Option<FactorioConfig> {
    let config_file = get_app_config_dir().join("paths.toml");
    let content = fs::read_to_string(config_file).ok()?;

    let mut bin_path = None;
    let mut data_path = None;

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("bin_path = ") {
            bin_path = Some(PathBuf::from(val.trim_matches('"')));
        } else if let Some(val) = line.strip_prefix("data_path = ") {
            data_path = Some(PathBuf::from(val.trim_matches('"')));
        }
    }

    Some(FactorioConfig {
        bin_path: bin_path?,
        data_path: data_path?,
    })
}


pub fn find_factorio_installation<P: AsRef<Path>>(install_root: P) -> Result<FactorioConfig, String> {
    let root = install_root.as_ref();

    #[cfg(target_os = "macos")]
    let (bin_rel, data_rel) = (
        PathBuf::from("factorio.app/Contents/MacOS/factorio"),
        PathBuf::from("factorio.app/Contents/data"),
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

    Ok(FactorioConfig { bin_path, data_path })
}