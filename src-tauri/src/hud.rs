//! The listening bar: a small floating window with Tally, shown at the bottom of the
//! screen the user is working on. It never takes focus, so the app they're typing in
//! stays active. On macOS it's a non-activating NSPanel that also floats over
//! full-screen apps.

use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl};

pub const LABEL: &str = "hud";
const WIDTH: f64 = 520.0;
const HEIGHT: f64 = 240.0;
/// Gap between the bar and the bottom of the screen's usable area (above the Dock/taskbar).
const BOTTOM_GAP: f64 = 28.0;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum HudState {
    Hidden,
    /// Holding the key (or hands-free); `ai` is true once ⇧ joins.
    Listening { ai: bool, context_words: usize, hands_free: bool },
    Transcribing { ai: bool },
    Thinking { instruction: String, context_words: usize, tone: Option<crate::voice::tone::Tone> },
    Done { message: String },
    Error { message: String },
}

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(HudPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true
        }
    })
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::{CollectionBehavior, PanelBuilder, PanelLevel, StyleMask};
        let panel = PanelBuilder::<_, HudPanel>::new(app, LABEL)
            .url(WebviewUrl::App("hud.html".into()))
            .title("Blurt")
            .level(PanelLevel::Status)
            .size(tauri::Size::Logical(tauri::LogicalSize {
                width: WIDTH,
                height: HEIGHT,
            }))
            .has_shadow(false)
            .transparent(true)
            .no_activate(true)
            .corner_radius(0.0)
            .style_mask(StyleMask::empty().borderless().nonactivating_panel())
            .with_window(|w| w.decorations(false).transparent(true).focusable(false))
            .collection_behavior(
                CollectionBehavior::new()
                    .can_join_all_spaces()
                    .full_screen_auxiliary(),
            )
            .build()
            .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!("{e:?}")))?;
        panel.set_ignores_mouse_events(true);
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let w = tauri::WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("hud.html".into()))
            .title("Blurt")
            .inner_size(WIDTH, HEIGHT)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focusable(false)
            .focused(false)
            .visible(false)
            .build()?;
        let _ = w.set_ignore_cursor_events(true);
    }
    Ok(())
}

pub fn set(app: &AppHandle, state: HudState) {
    let _ = app.emit_to(LABEL, "hud", &state);
    let show = !matches!(state, HudState::Hidden);
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if show {
            place(&app2);
            show_window(&app2);
        } else {
            hide_window(&app2);
        }
    });
}

pub fn level(app: &AppHandle, level: f32) {
    let _ = app.emit_to(LABEL, "level", level);
}

/// Puts the bar at the bottom centre of whichever screen the mouse is on.
fn place(app: &AppHandle) {
    let Some(w) = app.get_webview_window(LABEL) else {
        return;
    };
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| w.primary_monitor().ok().flatten());
    let Some(m) = monitor else { return };
    let scale = m.scale_factor();
    let area = m.work_area();
    let (x, y) = (
        area.position.x as f64 / scale,
        area.position.y as f64 / scale,
    );
    let (aw, ah) = (
        area.size.width as f64 / scale,
        area.size.height as f64 / scale,
    );
    let pos = LogicalPosition::new(x + (aw - WIDTH) / 2.0, y + ah - HEIGHT - BOTTOM_GAP);
    let _ = w.set_position(pos);
}

fn show_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(p) = app.get_webview_panel(LABEL) {
            p.order_front_regardless();
            p.show();
        }
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.show();
    }
}

fn hide_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(p) = app.get_webview_panel(LABEL) {
            p.hide();
        }
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
}
