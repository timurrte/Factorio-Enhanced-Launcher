use std::fs;
use std::path::PathBuf;
use std::process::Command;

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

#[cfg(target_os = "macos")]
const FACTORIO_BIN: &str = "/Applications/factorio.app/Contents/MacOS/factorio";
#[cfg(target_os = "macos")]
const FACTORIO_DATA: &str = "/Applications/factorio.app/Contents/data";

#[cfg(target_os = "windows")]
const FACTORIO_BIN: &str = r"C:\Program Files\Factorio\bin\x64\factorio.exe";
#[cfg(target_os = "windows")]
const FACTORIO_DATA: &str = r"C:\Program Files\Factorio\data";

#[cfg(target_os = "linux")]
const FACTORIO_BIN: &str = "/usr/bin/factorio";
#[cfg(target_os = "linux")]
const FACTORIO_DATA: &str = "/usr/share/factorio/data";

pub fn find_factorio_installation<P: AsRef<Path>>(install_root: P) -> Result<FactorioConfig, String> {
    let root = install_root.as_ref();

    #[cfg(target_os = "macos")]
    let (bin_rel, data_rel) = (
        PathBuf::from("factorio.app/Contents/MacOS/factorio"),
        PathBuf::from("factorio.app/Contents/data"),
    );

    #[cfg(target_os = "windows")]
    let (bin_rel, data_rel) = (
        PathBuf::from("bin/x64/factorio.exe"),
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
        Ok(iter) => iter
            .filter_map(|entry| {
                let entry = entry.ok()?;
                if entry.file_type().ok()?.is_dir() {
                    entry.file_name().into_string().ok()
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => vec![],
    }
}

pub fn create_profile_dir(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Profile name cannot be empty.".into());
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
        FACTORIO_DATA,
        profile_path.to_string_lossy()
    );

    fs::write(config_ini_path, config_content).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn launch_game(name: &str) -> Result<(), String> {
    let profiles_dir = get_profiles_dir();
    let config_ini_path = profiles_dir.join(name).join("config/config.ini");

    if !config_ini_path.exists() {
        create_profile_dir(name)?;
    }

    Command::new(FACTORIO_BIN)
        .arg("--config")
        .arg(&config_ini_path)
        .spawn()
        .map_err(|e| format!("Failed to launch Factorio: {}", e))?;

    Ok(())
}