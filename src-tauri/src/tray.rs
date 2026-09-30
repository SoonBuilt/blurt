//! Tally in the menu bar (macOS) / system tray (Windows). Tally's face follows what
//! Blurt is doing: hello when idle, listening, working, ready, and needs-you.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const ID: &str = "blurt";

#[derive(Debug, Clone, Copy)]
pub enum Mood {
    Hello,
    Listening,
    Working,
    Ready,
    Needs,
}

fn icon(mood: Mood) -> Image<'static> {
    let bytes: &'static [u8] = match mood {
        Mood::Hello => include_bytes!("../icons/tray/hello.png"),
        Mood::Listening => include_bytes!("../icons/tray/listening.png"),
        Mood::Working => include_bytes!("../icons/tray/working.png"),
        Mood::Ready => include_bytes!("../icons/tray/ready.png"),
        Mood::Needs => include_bytes!("../icons/tray/needs.png"),
    };
    Image::from_bytes(bytes).expect("valid tray png")
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let talk = if cfg!(target_os = "macos") {
        "Hold Right ⌥ to dictate · add ⇧ to ask AI"
    } else {
        "Hold Right Ctrl to dictate · add Shift to ask AI"
    };
    let hint = MenuItem::with_id(app, "hint", talk, false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let quit = MenuItem::with_id(app, "quit", "Quit Blurt", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(
        app,
        &[&hint, &PredefinedMenuItem::separator(app)?, &open, &quit],
    )?;

    TrayIconBuilder::with_id(ID)
        .icon(icon(Mood::Hello))
        .tooltip("Blurt")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            // On Windows, a left click on the tray icon opens Blurt.
            if cfg!(target_os = "windows") {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = e
                {
                    show_main(tray.app_handle());
                }
            }
        })
        .build(app)?;
    Ok(())
}

pub fn set_mood(app: &AppHandle, mood: Mood) {
    if let Some(tray) = app.tray_by_id(ID) {
        let _ = tray.set_icon(Some(icon(mood)));
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
}
