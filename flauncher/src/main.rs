use dioxus::prelude::*;
use flauncher_core::{
    create_profile_dir, find_factorio_installation, get_profiles, get_profiles_dir, launch_game,
    load_config, save_config,
};
use std::process::Command;

fn main() {
    launch(App);
}

#[component]
fn App() -> Element {
    let mut config = use_signal(|| load_config());
    let mut setup_error = use_signal(|| String::new());

    let mut new_profile_name = use_signal(|| String::new());
    let mut profiles = use_signal(|| get_profiles());
    let mut status_message = use_signal(|| String::new());

    let mut refresh_profiles = move || {
        profiles.set(get_profiles());
    };

    // --- FIRST-TIME SETUP SCREEN ---
    if config().is_none() {
        return rsx! {
            style { "body {{ background: #27374D; margin: 0; overflow: hidden; }}" }
            div { style: "width: 100vw; height: 100vh; margin: 0; padding: 40px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; box-sizing: border-box; background: #27374D; color: #DDE6ED; display: flex; flex-direction: column; align-items: center; justify-content: center;",
                div { style: "max-width: 500px; width: 100%; text-align: center;",
                    h2 { style: "color: #9DB2BF; margin-bottom: 5px;", "Welcome to Factorio Profile Manager" }
                    p { style: "color: #9DB2BF; font-size: 14px; margin-bottom: 25px;", 
                        "Please locate your Factorio installation folder using the button below." 
                    }

                    button {
                        onclick: move |_| {
                            if let Some(folder_path) = rfd::FileDialog::new().pick_folder() {
                                match find_factorio_installation(&folder_path) {
                                    Ok(cfg) => {
                                        if let Err(e) = save_config(&cfg) {
                                            setup_error.set(format!("Failed to save config: {}", e));
                                        } else {
                                            config.set(Some(cfg));
                                        }
                                    }
                                    Err(e) => {
                                        setup_error.set(format!(
                                            "Invalid folder ({}). Make sure you select the root Factorio folder!",
                                            e
                                        ))
                                    }
                                }
                            }
                        },
                        style: "background: #526D82; color: #DDE6ED; border: none; padding: 12px 24px; border-radius: 6px; cursor: pointer; font-weight: bold; font-size: 15px;",
                        "Browse for Factorio Folder 📂"
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

    let cfg = config().unwrap();

    // --- MAIN APPLICATION SCREEN ---
    rsx! {
        style { "body {{ background: #27374D; margin: 0; overflow: hidden; }}" }
        div { style: "width: 100vw; height: 100vh; margin: 0; padding: 40px 20px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; box-sizing: border-box; background: #27374D; color: #DDE6ED; display: flex; justify-content: center; overflow-y: auto;",
            div { style: "max-width: 600px; width: 100%;",
                h1 { style: "color: #DDE6ED; margin-bottom: 5px;", "Factorio Profile Manager" }
                p { style: "color: #9DB2BF; font-size: 14px; margin-top: 0;", "Manage isolated mod and save profiles for Factorio" }

                div { style: "background: #1b263b; padding: 15px; border-radius: 8px; margin-bottom: 20px; border: 1px solid #526D82;",
                    h3 { style: "margin-top: 0; font-size: 16px; color: #DDE6ED;", "Create New Profile" }
                    div { style: "display: flex; gap: 10px;",
                        input {
                            placeholder: "Profile Name",
                            value: "{new_profile_name}",
                            oninput: move |evt| new_profile_name.set(evt.value()),
                            style: "flex: 1; padding: 8px; background: #27374D; color: #DDE6ED; border: 1px solid #526D82; border-radius: 4px;"
                        }
                        button {
                            onclick: move |_| {
                                let name = new_profile_name();
                                match create_profile_dir(&name, &cfg) {
                                    Ok(_) => {
                                        status_message.set(format!("Created profile '{}'", name));
                                        new_profile_name.set(String::new());
                                        refresh_profiles();
                                    }
                                    Err(e) => status_message.set(format!("Error: {}", e)),
                                }
                            },
                            style: "background: #526D82; color: #DDE6ED; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-weight: bold;",
                            "Create"
                        }
                    }
                }

                if !status_message().is_empty() {
                    div { style: "padding: 10px; background: #1b263b; color: #9DB2BF; border: 1px solid #526D82; border-radius: 4px; margin-bottom: 15px; font-size: 14px;",
                        "{status_message}"
                    }
                }

                h3 { style: "font-size: 16px; color: #DDE6ED;", "Existing Profiles" }
                if profiles().is_empty() {
                    p { style: "color: #9DB2BF; font-style: italic;", "No profiles created yet. Create one above!" }
                } else {
                    ul { style: "list-style: none; padding: 0;",
                        for profile in profiles() {
                            li { style: "display: flex; justify-content: space-between; align-items: center; background: #1b263b; border: 1px solid #526D82; padding: 12px; border-radius: 6px; margin-bottom: 8px;",
                                span { style: "font-weight: 500; color: #DDE6ED;", "{profile}" }
                                div { style: "display: flex; gap: 8px;",
                                    {
                                        let p_launch = profile.clone();
                                        let p_folder = profile.clone();
                                        let active_cfg = cfg.clone();
                                        rsx! {
                                            button {
                                                onclick: move |_| {
                                                    match launch_game(&p_launch, &active_cfg) {
                                                        Ok(_) => status_message.set(format!("Launched Factorio with profile '{}'", p_launch)),
                                                        Err(e) => status_message.set(e),
                                                    }
                                                },
                                                style: "background: #526D82; color: #DDE6ED; border: none; padding: 6px 12px; border-radius: 4px; cursor: pointer;",
                                                "Launch Game"
                                            }
                                            button {
                                                onclick: move |_| {
                                                    let path = get_profiles_dir().join(&p_folder);
                                                    #[cfg(target_os = "windows")]
                                                    let _ = Command::new("explorer.exe").arg(path).spawn();
                                                    #[cfg(not(target_os = "windows"))]
                                                    let _ = Command::new("open").arg(path).spawn();
                                                },
                                                style: "background: #27374D; color: #9DB2BF; border: 1px solid #526D82; padding: 6px 12px; border-radius: 4px; cursor: pointer;",
                                                "Open Folder 📁"
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