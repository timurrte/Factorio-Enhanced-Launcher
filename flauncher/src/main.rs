use dioxus::prelude::*;
use flauncher_core::{
    create_profile_dir, fetch_latest_releases, find_factorio_installation, get_installed_version,
    get_profile_bin_path, get_profiles, get_profiles_dir, get_version_for_binary, has_space_age,
    infer_data_path, launch_game, load_config, perform_update, rename_profile,
    resolve_executable_path, save_config, set_profile_bin_path, BuildType, LatestReleases,
    UpdateStage,
};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Launcher,
    Configuration,
}

fn main() {
    LaunchBuilder::new()
        .with_cfg(
            dioxus::desktop::Config::new().with_window(
                dioxus::desktop::WindowBuilder::new()
                    .with_title("Factorio Enhanced Launcher")
                    .with_inner_size(dioxus::desktop::LogicalSize::new(760.0, 900.0)),
            ),
        )
        .launch(App);
}

#[component]
fn App() -> Element {
    let mut config = use_signal(load_config);
    let mut setup_error = use_signal(String::new);

    let mut active_tab = use_signal(|| ActiveTab::Launcher);

    let mut new_profile_name = use_signal(String::new);
    let mut profiles = use_signal(get_profiles);
    let mut status_message = use_signal(String::new);

    // Profile rename state
    let mut renaming_profile = use_signal(|| Option::<String>::None);
    let mut rename_input = use_signal(String::new);

    // Profile custom bin_path state (Launcher tab)
    let mut editing_profile_bin = use_signal(|| Option::<String>::None);
    let mut profile_bin_input = use_signal(String::new);
    let mut profile_bin_status = use_signal(String::new);

    // Profile custom bin_path state (Configuration submenu)
    let mut config_editing_profile_bin = use_signal(|| Option::<String>::None);
    let mut config_profile_bin_input = use_signal(String::new);
    let mut config_profile_bin_status = use_signal(String::new);

    // Global configuration submenu state
    let mut global_bin_input = use_signal(|| {
        config()
            .as_ref()
            .map(|c| c.bin_path.to_string_lossy().to_string())
            .unwrap_or_default()
    });
    let mut global_data_input = use_signal(|| {
        config()
            .as_ref()
            .map(|c| c.data_path.to_string_lossy().to_string())
            .unwrap_or_default()
    });
    let mut config_submenu_message = use_signal(String::new);

    // --- AUTOUPDATE STATE ---
    let mut installed_version = use_signal(|| {
        config().as_ref().and_then(get_installed_version)
    });
    let mut latest_releases = use_signal(|| Option::<LatestReleases>::None);
    let mut is_checking_releases = use_signal(|| false);
    let mut release_check_error = use_signal(String::new);

    let mut username_input = use_signal(|| {
        config()
            .as_ref()
            .and_then(|c| c.username.clone())
            .unwrap_or_default()
    });
    let mut token_input = use_signal(|| {
        config()
            .as_ref()
            .and_then(|c| c.token.clone())
            .unwrap_or_default()
    });
    let mut show_token = use_signal(|| false);

    let mut selected_build = use_signal(|| {
        if let Some(ref c) = config() {
            if let Some(ref b) = c.preferred_build {
                if b == "expansion" {
                    return BuildType::Expansion;
                } else if b == "alpha" {
                    return BuildType::Alpha;
                }
            }
            if has_space_age(c) {
                return BuildType::Expansion;
            }
        }
        BuildType::Alpha
    });

    let mut selected_branch = use_signal(|| {
        config()
            .as_ref()
            .and_then(|c| c.preferred_branch.clone())
            .unwrap_or_else(|| "stable".to_string())
    });

    let mut custom_version_input = use_signal(String::new);
    let mut update_stage = use_signal(|| UpdateStage::Idle);
    let mut is_updating = use_signal(|| false);
    let mut show_credentials_panel = use_signal(|| false);

    let mut refresh_profiles = move || {
        profiles.set(get_profiles());
    };

    // Auto-check releases on startup if config is present
    use_effect(move || {
        if config().is_some() && latest_releases().is_none() && !is_checking_releases() {
            is_checking_releases.set(true);
            release_check_error.set(String::new());
            spawn(async move {
                match fetch_latest_releases().await {
                    Ok(releases) => {
                        latest_releases.set(Some(releases));
                    }
                    Err(e) => {
                        release_check_error.set(e);
                    }
                }
                is_checking_releases.set(false);
            });
        }
    });

    // --- FIRST-TIME SETUP SCREEN ---
    if config().is_none() {
        return rsx! {
            style { "body {{ background: #27374D; margin: 0; overflow-x: hidden; }}" }
            div { style: "width: 100vw; height: 100vh; margin: 0; padding: 40px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; box-sizing: border-box; background: #27374D; color: #DDE6ED; display: flex; flex-direction: column; align-items: center; justify-content: center;",
                div { style: "max-width: 500px; width: 100%; text-align: center;",
                    h2 { style: "color: #DDE6ED; margin-bottom: 5px;", "Welcome to Factorio Enhanced Launcher" }
                    p { style: "color: #9DB2BF; font-size: 14px; margin-bottom: 25px;",
                        "Please locate your Factorio installation folder or application to begin."
                    }

                    button {
                        onclick: move |_| {
                            let dialog = rfd::FileDialog::new();
                            #[cfg(target_os = "macos")]
                            let selected_path = dialog.add_filter("Application", &["app"]).pick_file();
                            #[cfg(not(target_os = "macos"))]
                            let selected_path = dialog.pick_folder();
                            if let Some(folder_path) = selected_path {
                                match find_factorio_installation(&folder_path) {
                                    Ok(cfg) => {
                                        if let Err(e) = save_config(&cfg) {
                                            setup_error.set(format!("Failed to save config: {}", e));
                                        } else {
                                            let ver = get_installed_version(&cfg);
                                            installed_version.set(ver);
                                            global_bin_input.set(cfg.bin_path.to_string_lossy().to_string());
                                            global_data_input.set(cfg.data_path.to_string_lossy().to_string());
                                            config.set(Some(cfg));
                                        }
                                    }
                                    Err(e) => {
                                        setup_error.set(format!(
                                            "Invalid path ({}). Make sure you select the Factorio application or root folder!",
                                            e
                                         ))
                                    }
                                }
                            }
                        },
                        style: "background: #526D82; color: #DDE6ED; border: none; padding: 12px 24px; border-radius: 6px; cursor: pointer; font-weight: bold; font-size: 15px;",
                        {
                            if cfg!(target_os = "macos") {
                                "Browse for Factorio App 📂"
                            } else {
                                "Browse for Factorio Folder 📂"
                            }
                        }
                    }

                    if !setup_error().is_empty() {
                        div { style: "padding: 12px; background: #1b263b; color: #e74c3c; border: 1px solid #526D82; border-radius: 6px; margin-top: 20px; font-size: 14px; text-align: left;",
                            "{setup_error}"
                        }
                    }
                }
            }
        };
    }

    // Determine target version to download based on branch & build
    let target_version = match selected_branch().as_str() {
        "stable" => latest_releases()
            .as_ref()
            .and_then(|r| match selected_build() {
                BuildType::Alpha => r.stable.alpha.clone(),
                BuildType::Expansion => r.stable.expansion.clone().or_else(|| r.stable.alpha.clone()),
            })
            .unwrap_or_else(|| "latest".to_string()),
        "experimental" => latest_releases()
            .as_ref()
            .and_then(|r| match selected_build() {
                BuildType::Alpha => r.experimental.alpha.clone(),
                BuildType::Expansion => r.experimental.expansion.clone().or_else(|| r.experimental.alpha.clone()),
            })
            .unwrap_or_else(|| "latest".to_string()),
        _ => {
            let custom = custom_version_input();
            if custom.trim().is_empty() {
                "latest".to_string()
            } else {
                custom.trim().to_string()
            }
        }
    };

    let has_update = match (installed_version(), &target_version) {
        (Some(curr), target) if target != "latest" && !target.is_empty() => curr != *target,
        _ => false,
    };

    // --- MAIN APPLICATION SCREEN ---
    rsx! {
        style { "body {{ background: #27374D; margin: 0; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; color: #DDE6ED; }}" }
        div { style: "width: 100vw; min-height: 100vh; margin: 0; padding: 25px 20px; box-sizing: border-box; background: #27374D; display: flex; justify-content: center; overflow-y: auto;",
            div { style: "max-width: 720px; width: 100%;",

                // Header
                div { style: "display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 18px;",
                    div {
                        h1 { style: "color: #DDE6ED; margin: 0 0 5px 0; font-size: 26px;", "Factorio Enhanced Launcher" }
                        p { style: "color: #9DB2BF; font-size: 13px; margin: 0;", "Standalone releases autoupdate & isolated profile management" }
                    }
                    div { style: "display: flex; gap: 8px; align-items: center;",
                        span {
                            style: "display: inline-block; padding: 4px 10px; border-radius: 12px; font-size: 12px; font-weight: bold; background: #1b263b; border: 1px solid #526D82; color: #9DB2BF;",
                            {
                                match installed_version() {
                                    Some(ver) => format!("v{}", ver),
                                    None => "Version unknown".to_string(),
                                }
                            }
                        }
                    }
                }

                // Submenu Navigation Bar
                div { style: "display: flex; gap: 8px; margin-bottom: 20px; border-bottom: 1px solid #526D82; padding-bottom: 12px;",
                    button {
                        onclick: move |_| active_tab.set(ActiveTab::Launcher),
                        style: if active_tab() == ActiveTab::Launcher {
                            "background: #526D82; color: #DDE6ED; border: 1px solid #DDE6ED; padding: 8px 18px; border-radius: 6px; font-weight: bold; font-size: 13px; cursor: pointer; display: flex; align-items: center; gap: 6px;"
                        } else {
                            "background: #1b263b; color: #9DB2BF; border: 1px solid #526D82; padding: 8px 18px; border-radius: 6px; font-size: 13px; cursor: pointer; display: flex; align-items: center; gap: 6px;"
                        },
                        span { "🚀" }
                        span { "Launcher & Profiles" }
                    }
                    button {
                        onclick: move |_| {
                            active_tab.set(ActiveTab::Configuration);
                            if let Some(ref c) = config() {
                                global_bin_input.set(c.bin_path.to_string_lossy().to_string());
                                global_data_input.set(c.data_path.to_string_lossy().to_string());
                            }
                            config_submenu_message.set(String::new());
                        },
                        style: if active_tab() == ActiveTab::Configuration {
                            "background: #526D82; color: #DDE6ED; border: 1px solid #DDE6ED; padding: 8px 18px; border-radius: 6px; font-weight: bold; font-size: 13px; cursor: pointer; display: flex; align-items: center; gap: 6px;"
                        } else {
                            "background: #1b263b; color: #9DB2BF; border: 1px solid #526D82; padding: 8px 18px; border-radius: 6px; font-size: 13px; cursor: pointer; display: flex; align-items: center; gap: 6px;"
                        },
                        span { "⚙️" }
                        span { "Configuration Submenu" }
                    }
                }

                if !status_message().is_empty() {
                    div { style: "padding: 12px; background: #1b263b; color: #DDE6ED; border: 1px solid #526D82; border-radius: 6px; margin-bottom: 20px; font-size: 14px; display: flex; justify-content: space-between; align-items: center;",
                        span { "{status_message}" }
                        button {
                            onclick: move |_| status_message.set(String::new()),
                            style: "background: none; border: none; color: #9DB2BF; cursor: pointer; font-size: 14px;",
                            "✕"
                        }
                    }
                }

                // ========================================================
                // VIEW 1: LAUNCHER & PROFILES TAB
                // ========================================================
                if active_tab() == ActiveTab::Launcher {
                    // AUTOUPDATE CARD
                    div { style: "background: #1b263b; padding: 20px; border-radius: 8px; margin-bottom: 20px; border: 1px solid #526D82;",
                            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 15px;",
                                div {
                                    h2 { style: "margin: 0; font-size: 17px; color: #DDE6ED;", "🚀 Factorio Game Autoupdate" }
                                    span { style: "font-size: 12px; color: #9DB2BF;", "Official standalone updates direct from factorio.com" }
                                }
                                button {
                                    disabled: is_checking_releases() || is_updating(),
                                    onclick: move |_| {
                                        is_checking_releases.set(true);
                                        release_check_error.set(String::new());
                                        spawn(async move {
                                            match fetch_latest_releases().await {
                                                Ok(releases) => latest_releases.set(Some(releases)),
                                                Err(e) => release_check_error.set(e),
                                            }
                                            is_checking_releases.set(false);
                                        });
                                    },
                                    style: "background: #27374D; color: #DDE6ED; border: 1px solid #526D82; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                    if is_checking_releases() { "Checking... ⏳" } else { "Check Updates 🔄" }
                                }
                            }

                            // Release Info Grid
                            div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; margin-bottom: 15px;",
                                div { style: "background: #27374D; padding: 10px; border-radius: 6px; text-align: center; border: 1px solid #526D82;",
                                    div { style: "font-size: 11px; color: #9DB2BF; text-transform: uppercase; font-weight: 600;", "Installed" }
                                    div { style: "font-size: 15px; font-weight: bold; margin-top: 4px; color: #DDE6ED;",
                                        {
                                            match installed_version() {
                                                Some(v) => format!("v{}", v),
                                                None => "Not detected".to_string(),
                                            }
                                        }
                                    }
                                }
                                div { style: "background: #27374D; padding: 10px; border-radius: 6px; text-align: center; border: 1px solid #526D82;",
                                    div { style: "font-size: 11px; color: #9DB2BF; text-transform: uppercase; font-weight: 600;", "Latest Stable" }
                                    div { style: "font-size: 15px; font-weight: bold; margin-top: 4px; color: #2ecc71;",
                                        {
                                            match latest_releases() {
                                                Some(ref r) => match selected_build() {
                                                    BuildType::Alpha => r.stable.alpha.as_deref().unwrap_or("—").to_string(),
                                                    BuildType::Expansion => r.stable.expansion.as_deref().unwrap_or("—").to_string(),
                                                },
                                                None => "—".to_string(),
                                            }
                                        }
                                    }
                                }
                                div { style: "background: #27374D; padding: 10px; border-radius: 6px; text-align: center; border: 1px solid #526D82;",
                                    div { style: "font-size: 11px; color: #9DB2BF; text-transform: uppercase; font-weight: 600;", "Latest Experimental" }
                                    div { style: "font-size: 15px; font-weight: bold; margin-top: 4px; color: #f39c12;",
                                        {
                                            match latest_releases() {
                                                Some(ref r) => match selected_build() {
                                                    BuildType::Alpha => r.experimental.alpha.as_deref().unwrap_or("—").to_string(),
                                                    BuildType::Expansion => r.experimental.expansion.as_deref().unwrap_or("—").to_string(),
                                                },
                                                None => "—".to_string(),
                                            }
                                        }
                                    }
                                }
                            }

                            if !release_check_error().is_empty() {
                                div { style: "padding: 8px 12px; background: #27374D; color: #e74c3c; border: 1px solid #e74c3c; border-radius: 4px; margin-bottom: 12px; font-size: 13px;",
                                    "⚠️ Release check failed: {release_check_error}"
                                }
                            }

                            // Update Available Notification Banner
                            if has_update {
                                div { style: "background: #2c3e50; border: 1px solid #27ae60; padding: 12px; border-radius: 6px; margin-bottom: 15px; display: flex; justify-content: space-between; align-items: center;",
                                    div {
                                        div { style: "font-weight: bold; color: #2ecc71; font-size: 14px;", "✨ New Factorio update available!" }
                                        div { style: "color: #DDE6ED; font-size: 13px; margin-top: 2px;",
                                            "Upgrade from "
                                            span { style: "text-decoration: line-through; color: #9DB2BF;", "{installed_version().unwrap_or_default()}" }
                                            " to "
                                            strong { style: "color: #2ecc71;", "v{target_version}" }
                                        }
                                    }
                                }
                            }

                            // Update Settings / Target Options
                            div { style: "display: flex; gap: 15px; margin-bottom: 15px; flex-wrap: wrap;",
                                // Build selection
                                div { style: "flex: 1; min-width: 200px;",
                                    label { style: "display: block; font-size: 12px; color: #9DB2BF; margin-bottom: 6px; font-weight: 500;", "Build Type:" }
                                    div { style: "display: flex; gap: 6px;",
                                        button {
                                            onclick: move |_| selected_build.set(BuildType::Expansion),
                                            style: if selected_build() == BuildType::Expansion {
                                                "flex: 1; background: #526D82; color: #DDE6ED; border: 1px solid #DDE6ED; padding: 7px; border-radius: 4px; font-size: 12px; font-weight: bold; cursor: pointer;"
                                            } else {
                                                "flex: 1; background: #27374D; color: #9DB2BF; border: 1px solid #526D82; padding: 7px; border-radius: 4px; font-size: 12px; cursor: pointer;"
                                            },
                                            "Space Age 🪐"
                                        }
                                        button {
                                            onclick: move |_| selected_build.set(BuildType::Alpha),
                                            style: if selected_build() == BuildType::Alpha {
                                                "flex: 1; background: #526D82; color: #DDE6ED; border: 1px solid #DDE6ED; padding: 7px; border-radius: 4px; font-size: 12px; font-weight: bold; cursor: pointer;"
                                            } else {
                                                "flex: 1; background: #27374D; color: #9DB2BF; border: 1px solid #526D82; padding: 7px; border-radius: 4px; font-size: 12px; cursor: pointer;"
                                            },
                                            "Base Game ⚙️"
                                        }
                                    }
                                }

                                // Branch selection
                                div { style: "flex: 1; min-width: 200px;",
                                    label { style: "display: block; font-size: 12px; color: #9DB2BF; margin-bottom: 6px; font-weight: 500;", "Release Branch:" }
                                    div { style: "display: flex; gap: 6px;",
                                        button {
                                            onclick: move |_| selected_branch.set("stable".to_string()),
                                            style: if selected_branch() == "stable" {
                                                "flex: 1; background: #526D82; color: #DDE6ED; border: 1px solid #DDE6ED; padding: 7px; border-radius: 4px; font-size: 12px; font-weight: bold; cursor: pointer;"
                                            } else {
                                                "flex: 1; background: #27374D; color: #9DB2BF; border: 1px solid #526D82; padding: 7px; border-radius: 4px; font-size: 12px; cursor: pointer;"
                                            },
                                            "Stable"
                                        }
                                        button {
                                            onclick: move |_| selected_branch.set("experimental".to_string()),
                                            style: if selected_branch() == "experimental" {
                                                "flex: 1; background: #526D82; color: #DDE6ED; border: 1px solid #DDE6ED; padding: 7px; border-radius: 4px; font-size: 12px; font-weight: bold; cursor: pointer;"
                                            } else {
                                                "flex: 1; background: #27374D; color: #9DB2BF; border: 1px solid #526D82; padding: 7px; border-radius: 4px; font-size: 12px; cursor: pointer;"
                                            },
                                            "Experimental"
                                        }
                                        button {
                                            onclick: move |_| selected_branch.set("custom".to_string()),
                                            style: if selected_branch() == "custom" {
                                                "flex: 1; background: #526D82; color: #DDE6ED; border: 1px solid #DDE6ED; padding: 7px; border-radius: 4px; font-size: 12px; font-weight: bold; cursor: pointer;"
                                            } else {
                                                "flex: 1; background: #27374D; color: #9DB2BF; border: 1px solid #526D82; padding: 7px; border-radius: 4px; font-size: 12px; cursor: pointer;"
                                            },
                                            "Custom"
                                        }
                                    }
                                }
                            }

                            if selected_branch() == "custom" {
                                div { style: "margin-bottom: 15px;",
                                    label { style: "display: block; font-size: 12px; color: #9DB2BF; margin-bottom: 6px;", "Specific Version (e.g. 2.0.10 or latest):" }
                                    input {
                                        placeholder: "e.g. 2.0.10",
                                        value: "{custom_version_input}",
                                        oninput: move |evt| custom_version_input.set(evt.value()),
                                        style: "width: 100%; box-sizing: border-box; padding: 8px; background: #27374D; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px; font-size: 13px;"
                                    }
                                }
                            }

                            // Credentials Section Header / Toggle
                            div { style: "border-top: 1px solid #27374D; padding-top: 15px; margin-top: 15px; display: flex; justify-content: space-between; align-items: center;",
                                span { style: "font-size: 13px; font-weight: 500; color: #DDE6ED;",
                                    "Factorio Credentials "
                                    if username_input().is_empty() || token_input().is_empty() {
                                        span { style: "color: #e67e22; font-size: 12px;", "(Required for updates)" }
                                    } else {
                                        span { style: "color: #2ecc71; font-size: 12px;", "✓ Saved" }
                                    }
                                }
                                button {
                                    onclick: move |_| show_credentials_panel.set(!show_credentials_panel()),
                                    style: "background: none; border: none; color: #9DB2BF; cursor: pointer; font-size: 12px; text-decoration: underline;",
                                    if show_credentials_panel() { "Hide Settings ▲" } else { "Configure Credentials ⚙️" }
                                }
                            }

                            // Credentials Panel
                            if show_credentials_panel() || username_input().is_empty() || token_input().is_empty() {
                                div { style: "background: #27374D; padding: 12px; border-radius: 6px; margin-top: 10px; border: 1px solid #526D82;",
                                    p { style: "font-size: 12px; color: #9DB2BF; margin: 0 0 10px 0;",
                                        "Factorio requires your Factorio account Username and Service Token to download official updates. You can find your Service Token in your "
                                        strong { "factorio.com -> Profile -> Service Token" }
                                        " section."
                                    }
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-bottom: 10px;",
                                        div {
                                            label { style: "display: block; font-size: 11px; color: #9DB2BF; margin-bottom: 4px;", "Factorio Username:" }
                                            input {
                                                placeholder: "Factorio Username",
                                                value: "{username_input}",
                                                oninput: move |evt| username_input.set(evt.value()),
                                                style: "width: 100%; box-sizing: border-box; padding: 7px; background: #1b263b; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px; font-size: 13px;"
                                            }
                                        }
                                        div {
                                            label { style: "display: block; font-size: 11px; color: #9DB2BF; margin-bottom: 4px;", "Service Token:" }
                                            div { style: "display: flex; gap: 5px;",
                                                input {
                                                    r#type: if show_token() { "text" } else { "password" },
                                                    placeholder: "Service Token",
                                                    value: "{token_input}",
                                                    oninput: move |evt| token_input.set(evt.value()),
                                                    style: "flex: 1; box-sizing: border-box; padding: 7px; background: #1b263b; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px; font-size: 13px;"
                                                }
                                                button {
                                                    onclick: move |_| show_token.set(!show_token()),
                                                    style: "background: #1b263b; border: 1px solid #526D82; color: #9DB2BF; border-radius: 4px; padding: 0 8px; cursor: pointer;",
                                                    if show_token() { "👁️" } else { "🔒" }
                                                }
                                            }
                                        }
                                    }
                                    button {
                                        onclick: move |_| {
                                            let Some(mut updated_cfg) = config() else { return; };
                                            updated_cfg.username = Some(username_input());
                                            updated_cfg.token = Some(token_input());
                                            updated_cfg.preferred_branch = Some(selected_branch());
                                            updated_cfg.preferred_build = Some(selected_build().as_str().to_string());
                                            if let Err(e) = save_config(&updated_cfg) {
                                                status_message.set(format!("Failed to save credentials: {}", e));
                                            } else {
                                                config.set(Some(updated_cfg));
                                                status_message.set("Credentials saved successfully!".into());
                                            }
                                        },
                                        style: "background: #526D82; color: #DDE6ED; border: none; padding: 6px 14px; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: 500;",
                                        "Save Credentials 💾"
                                    }
                                }
                            }

                            // Update Execution Button & Progress
                            div { style: "margin-top: 15px;",
                                if is_updating() {
                                    div { style: "background: #27374D; border: 1px solid #526D82; border-radius: 6px; padding: 15px;",
                                        match update_stage() {
                                            UpdateStage::Checking => rsx! {
                                                div { style: "font-size: 13px; color: #DDE6ED;", "Connecting to Factorio servers..." }
                                            },
                                            UpdateStage::Downloading(prog) => {
                                                let pct = prog.percentage;
                                                let dl_mb = prog.downloaded_bytes as f64 / 1_048_576.0;
                                                let tot_mb = prog.total_bytes as f64 / 1_048_576.0;
                                                rsx! {
                                                    div {
                                                        div { style: "display: flex; justify-content: space-between; font-size: 12px; margin-bottom: 6px;",
                                                            span { "Downloading update archive..." }
                                                            span { style: "font-weight: bold; color: #2ecc71;", "{pct:.1}%" }
                                                        }
                                                        div { style: "width: 100%; height: 10px; background: #1b263b; border-radius: 5px; overflow: hidden; margin-bottom: 6px; border: 1px solid #526D82;",
                                                            div { style: "width: {pct:.1}%; height: 100%; background: #2ecc71; transition: width 0.2s;" }
                                                        }
                                                        div { style: "font-size: 11px; color: #9DB2BF; text-align: right;",
                                                            "{dl_mb:.1} MB / {tot_mb:.1} MB"
                                                        }
                                                    }
                                                }
                                            },
                                            UpdateStage::Extracting(msg) => rsx! {
                                                div { style: "display: flex; align-items: center; gap: 8px; font-size: 13px; color: #f39c12;",
                                                    span { "📦" }
                                                    span { "{msg}" }
                                                }
                                            },
                                            UpdateStage::Installing => rsx! {
                                                div { style: "display: flex; align-items: center; gap: 8px; font-size: 13px; color: #3498db;",
                                                    span { "⚙️" }
                                                    span { "Overwriting local Factorio installation..." }
                                                }
                                            },
                                            _ => rsx! { div {} },
                                        }
                                    }
                                } else {
                                    {
                                        let ver_to_install = target_version.clone();
                                        let build_to_install = selected_build();
                                        let btn_label = if has_update {
                                            format!("Update Factorio to v{} ({})", target_version, selected_build().display_name())
                                        } else {
                                            format!("Install / Reinstall Factorio v{} ({})", target_version, selected_build().display_name())
                                        };
                                        rsx! {
                                            button {
                                                onclick: move |_| {
                                                    let Some(active_cfg) = config() else { return; };
                                                    let username = username_input();
                                                    let token = token_input();
                                                    if username.trim().is_empty() || token.trim().is_empty() {
                                                        status_message.set("Please provide your Factorio Username and Service Token first.".into());
                                                        show_credentials_panel.set(true);
                                                        return;
                                                    }

                                                    // Auto-save credentials
                                                    let mut updated_cfg = active_cfg.clone();
                                                    updated_cfg.username = Some(username.clone());
                                                    updated_cfg.token = Some(token.clone());
                                                    updated_cfg.preferred_branch = Some(selected_branch());
                                                    updated_cfg.preferred_build = Some(build_to_install.as_str().to_string());
                                                    let _ = save_config(&updated_cfg);
                                                    config.set(Some(updated_cfg.clone()));

                                                    is_updating.set(true);
                                                    status_message.set(String::new());

                                                    let target_ver = ver_to_install.clone();
                                                    let cfg_clone = updated_cfg.clone();

                                                    spawn(async move {
                                                        let result = perform_update(
                                                            &cfg_clone,
                                                            &target_ver,
                                                            build_to_install,
                                                            &username,
                                                            &token,
                                                            move |stage| update_stage.set(stage),
                                                        ).await;

                                                        match result {
                                                            Ok(new_ver) => {
                                                                installed_version.set(Some(new_ver.clone()));
                                                                status_message.set(format!("🎉 Factorio successfully updated to v{}!", new_ver));
                                                            }
                                                            Err(e) => {
                                                                status_message.set(format!("Update failed: {}", e));
                                                            }
                                                        }
                                                        is_updating.set(false);
                                                    });
                                                },
                                                style: "width: 100%; background: #27ae60; color: #ffffff; border: none; padding: 12px; border-radius: 6px; cursor: pointer; font-weight: bold; font-size: 14px; display: flex; justify-content: center; align-items: center; gap: 8px;",
                                                span { "🚀" }
                                                span { "{btn_label}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // PROFILES CARD
                        div { style: "background: #1b263b; padding: 20px; border-radius: 8px; margin-bottom: 20px; border: 1px solid #526D82;",
                            h2 { style: "margin-top: 0; font-size: 17px; color: #DDE6ED; margin-bottom: 12px;", "Manage Factorio Profiles" }

                            div { style: "display: flex; gap: 10px; margin-bottom: 20px;",
                                input {
                                    placeholder: "New Profile Name",
                                    value: "{new_profile_name}",
                                    oninput: move |evt| new_profile_name.set(evt.value()),
                                    onkeydown: move |evt| {
                                        if evt.key() == Key::Enter {
                                            let Some(current_cfg) = config() else { return; };
                                            let name = new_profile_name();
                                            match create_profile_dir(&name, &current_cfg) {
                                                Ok(_) => {
                                                    status_message.set(format!("Created profile '{}'", name));
                                                    new_profile_name.set(String::new());
                                                    refresh_profiles();
                                                }
                                                Err(e) => status_message.set(format!("Error: {}", e)),
                                            }
                                        }
                                    },
                                    style: "flex: 1; padding: 8px 12px; background: #27374D; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px; font-size: 13px;"
                                }
                                button {
                                    onclick: move |_| {
                                        let Some(current_cfg) = config() else { return; };
                                        let name = new_profile_name();
                                        match create_profile_dir(&name, &current_cfg) {
                                            Ok(_) => {
                                                status_message.set(format!("Created profile '{}'", name));
                                                new_profile_name.set(String::new());
                                                refresh_profiles();
                                            }
                                            Err(e) => status_message.set(format!("Error: {}", e)),
                                        }
                                    },
                                    style: "background: #526D82; color: #DDE6ED; border: none; padding: 8px 18px; border-radius: 4px; cursor: pointer; font-weight: bold; font-size: 13px;",
                                    "Create Profile"
                                }
                            }

                            h3 { style: "font-size: 14px; color: #9DB2BF; margin-bottom: 10px;", "Existing Profiles" }
                            if profiles().is_empty() {
                                p { style: "color: #9DB2BF; font-style: italic; font-size: 13px;", "No profiles created yet. Create one above to manage isolated saves and mods!" }
                            } else {
                                ul { style: "list-style: none; padding: 0; margin: 0;",
                                    for profile in profiles() {
                                        {
                                            let p_name = profile.clone();
                                            let is_renaming = renaming_profile() == Some(p_name.clone());
                                            let is_editing_bin = editing_profile_bin() == Some(p_name.clone());
                                            let custom_bin = get_profile_bin_path(&p_name);
                                            let has_override = custom_bin.is_some();

                                            let p_launch = p_name.clone();
                                            let p_folder = p_name.clone();
                                            let p_rename_trigger = p_name.clone();
                                            let p_rename_key = p_name.clone();
                                            let p_rename_click = p_name.clone();
                                            let p_bin_trigger = p_name.clone();
                                            let p_bin_save = p_name.clone();
                                            let p_bin_reset = p_name.clone();

                                            rsx! {
                                                li {
                                                    key: "{p_name}",
                                                    style: "background: #27374D; border: 1px solid #526D82; padding: 12px 14px; border-radius: 6px; margin-bottom: 10px;",

                                                    // Row 1: Profile info & Actions
                                                    div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;",
                                                        if is_renaming {
                                                            div { style: "display: flex; gap: 8px; flex: 1; min-width: 260px; align-items: center;",
                                                                input {
                                                                    value: "{rename_input}",
                                                                    oninput: move |evt| rename_input.set(evt.value()),
                                                                    onkeydown: move |evt| {
                                                                        if evt.key() == Key::Enter {
                                                                            let old = p_rename_key.clone();
                                                                            let new_name = rename_input();
                                                                            match rename_profile(&old, &new_name) {
                                                                                Ok(_) => {
                                                                                    status_message.set(format!("Profile successfully renamed from '{}' to '{}'", old, new_name));
                                                                                    renaming_profile.set(None);
                                                                                    refresh_profiles();
                                                                                }
                                                                                Err(e) => status_message.set(format!("Rename error: {}", e)),
                                                                            }
                                                                        } else if evt.key() == Key::Escape {
                                                                            renaming_profile.set(None);
                                                                        }
                                                                    },
                                                                    style: "flex: 1; padding: 6px 10px; background: #1b263b; color: #DDE6ED; border: 1px solid #2ecc71; border-radius: 4px; font-size: 13px;"
                                                                }
                                                                button {
                                                                    onclick: move |_| {
                                                                        let old = p_rename_click.clone();
                                                                        let new_name = rename_input();
                                                                        match rename_profile(&old, &new_name) {
                                                                            Ok(_) => {
                                                                                status_message.set(format!("Profile successfully renamed from '{}' to '{}'", old, new_name));
                                                                                renaming_profile.set(None);
                                                                                refresh_profiles();
                                                                            }
                                                                            Err(e) => status_message.set(format!("Rename error: {}", e)),
                                                                        }
                                                                    },
                                                                    style: "background: #27ae60; color: #ffffff; border: none; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: bold;",
                                                                    "Save ✓"
                                                                }
                                                                button {
                                                                    onclick: move |_| renaming_profile.set(None),
                                                                    style: "background: #1b263b; color: #9DB2BF; border: 1px solid #526D82; padding: 6px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                                                    "Cancel ✕"
                                                                }
                                                            }
                                                        } else {
                                                            div { style: "display: flex; align-items: center; gap: 8px; flex-wrap: wrap;",
                                                                span { style: "font-weight: 600; color: #DDE6ED; font-size: 15px;", "{p_name}" }
                                                                if let Some(ref c_bin) = custom_bin {
                                                                    span {
                                                                        style: "display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 11px; font-weight: bold; background: #2980b9; color: #ffffff;",
                                                                        title: "{c_bin.display()}",
                                                                        "Custom Executable"
                                                                    }
                                                                } else {
                                                                    span {
                                                                        style: "display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 11px; background: #1b263b; border: 1px solid #526D82; color: #9DB2BF;",
                                                                        "Default Executable"
                                                                    }
                                                                }
                                                            }
                                                        }

                                                        div { style: "display: flex; gap: 6px; align-items: center; flex-wrap: wrap;",
                                                            button {
                                                                disabled: is_updating(),
                                                                onclick: move |_| {
                                                                    let Some(active_cfg) = config() else { return; };
                                                                    match launch_game(&p_launch, &active_cfg) {
                                                                        Ok(_) => status_message.set(format!("Launched Factorio with profile '{}'", p_launch)),
                                                                        Err(e) => status_message.set(e),
                                                                    }
                                                                },
                                                                style: "background: #27ae60; color: #ffffff; border: none; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: bold;",
                                                                "Launch Game ▶"
                                                            }
                                                            button {
                                                                onclick: move |_| {
                                                                    let path = get_profiles_dir().join(&p_folder);
                                                                    #[cfg(target_os = "windows")]
                                                                    let _ = Command::new("explorer.exe").arg(path).spawn();
                                                                    #[cfg(not(target_os = "windows"))]
                                                                    let _ = Command::new("open").arg(path).spawn();
                                                                },
                                                                style: "background: #1b263b; color: #9DB2BF; border: 1px solid #526D82; padding: 6px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                                                "📁"
                                                            }
                                                            button {
                                                                disabled: is_renaming,
                                                                onclick: move |_| {
                                                                    renaming_profile.set(Some(p_rename_trigger.clone()));
                                                                    rename_input.set(p_rename_trigger.clone());
                                                                },
                                                                style: "background: #1b263b; color: #DDE6ED; border: 1px solid #526D82; padding: 6px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                                                "✏️"
                                                            }
                                                            button {
                                                                onclick: move |_| {
                                                                    if editing_profile_bin() == Some(p_bin_trigger.clone()) {
                                                                        editing_profile_bin.set(None);
                                                                    } else {
                                                                        editing_profile_bin.set(Some(p_bin_trigger.clone()));
                                                                        let cur = get_profile_bin_path(&p_bin_trigger);
                                                                        profile_bin_input.set(cur.map(|p| p.to_string_lossy().to_string()).unwrap_or_default());
                                                                        profile_bin_status.set(String::new());
                                                                    }
                                                                },
                                                                style: if is_editing_bin {
                                                                    "background: #526D82; color: #DDE6ED; border: 1px solid #DDE6ED; padding: 6px 10px; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: bold;"
                                                                } else {
                                                                    "background: #1b263b; color: #DDE6ED; border: 1px solid #526D82; padding: 6px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;"
                                                                },
                                                                "⚙️"
                                                            }
                                                        }
                                                    }

                                                    // Row 2: Per-Profile Executable Configuration Drawer
                                                    if is_editing_bin {
                                                        div { style: "margin-top: 12px; padding: 12px; background: #1b263b; border: 1px solid #526D82; border-radius: 6px;",
                                                            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;",
                                                                div {
                                                                    span { style: "font-weight: bold; color: #DDE6ED; font-size: 13px;", "Custom Executable for '{p_name}'" }
                                                                    div { style: "font-size: 11px; color: #9DB2BF; margin-top: 2px;",
                                                                        {
                                                                            if let Some(ref c) = custom_bin {
                                                                                let ver = get_version_for_binary(c);
                                                                                format!("Custom path set: {} {}", c.display(), ver.map(|v| format!("(v{})", v)).unwrap_or_default())
                                                                            } else if let Some(ref cfg) = config() {
                                                                                format!("Currently using global default: {}", cfg.bin_path.display())
                                                                            } else {
                                                                                "Using global default".to_string()
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                button {
                                                                    onclick: move |_| editing_profile_bin.set(None),
                                                                    style: "background: none; border: none; color: #9DB2BF; cursor: pointer; font-size: 12px;",
                                                                    "✕ Close"
                                                                }
                                                            }

                                                            div { style: "display: flex; gap: 8px; margin-bottom: 8px;",
                                                                input {
                                                                    placeholder: "e.g. /path/to/Factorio.app or /bin/x64/factorio",
                                                                    value: "{profile_bin_input}",
                                                                    oninput: move |evt| profile_bin_input.set(evt.value()),
                                                                    style: "flex: 1; padding: 7px 10px; background: #27374D; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px; font-size: 12px;"
                                                                }
                                                                button {
                                                                    onclick: move |_| {
                                                                        let dialog = rfd::FileDialog::new();
                                                                        #[cfg(target_os = "macos")]
                                                                        let selected = dialog.add_filter("Application or Executable", &["app", ""]).pick_file();
                                                                        #[cfg(target_os = "windows")]
                                                                        let selected = dialog.add_filter("Factorio Executable", &["exe"]).pick_file();
                                                                        #[cfg(target_os = "linux")]
                                                                        let selected = dialog.pick_file();

                                                                        if let Some(path) = selected {
                                                                            let resolved = resolve_executable_path(&path);
                                                                            profile_bin_input.set(resolved.to_string_lossy().to_string());
                                                                        }
                                                                    },
                                                                    style: "background: #526D82; color: #DDE6ED; border: none; padding: 7px 12px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                                                    "Browse 📂"
                                                                }
                                                            }

                                                            div { style: "display: flex; gap: 8px; align-items: center; flex-wrap: wrap;",
                                                                button {
                                                                    onclick: move |_| {
                                                                        let val = profile_bin_input().trim().to_string();
                                                                        if val.is_empty() {
                                                                            profile_bin_status.set("Please provide a valid path or click 'Reset to Global Default'.".into());
                                                                            return;
                                                                        }
                                                                        let target = resolve_executable_path(Path::new(&val));
                                                                        if !target.exists() {
                                                                            profile_bin_status.set(format!("Executable file does not exist at '{}'.", target.display()));
                                                                            return;
                                                                        }

                                                                        match set_profile_bin_path(&p_bin_save, Some(target.clone())) {
                                                                            Ok(_) => {
                                                                                profile_bin_input.set(target.to_string_lossy().to_string());
                                                                                profile_bin_status.set("✓ Custom executable saved for this profile!".into());
                                                                                status_message.set(format!("Updated custom executable for profile '{}'", p_bin_save));
                                                                            }
                                                                            Err(e) => profile_bin_status.set(format!("Failed to save: {}", e)),
                                                                        }
                                                                    },
                                                                    style: "background: #27ae60; color: #ffffff; border: none; padding: 6px 14px; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: bold;",
                                                                    "Save Override 💾"
                                                                }
                                                                if has_override {
                                                                    button {
                                                                        onclick: move |_| {
                                                                            match set_profile_bin_path(&p_bin_reset, None) {
                                                                                Ok(_) => {
                                                                                    profile_bin_input.set(String::new());
                                                                                    profile_bin_status.set("✓ Reset to global launcher default executable.".into());
                                                                                    status_message.set(format!("Reset profile '{}' to global default executable", p_bin_reset));
                                                                                }
                                                                                Err(e) => profile_bin_status.set(format!("Failed to reset: {}", e)),
                                                                            }
                                                                        },
                                                                        style: "background: #27374D; color: #e67e22; border: 1px solid #e67e22; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                                                        "Reset to Global Default 🔄"
                                                                    }
                                                                }
                                                            }

                                                            if !profile_bin_status().is_empty() {
                                                                div { style: "margin-top: 8px; font-size: 12px; color: #DDE6ED;", "{profile_bin_status}" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ========================================================
                // VIEW 2: CONFIGURATION SUBMENU
                // ========================================================
                if active_tab() == ActiveTab::Configuration {
                    div {
                        // Submenu Description
                            div { style: "margin-bottom: 20px;",
                                h2 { style: "margin: 0 0 5px 0; font-size: 20px; color: #DDE6ED;", "⚙️ Configuration Submenu" }
                                p { style: "margin: 0; font-size: 13px; color: #9DB2BF;",
                                    "Change the global Factorio executable location, customize executables per profile, or update credentials."
                                }
                            }

                            if !config_submenu_message().is_empty() {
                                div { style: "padding: 12px; background: #1b263b; color: #DDE6ED; border: 1px solid #526D82; border-radius: 6px; margin-bottom: 20px; font-size: 14px; display: flex; justify-content: space-between; align-items: center;",
                                    span { "{config_submenu_message}" }
                                    button {
                                        onclick: move |_| config_submenu_message.set(String::new()),
                                        style: "background: none; border: none; color: #9DB2BF; cursor: pointer; font-size: 14px;",
                                        "✕"
                                    }
                                }
                            }

                            // 1. GLOBAL FACTORIO EXECUTABLE (bin_path) CARD
                            div { style: "background: #1b263b; padding: 20px; border-radius: 8px; margin-bottom: 20px; border: 1px solid #526D82;",
                                h3 { style: "margin-top: 0; font-size: 16px; color: #DDE6ED; margin-bottom: 6px;", "Global Factorio Executable (bin_path)" }
                                p { style: "font-size: 13px; color: #9DB2BF; margin: 0 0 15px 0;",
                                    "This is the default executable used to launch Factorio across all profiles (unless specifically overridden by a profile)."
                                }

                                // Current Status details
                                div { style: "background: #27374D; border: 1px solid #526D82; padding: 12px; border-radius: 6px; margin-bottom: 15px;",
                                    div { style: "margin-bottom: 8px;",
                                        div { style: "font-size: 11px; color: #9DB2BF; text-transform: uppercase; font-weight: bold; margin-bottom: 2px;", "Current Executable Path" }
                                        div { style: "font-size: 13px; font-family: monospace; color: #DDE6ED; word-break: break-all;",
                                            {
                                                config().as_ref().map(|c| c.bin_path.display().to_string()).unwrap_or_else(|| "Not configured".to_string())
                                            }
                                        }
                                    }

                                    div { style: "display: flex; gap: 12px; align-items: center; flex-wrap: wrap;",
                                        div {
                                            span { style: "font-size: 11px; color: #9DB2BF; text-transform: uppercase; font-weight: bold; margin-right: 6px;", "Status:" }
                                            {
                                                if let Some(ref c) = config() {
                                                    if c.bin_path.exists() {
                                                        let ver = get_version_for_binary(&c.bin_path);
                                                        rsx! {
                                                            span { style: "color: #2ecc71; font-size: 12px; font-weight: bold;",
                                                                "✓ File exists "
                                                                if let Some(v) = ver {
                                                                    span { style: "color: #DDE6ED;", "(v{v})" }
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        rsx! {
                                                            span { style: "color: #e74c3c; font-size: 12px; font-weight: bold;",
                                                                "⚠️ Executable file not found!"
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    rsx! { span { style: "color: #9DB2BF; font-size: 12px;", "—" } }
                                                }
                                            }
                                        }

                                        div {
                                            span { style: "font-size: 11px; color: #9DB2BF; text-transform: uppercase; font-weight: bold; margin-right: 6px;", "Data Path:" }
                                            span { style: "font-size: 12px; font-family: monospace; color: #9DB2BF;",
                                                {
                                                    config().as_ref().map(|c| c.data_path.display().to_string()).unwrap_or_default()
                                                }
                                            }
                                        }
                                    }
                                }

                                // Change Executable Path Form
                                label { style: "display: block; font-size: 12px; color: #9DB2BF; margin-bottom: 6px; font-weight: 500;", "Change Factorio Executable Location:" }
                                div { style: "display: flex; gap: 8px; margin-bottom: 12px; flex-wrap: wrap;",
                                    input {
                                        placeholder: "Full path to Factorio executable or Factorio.app",
                                        value: "{global_bin_input}",
                                        oninput: move |evt| global_bin_input.set(evt.value()),
                                        style: "flex: 1; min-width: 250px; padding: 8px 12px; background: #27374D; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px; font-size: 13px;"
                                    }
                                    button {
                                        onclick: move |_| {
                                            let dialog = rfd::FileDialog::new();
                                            #[cfg(target_os = "macos")]
                                            let selected = dialog.add_filter("Application or Executable", &["app", ""]).pick_file();
                                            #[cfg(target_os = "windows")]
                                            let selected = dialog.add_filter("Factorio Executable", &["exe"]).pick_file();
                                            #[cfg(target_os = "linux")]
                                            let selected = dialog.pick_file();

                                            if let Some(path) = selected {
                                                let resolved = resolve_executable_path(&path);
                                                global_bin_input.set(resolved.to_string_lossy().to_string());
                                                if let Some(inferred) = infer_data_path(&resolved) {
                                                    global_data_input.set(inferred.to_string_lossy().to_string());
                                                }
                                            }
                                        },
                                        style: "background: #526D82; color: #DDE6ED; border: none; padding: 8px 14px; border-radius: 4px; cursor: pointer; font-size: 13px; font-weight: 500;",
                                        "Browse Executable 📂"
                                    }
                                    button {
                                        onclick: move |_| {
                                            let dialog = rfd::FileDialog::new();
                                            #[cfg(target_os = "macos")]
                                            let selected = dialog.add_filter("Application", &["app"]).pick_file();
                                            #[cfg(not(target_os = "macos"))]
                                            let selected = dialog.pick_folder();

                                            if let Some(folder) = selected {
                                                match find_factorio_installation(&folder) {
                                                    Ok(detected) => {
                                                        global_bin_input.set(detected.bin_path.to_string_lossy().to_string());
                                                        global_data_input.set(detected.data_path.to_string_lossy().to_string());
                                                        config_submenu_message.set("✓ Automatically detected Factorio executable and data directory!".into());
                                                    }
                                                    Err(e) => {
                                                        config_submenu_message.set(format!("Could not auto-detect installation: {}", e));
                                                    }
                                                }
                                            }
                                        },
                                        style: "background: #27374D; color: #DDE6ED; border: 1px solid #526D82; padding: 8px 14px; border-radius: 4px; cursor: pointer; font-size: 13px;",
                                        "Browse Folder / App 📁"
                                    }
                                }

                                // Optional Data Path Input
                                div { style: "margin-bottom: 15px;",
                                    label { style: "display: block; font-size: 12px; color: #9DB2BF; margin-bottom: 6px;", "Data Directory (data_path):" }
                                    div { style: "display: flex; gap: 8px;",
                                        input {
                                            placeholder: "Path to data directory (contains base/)",
                                            value: "{global_data_input}",
                                            oninput: move |evt| global_data_input.set(evt.value()),
                                            style: "flex: 1; padding: 8px 12px; background: #27374D; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px; font-size: 13px;"
                                        }
                                        button {
                                            onclick: move |_| {
                                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                                    global_data_input.set(folder.to_string_lossy().to_string());
                                                }
                                            },
                                            style: "background: #27374D; color: #DDE6ED; border: 1px solid #526D82; padding: 8px 12px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                            "Browse Data 📂"
                                        }
                                    }
                                }

                                button {
                                    onclick: move |_| {
                                        let bin_str = global_bin_input().trim().to_string();
                                        if bin_str.is_empty() {
                                            config_submenu_message.set("Executable path cannot be empty.".into());
                                            return;
                                        }
                                        let resolved_bin = resolve_executable_path(Path::new(&bin_str));
                                        if !resolved_bin.exists() {
                                            config_submenu_message.set(format!("Executable file does not exist at '{}'.", resolved_bin.display()));
                                            return;
                                        }

                                        let Some(mut updated_cfg) = config() else { return; };
                                        updated_cfg.bin_path = resolved_bin.clone();

                                        let data_str = global_data_input().trim().to_string();
                                        if !data_str.is_empty() && Path::new(&data_str).exists() {
                                            updated_cfg.data_path = PathBuf::from(data_str);
                                        } else if let Some(inferred) = infer_data_path(&resolved_bin) {
                                            updated_cfg.data_path = inferred.clone();
                                            global_data_input.set(inferred.to_string_lossy().to_string());
                                        }

                                        if let Err(e) = save_config(&updated_cfg) {
                                            config_submenu_message.set(format!("Failed to save config: {}", e));
                                        } else {
                                            let ver = get_installed_version(&updated_cfg);
                                            installed_version.set(ver.clone());
                                            config.set(Some(updated_cfg));
                                            global_bin_input.set(resolved_bin.to_string_lossy().to_string());
                                            config_submenu_message.set(format!(
                                                "✓ Global Factorio executable updated successfully!{}",
                                                ver.map(|v| format!(" (Detected: v{})", v)).unwrap_or_default()
                                            ));
                                            status_message.set("Global Factorio executable location saved.".into());
                                        }
                                    },
                                    style: "background: #27ae60; color: #ffffff; border: none; padding: 10px 20px; border-radius: 4px; cursor: pointer; font-weight: bold; font-size: 13px;",
                                    "Save Global Executable Location 💾"
                                }
                            }

                            // 2. PROFILE-SPECIFIC EXECUTABLE OVERRIDES CARD
                            div { style: "background: #1b263b; padding: 20px; border-radius: 8px; margin-bottom: 20px; border: 1px solid #526D82;",
                                h3 { style: "margin-top: 0; font-size: 16px; color: #DDE6ED; margin-bottom: 6px;", "Profile-Specific Executable Overrides" }
                                p { style: "font-size: 13px; color: #9DB2BF; margin: 0 0 15px 0;",
                                    "Override the default executable for each profile independently. Useful for testing different branches, versions, or standalone installs."
                                }

                                if profiles().is_empty() {
                                    p { style: "color: #9DB2BF; font-style: italic; font-size: 13px;", "No profiles created yet. Create a profile in the Launcher & Profiles tab." }
                                } else {
                                    ul { style: "list-style: none; padding: 0; margin: 0;",
                                        for p_entry in profiles() {
                                            {
                                                let p_name = p_entry.clone();
                                                let is_editing = config_editing_profile_bin() == Some(p_name.clone());
                                                let custom_bin = get_profile_bin_path(&p_name);
                                                let has_override = custom_bin.is_some();

                                                let p_toggle = p_name.clone();
                                                let p_save = p_name.clone();
                                                let p_reset_row = p_name.clone();
                                                let p_reset_drawer = p_name.clone();

                                                rsx! {
                                                    li {
                                                        key: "{p_name}",
                                                        style: "background: #27374D; border: 1px solid #526D82; padding: 12px 14px; border-radius: 6px; margin-bottom: 8px;",

                                                        div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px;",
                                                            div {
                                                                span { style: "font-weight: 600; color: #DDE6ED; font-size: 14px;", "{p_name}" }
                                                                div { style: "font-size: 12px; color: #9DB2BF; margin-top: 2px;",
                                                                    {
                                                                        if let Some(ref cb) = custom_bin {
                                                                            let ver = get_version_for_binary(cb);
                                                                            format!("Overridden: {} {}", cb.display(), ver.map(|v| format!("(v{})", v)).unwrap_or_default())
                                                                        } else {
                                                                            "Using Global Default".to_string()
                                                                        }
                                                                    }
                                                                }
                                                            }

                                                            div { style: "display: flex; gap: 6px; align-items: center;",
                                                                button {
                                                                    onclick: move |_| {
                                                                        if config_editing_profile_bin() == Some(p_toggle.clone()) {
                                                                            config_editing_profile_bin.set(None);
                                                                        } else {
                                                                            config_editing_profile_bin.set(Some(p_toggle.clone()));
                                                                            let cur = get_profile_bin_path(&p_toggle);
                                                                            config_profile_bin_input.set(cur.map(|p| p.to_string_lossy().to_string()).unwrap_or_default());
                                                                            config_profile_bin_status.set(String::new());
                                                                        }
                                                                    },
                                                                    style: if is_editing {
                                                                        "background: #526D82; color: #DDE6ED; border: 1px solid #DDE6ED; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: bold;"
                                                                    } else {
                                                                        "background: #1b263b; color: #DDE6ED; border: 1px solid #526D82; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 12px;"
                                                                    },
                                                                    "Edit Executable ⚙️"
                                                                }
                                                                if has_override {
                                                                    button {
                                                                        onclick: move |_| {
                                                                            let _ = set_profile_bin_path(&p_reset_row, None);
                                                                            config_profile_bin_input.set(String::new());
                                                                            config_profile_bin_status.set("Reset to global default executable.".into());
                                                                        },
                                                                        style: "background: #1b263b; color: #e67e22; border: 1px solid #e67e22; padding: 6px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                                                        "Reset 🔄"
                                                                    }
                                                                }
                                                            }
                                                        }

                                                        if is_editing {
                                                            div { style: "margin-top: 10px; padding: 10px; background: #1b263b; border-radius: 4px; border: 1px solid #526D82;",
                                                                div { style: "display: flex; gap: 8px; margin-bottom: 8px;",
                                                                    input {
                                                                        placeholder: "Custom Factorio executable path",
                                                                        value: "{config_profile_bin_input}",
                                                                        oninput: move |evt| config_profile_bin_input.set(evt.value()),
                                                                        style: "flex: 1; padding: 6px 10px; background: #27374D; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px; font-size: 12px;"
                                                                    }
                                                                    button {
                                                                        onclick: move |_| {
                                                                            let dialog = rfd::FileDialog::new();
                                                                            #[cfg(target_os = "macos")]
                                                                            let selected = dialog.add_filter("Application or Executable", &["app", ""]).pick_file();
                                                                            #[cfg(target_os = "windows")]
                                                                            let selected = dialog.add_filter("Factorio Executable", &["exe"]).pick_file();
                                                                            #[cfg(target_os = "linux")]
                                                                            let selected = dialog.pick_file();

                                                                            if let Some(path) = selected {
                                                                                let resolved = resolve_executable_path(&path);
                                                                                config_profile_bin_input.set(resolved.to_string_lossy().to_string());
                                                                            }
                                                                        },
                                                                        style: "background: #526D82; color: #DDE6ED; border: none; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                                                        "Browse 📂"
                                                                    }
                                                                }

                                                                div { style: "display: flex; gap: 8px;",
                                                                    button {
                                                                        onclick: move |_| {
                                                                            let val = config_profile_bin_input().trim().to_string();
                                                                            if val.is_empty() {
                                                                                config_profile_bin_status.set("Please provide an executable path or click 'Reset to Global Default'.".into());
                                                                                return;
                                                                            }
                                                                            let target = resolve_executable_path(Path::new(&val));
                                                                            if !target.exists() {
                                                                                config_profile_bin_status.set(format!("File does not exist at '{}'.", target.display()));
                                                                                return;
                                                                            }
                                                                            match set_profile_bin_path(&p_save, Some(target.clone())) {
                                                                                Ok(_) => {
                                                                                    config_profile_bin_input.set(target.to_string_lossy().to_string());
                                                                                    config_profile_bin_status.set("✓ Custom executable saved for this profile!".into());
                                                                                }
                                                                                Err(e) => config_profile_bin_status.set(format!("Failed to save: {}", e)),
                                                                            }
                                                                        },
                                                                        style: "background: #27ae60; color: #ffffff; border: none; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: bold;",
                                                                        "Save Override 💾"
                                                                    }
                                                                    if has_override {
                                                                        button {
                                                                            onclick: move |_| {
                                                                                let _ = set_profile_bin_path(&p_reset_drawer, None);
                                                                                config_profile_bin_input.set(String::new());
                                                                                config_profile_bin_status.set("✓ Reset to global launcher default executable.".into());
                                                                            },
                                                                            style: "background: #27374D; color: #e67e22; border: 1px solid #e67e22; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                                                            "Reset to Global Default 🔄"
                                                                        }
                                                                    }
                                                                    button {
                                                                        onclick: move |_| config_editing_profile_bin.set(None),
                                                                        style: "background: #27374D; color: #9DB2BF; border: 1px solid #526D82; padding: 6px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;",
                                                                        "Close ✕"
                                                                    }
                                                                }

                                                                if !config_profile_bin_status().is_empty() {
                                                                    div { style: "margin-top: 8px; font-size: 12px; color: #DDE6ED;", "{config_profile_bin_status}" }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // 3. FACTORIO ACCOUNT CREDENTIALS CARD
                            div { style: "background: #1b263b; padding: 20px; border-radius: 8px; margin-bottom: 20px; border: 1px solid #526D82;",
                                h3 { style: "margin-top: 0; font-size: 16px; color: #DDE6ED; margin-bottom: 6px;", "Factorio Account Credentials" }
                                p { style: "font-size: 13px; color: #9DB2BF; margin: 0 0 15px 0;",
                                    "Used to authenticate downloads for official Factorio updates from factorio.com."
                                }

                                div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px;",
                                    div {
                                        label { style: "display: block; font-size: 12px; color: #9DB2BF; margin-bottom: 4px;", "Username:" }
                                        input {
                                            placeholder: "Factorio Username",
                                            value: "{username_input}",
                                            oninput: move |evt| username_input.set(evt.value()),
                                            style: "width: 100%; box-sizing: border-box; padding: 8px; background: #27374D; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px; font-size: 13px;"
                                        }
                                    }
                                    div {
                                        label { style: "display: block; font-size: 12px; color: #9DB2BF; margin-bottom: 4px;", "Service Token:" }
                                        div { style: "display: flex; gap: 6px;",
                                            input {
                                                r#type: if show_token() { "text" } else { "password" },
                                                placeholder: "Service Token",
                                                value: "{token_input}",
                                                oninput: move |evt| token_input.set(evt.value()),
                                                style: "flex: 1; box-sizing: border-box; padding: 8px; background: #27374D; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px; font-size: 13px;"
                                            }
                                            button {
                                                onclick: move |_| show_token.set(!show_token()),
                                                style: "background: #27374D; border: 1px solid #526D82; color: #9DB2BF; border-radius: 4px; padding: 0 10px; cursor: pointer;",
                                                if show_token() { "👁️" } else { "🔒" }
                                            }
                                        }
                                    }
                                }

                                button {
                                    onclick: move |_| {
                                        let Some(mut updated_cfg) = config() else { return; };
                                        updated_cfg.username = Some(username_input());
                                        updated_cfg.token = Some(token_input());
                                        if let Err(e) = save_config(&updated_cfg) {
                                            config_submenu_message.set(format!("Failed to save credentials: {}", e));
                                        } else {
                                            config.set(Some(updated_cfg));
                                            config_submenu_message.set("✓ Account credentials saved successfully!".into());
                                        }
                                    },
                                    style: "background: #526D82; color: #DDE6ED; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-size: 13px; font-weight: 500;",
                                    "Save Credentials 💾"
                                }
                            }
                        }
                    }
                }
            }
        }
