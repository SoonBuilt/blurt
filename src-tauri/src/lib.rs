//! Blurt: hold a key and talk. Clean text wherever you type, or add ⇧ to ask AI.

pub mod ai;
pub mod apple;
pub mod audio;
mod engine;
mod memory;
pub mod models;
pub mod voice;
mod history;
mod hud;
pub mod insert;
pub mod keys;
pub mod settings;
pub mod stt;
pub mod text;
mod tray;

use engine::Engine;
use serde::Serialize;
use settings::{AiProvider, Settings};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    platform: &'static str,
    settings: Settings,
    model_downloaded: bool,
    tone_ready: bool,
    /// Newest first: (tone, seconds ago).
    recent_tones: Vec<(voice::tone::Tone, u64)>,
    accessibility: bool,
    microphone: &'static str,
    /// Empty when Apple Intelligence is ready.
    apple_ai: String,
    has_anthropic_key: bool,
    has_openai_key: bool,
}

#[tauri::command]
fn get_status(engine: State<Arc<Engine>>) -> Status {
    Status {
        platform: if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        },
        settings: engine.settings.read().clone(),
        model_downloaded: models::is_ready(&engine.data_dir, models::Pack::Speech),
        tone_ready: models::is_ready(&engine.data_dir, models::Pack::Tone),
        recent_tones: engine.recent_tones.lock().iter().rev().map(|(t, at)| (*t, at.elapsed().as_secs())).collect(),
        accessibility: accessibility_granted(),
        microphone: apple::mic_status(),
        apple_ai: apple::status(),
        has_anthropic_key: ai::api_key(AiProvider::Anthropic).is_some(),
        has_openai_key: ai::api_key(AiProvider::Openai).is_some(),
    }
}

#[tauri::command]
fn save_settings(engine: State<Arc<Engine>>, settings: Settings) -> Result<(), String> {
    settings::save(&engine.config_dir, &settings).map_err(|e| e.to_string())?;
    if !settings.voice.memory {
        engine.memory.clear();
    }
    *engine.settings.write() = settings;
    Ok(())
}

#[tauri::command]
async fn download_pack(app: AppHandle, engine: State<'_, Arc<Engine>>, pack: models::Pack) -> Result<(), String> {
    let engine = engine.inner().clone();
    models::download(&engine.data_dir, pack, |done, total| {
        let _ = app.emit("pack-progress", (pack, done, total));
    })
    .await
    .map_err(|e| format!("Download failed: {e}"))?;
    // Warm new models up so the first use is instant.
    engine.warm_up();
    Ok(())
}

#[tauri::command]
fn set_api_key(provider: AiProvider, key: String) -> Result<(), String> {
    ai::set_api_key(provider, &key).map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_ai(engine: State<'_, Arc<Engine>>) -> Result<String, String> {
    let (s, profile) = {
        let st = engine.settings.read();
        (st.ai.clone(), st.profile.clone())
    };
    ai::ask(&s, &profile, "Say hi to the user in one short, friendly sentence.", None, None, None)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn request_microphone() -> bool {
    tauri::async_runtime::spawn_blocking(apple::request_mic)
        .await
        .unwrap_or(false)
}

#[tauri::command]
fn history_list(engine: State<Arc<Engine>>) -> Vec<history::Entry> {
    engine.history.list()
}

#[tauri::command]
fn history_clear(engine: State<Arc<Engine>>) {
    engine.history.clear();
}

/// Puts a past result back on the clipboard, so it can be pasted wherever it's wanted.
#[tauri::command]
fn history_copy(text: String) -> Result<(), String> {
    use arboard::Clipboard;
    Clipboard::new().and_then(|mut c| c.set_text(text)).map_err(|e| e.to_string())
}

#[tauri::command]
fn request_accessibility() {
    #[cfg(target_os = "macos")]
    {
        let _ = handy_keys::open_accessibility_settings();
    }
}

#[tauri::command]
fn open_privacy_settings(pane: String) {
    #[cfg(target_os = "macos")]
    {
        let anchor = match pane.as_str() {
            "microphone" => "Privacy_Microphone",
            "ai" => "", // Apple Intelligence & Siri
            _ => "Privacy_Accessibility",
        };
        let url = if anchor.is_empty() {
            "x-apple.systempreferences:com.apple.Siri-Settings.extension".to_string()
        } else {
            format!("x-apple.systempreferences:com.apple.preference.security?{anchor}")
        };
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = pane;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "ms-settings:privacy-microphone"])
            .spawn();
    }
}

