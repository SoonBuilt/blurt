//! Putting text into whatever app the user is in, and reading what they've highlighted.
//!
//! Text goes in through the clipboard plus a synthetic ⌘V / Ctrl+V (fast and works in
//! nearly every app), and the previous clipboard is restored afterwards. Selections are
//! read through macOS Accessibility first (no keystrokes), falling back to a synthetic copy.

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use tauri::AppHandle;

#[derive(Clone, Copy)]
enum Chord {
    Copy,
    Paste,
}

/// How the text reached the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivered {
    /// Typed into the focused app.
    Pasted,
    /// Only copied: macOS isn't letting Blurt type yet (Accessibility is off).
    Copied,
}

/// Puts `text` into the focused app and, when `keep` is true, leaves it on the clipboard.
/// Without Accessibility access it falls back to copying, so nothing is ever lost.
pub fn deliver(app: &AppHandle, text: &str, keep: bool) -> anyhow::Result<Delivered> {
    if !can_type() {
        Clipboard::new()?.set_text(text.to_string())?;
        return Ok(Delivered::Copied);
    }
    paste(app, text, !keep)?;
    Ok(Delivered::Pasted)
}

/// Whether the OS lets Blurt send keystrokes to other apps.
pub fn can_type() -> bool {
    #[cfg(target_os = "macos")]
    return handy_keys::check_accessibility();
    #[cfg(not(target_os = "macos"))]
    return true;
}

/// Pastes `text` into the focused app. With `restore`, the user's previous clipboard
/// text comes back shortly after.
pub fn paste(app: &AppHandle, text: &str, restore: bool) -> anyhow::Result<()> {
    wait_for_modifiers_released();
    let mut cb = Clipboard::new()?;
    let saved = if restore { cb.get_text().ok() } else { None };
    cb.set_text(text.to_string())?;
    std::thread::sleep(Duration::from_millis(40));
    chord(app, Chord::Paste)?;

    if restore {
        let ours = text.to_string();
        std::thread::spawn(move || {
            // Give slow apps (Electron, remote desktops) time to read the clipboard first.
            std::thread::sleep(Duration::from_millis(700));
            if let Ok(mut cb) = Clipboard::new() {
                // Only restore if the clipboard still holds our text: if the user copied
                // something in the meantime, theirs wins.
                if cb.get_text().ok().as_deref() == Some(ours.as_str()) {
                    let _ = match saved {
                        Some(s) => cb.set_text(s),
                        None => cb.clear(),
                    };
                }
            }
        });
    }
    Ok(())
}

/// Returns the text currently highlighted in the focused app, if any.
pub fn selected_text(app: &AppHandle) -> Option<String> {
    #[cfg(target_os = "macos")]
    if let Some(t) = mac_ax::selected_text() {
        return Some(t);
    }
    copy_selection(app)
}

fn copy_selection(app: &AppHandle) -> Option<String> {
    wait_for_modifiers_released();
    let mut cb = Clipboard::new().ok()?;
    let saved = cb.get_text().ok();
    // A unique marker tells us whether the copy actually produced anything.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let marker = format!("\u{2063}blurt-{nanos}");
    cb.set_text(marker.clone()).ok()?;
    chord(app, Chord::Copy).ok()?;

    let mut got = None;
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(10));
        if let Ok(t) = cb.get_text() {
            if t != marker {
                got = Some(t);
                break;
            }
        }
    }
    let _ = match saved {
        Some(s) => cb.set_text(s),
        None => cb.clear(),
    };
    got.filter(|t| !t.trim().is_empty())
}

/// Sends the chord on the main thread (macOS input APIs expect it).
fn chord(app: &AppHandle, which: Chord) -> anyhow::Result<()> {
    let (tx, rx) = channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(send_chord(which));
    })?;
    rx.recv_timeout(Duration::from_secs(2))?
}

