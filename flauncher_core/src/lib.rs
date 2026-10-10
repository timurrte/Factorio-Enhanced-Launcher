use std::fs;
use std::path::PathBuf;
use std::process::Command;

const FACTORIO_BIN: &str = "/Applications/factorio.app/Contents/MacOS/factorio";
const FACTORIO_DATA: &str = "/Applications/factorio.app/Contents/data";

pub fn get_profiles_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("Documents/FactorioProfiles")
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