#[tauri::command]
/// The tour was finished or skipped. The window stays open; nothing else changes.
fn finish_onboarding(engine: State<Arc<Engine>>) -> Result<(), String> {
    let mut s = engine.settings.read().clone();
    s.onboarded = true;
    settings::save(&engine.config_dir, &s).map_err(|e| e.to_string())?;
    *engine.settings.write() = s;
    Ok(())
}

fn accessibility_granted() -> bool {
    #[cfg(target_os = "macos")]
    return handy_keys::check_accessibility();
    #[cfg(not(target_os = "macos"))]
    return true;
}

fn hide_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    // Back to a menu-bar-only app: no Dock icon while the window is closed.
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

/// Listens for the hold-to-talk key forever. On macOS this needs Accessibility access,
/// so it keeps retrying quietly until the user grants it during setup.
fn spawn_key_listener(engine: Arc<Engine>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let cfg_engine = engine.clone();
    std::thread::Builder::new()
        .name("blurt-keys".into())
        .spawn(move || {
            let mut told = false;
            loop {
                let tx = tx.clone();
                let (e1, e2, e3) = (cfg_engine.clone(), cfg_engine.clone(), cfg_engine.clone());
                let config = move || {
                    let s = e1.settings.read();
                    keys::KeyConfig::parse(&s.talk_key, &s.ai_modifier)
                };
                let recording = move || e2.is_recording();
                let hands_free = move || e3.settings.read().voice.hands_free;
                if let Err(err) = keys::run(tx, config, recording, hands_free) {
                    if !told {
                        log::info!("waiting for the talk key listener: {err}");
                        told = true;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        })
        .expect("spawn key thread");
    std::thread::Builder::new()
        .name("blurt-engine".into())
        .spawn(move || {
            while let Ok(action) = rx.recv() {
                engine.handle(action);
            }
        })
        .expect("spawn engine thread");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("blurt_lib=info"))
        .init();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            tray::show_main(app)
        }))
        .plugin(tauri_plugin_opener::init());
    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app.path().app_data_dir()?;
            let config_dir = app.path().app_config_dir()?;
            let engine = Engine::new(handle.clone(), data_dir, config_dir);
            app.manage(engine.clone());

            hud::create(&handle)?;
            tray::create(&handle)?;

            // Opening Blurt yourself always shows its window, so it never looks like nothing
            // happened. Only a background start (e.g. at login) stays in the menu bar.
            let background = std::env::args().any(|a| a == "--background");
            if background && engine.settings.read().onboarded {
                #[cfg(target_os = "macos")]
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            } else {
                tray::show_main(&handle);
            }

            // Load models in the background so the first use is instant.
            engine.warm_up();
            spawn_key_listener(engine);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window just hides it; Blurt keeps running in the menu bar / tray.
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    hide_main(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            save_settings,
            download_pack,
            set_api_key,
            test_ai,
            request_microphone,
            request_accessibility,
            open_privacy_settings,
            history_list,
            history_clear,
            history_copy,
            finish_onboarding
        ])
        .build(tauri::generate_context!())
        .expect("error while building Blurt")
        .run(|app, event| {
            // Clicking Blurt again (Dock, Finder, Spotlight) while it runs brings the window back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                tray::show_main(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