fn send_chord(which: Chord) -> anyhow::Result<()> {
    let mut e = Enigo::new(&Settings::default())?;
    // Raw key codes avoid keyboard-layout lookups (which must run on the main thread on
    // macOS and misbehave on some layouts).
    #[cfg(target_os = "macos")]
    let (modifier, key) = (
        Key::Meta,
        Key::Other(match which {
            Chord::Copy => 8,
            Chord::Paste => 9,
        }),
    );
    #[cfg(target_os = "windows")]
    let (modifier, key) = (
        Key::Control,
        Key::Other(match which {
            Chord::Copy => 0x43,
            Chord::Paste => 0x56,
        }),
    );
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let (modifier, key) = (
        Key::Control,
        Key::Unicode(match which {
            Chord::Copy => 'c',
            Chord::Paste => 'v',
        }),
    );

    e.key(modifier, Direction::Press)?;
    std::thread::sleep(Duration::from_millis(8));
    e.key(key, Direction::Click)?;
    std::thread::sleep(Duration::from_millis(8));
    e.key(modifier, Direction::Release)?;
    Ok(())
}

/// Waits (briefly) until the user has let go of ⇧/⌥/⌃/⌘, so our ⌘V isn't turned into ⌘⇧V.
fn wait_for_modifiers_released() {
    let start = Instant::now();
    while modifiers_down() && start.elapsed() < Duration::from_millis(600) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "macos")]
fn modifiers_down() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceFlagsState(state_id: i32) -> u64;
    }
    const HID_SYSTEM_STATE: i32 = 1;
    const MASK: u64 = 0x0002_0000 | 0x0004_0000 | 0x0008_0000 | 0x0010_0000; // shift, ctrl, alt, cmd
    unsafe { CGEventSourceFlagsState(HID_SYSTEM_STATE) & MASK != 0 }
}

#[cfg(target_os = "windows")]
fn modifiers_down() -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(vkey: i32) -> i16;
    }
    // VK_SHIFT, VK_CONTROL, VK_MENU (Alt), VK_LWIN, VK_RWIN
    [0x10, 0x11, 0x12, 0x5B, 0x5C]
        .iter()
        .any(|&vk| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn modifiers_down() -> bool {
    false
}

#[cfg(target_os = "macos")]
mod mac_ax {
    //! Reads the focused element's selected text through the Accessibility API.
    use std::ffi::c_void;

    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> CFTypeRef;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: CFTypeRef);
        fn CFGetTypeID(cf: CFTypeRef) -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFStringCreateWithBytes(
            alloc: CFTypeRef,
            bytes: *const u8,
            len: isize,
            encoding: u32,
            external: u8,
        ) -> CFStringRef;
        fn CFStringGetLength(s: CFStringRef) -> isize;
        fn CFStringGetCString(s: CFStringRef, buf: *mut u8, size: isize, encoding: u32) -> u8;
    }
    const UTF8: u32 = 0x0800_0100;

    unsafe fn cfstr(s: &str) -> CFStringRef {
        CFStringCreateWithBytes(std::ptr::null(), s.as_ptr(), s.len() as isize, UTF8, 0)
    }

    unsafe fn copy_attr(el: CFTypeRef, name: &str) -> Option<CFTypeRef> {
        let attr = cfstr(name);
        let mut out: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(el, attr, &mut out);
        CFRelease(attr);
        (err == 0 && !out.is_null()).then_some(out)
    }

    pub fn selected_text() -> Option<String> {
        unsafe {
            let system = AXUIElementCreateSystemWide();
            if system.is_null() {
                return None;
            }
            let focused = copy_attr(system, "AXFocusedUIElement");
            CFRelease(system);
            let focused = focused?;
            let value = copy_attr(focused, "AXSelectedText");
            CFRelease(focused);
            let value = value?;
            let text = if CFGetTypeID(value) == CFStringGetTypeID() {
                let len = CFStringGetLength(value);
                let cap = len * 4 + 1;
                let mut buf = vec![0u8; cap as usize];
                (CFStringGetCString(value, buf.as_mut_ptr(), cap, UTF8) != 0).then(|| {
                    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
                    String::from_utf8_lossy(&buf[..end]).into_owned()
                })
            } else {
                None
            };
            CFRelease(value);
            text.filter(|t| !t.trim().is_empty())
        }
    }
}
