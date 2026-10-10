use dioxus::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use flauncher_core::{get_profiles, get_profiles_dir, create_profile_dir, launch_game};

fn main() {
    launch(App);
}

#[component]
fn App() -> Element {
    let mut new_profile_name = use_signal(|| String::new());
    let mut profiles = use_signal(|| get_profiles());
    let mut status_message = use_signal(|| String::new());

    let mut refresh_profiles = move || {
        profiles.set(get_profiles());
    };

    rsx! {
        div { style: "padding: 30px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: auto;",
            h1 { style: "color: #e67e22; margin-bottom: 5px;", "Factorio Profile Manager" }
            p { style: "color: #7f8c8d; font-size: 14px; margin-top: 0;", "Manage isolated mod and save profiles for macOS" }

            div { style: "background: #f8f9fa; padding: 15px; border-radius: 8px; margin-bottom: 20px; border: 1px solid #e9ecef;",
                h3 { style: "margin-top: 0; font-size: 16px;", "Create New Profile" }
                div { style: "display: flex; gap: 10px;",
                    input {
                        placeholder: "Profile Name",
                        value: "{new_profile_name}",
                        oninput: move |evt| new_profile_name.set(evt.value()),
                        style: "flex: 1; padding: 8px; border: 1px solid #ced4da; border-radius: 4px;"
                    }
                    button {
                        onclick: move |_| {
                            let name = new_profile_name();
                            match create_profile_dir(&name) {
                                Ok(_) => {
                                    status_message.set(format!("Created profile '{}'", name));
                                    new_profile_name.set(String::new());
                                    refresh_profiles();
                                }
                                Err(e) => status_message.set(format!("Error: {}", e)),
                            }
                        },
                        style: "background: #2ecc71; color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-weight: bold;",
                        "Create"
                    }
                }
            }

            if !status_message().is_empty() {
                div { style: "padding: 10px; background: #e8f4fd; color: #1d6fa5; border-radius: 4px; margin-bottom: 15px; font-size: 14px;",
                    "{status_message}"
                }
            }

            h3 { style: "font-size: 16px;", "Existing Profiles" }
            if profiles().is_empty() {
                p { style: "color: #95a5a6; font-style: italic;", "No profiles created yet. Create one above!" }
            } else {
                ul { style: "list-style: none; padding: 0;",
                    for profile in profiles() {
                        li { style: "display: flex; justify-content: space-between; align-items: center; background: #ffffff; border: 1px solid #e9ecef; padding: 12px; border-radius: 6px; margin-bottom: 8px;",
                            span { style: "font-weight: 500;", "{profile}" }
                            div { style: "display: flex; gap: 8px;",
                                {
                                    let p_launch = profile.clone();
                                    let p_folder = profile.clone();
                                    rsx! {
                                        button {
                                            onclick: move |_| {
                                                match launch_game(&p_launch) {
                                                    Ok(_) => status_message.set(format!("Launched Factorio with profile '{}'", p_launch)),
                                                    Err(e) => status_message.set(e),
                                                }
                                            },
                                            style: "background: #3498db; color: white; border: none; padding: 6px 12px; border-radius: 4px; cursor: pointer;",
                                            "Launch Game"
                                        }
                                        button {
                                            onclick: move |_| {
                                                let path = get_profiles_dir().join(&p_folder);
                                                let _ = Command::new("open").arg(path).spawn();
                                            },
                                            style: "background: #95a5a6; color: white; border: none; padding: 6px 12px; border-radius: 4px; cursor: pointer;",
                                            "Open Folder"
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