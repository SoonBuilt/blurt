//! Apple Intelligence (on-device) through the Swift bridge in `swift/BlurtApple.swift`.
//! On other platforms these report "unavailable".

#[cfg(target_os = "macos")]
mod ffi {
    use std::os::raw::c_char;
    extern "C" {
        pub fn blurt_ai_available() -> i32;
        pub fn blurt_ai_status() -> *mut c_char;
        pub fn blurt_ai_complete(
            system: *const c_char,
            user: *const c_char,
            ok: *mut i32,
        ) -> *mut c_char;
        pub fn blurt_free(ptr: *mut c_char);
        pub fn blurt_mic_status() -> i32;
        pub fn blurt_mic_request() -> i32;
    }
}

#[cfg(target_os = "macos")]
fn take(ptr: *mut std::os::raw::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let s = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { ffi::blurt_free(ptr) };
    s
}

pub fn available() -> bool {
    #[cfg(target_os = "macos")]
    return unsafe { ffi::blurt_ai_available() } == 1;
    #[cfg(not(target_os = "macos"))]
    return false;
}

/// Empty when Apple Intelligence is ready, otherwise a plain-English reason.
pub fn status() -> String {
    #[cfg(target_os = "macos")]
    return take(unsafe { ffi::blurt_ai_status() });
    #[cfg(not(target_os = "macos"))]
    return "Apple Intelligence is only on Mac.".into();
}

pub async fn complete(system: &str, user: &str) -> anyhow::Result<String> {
    #[cfg(target_os = "macos")]
    {
        let system = std::ffi::CString::new(system.replace('\0', ""))?;
        let user = std::ffi::CString::new(user.replace('\0', ""))?;
        return tokio::task::spawn_blocking(move || {
            let mut ok = 0i32;
            let out =
                take(unsafe { ffi::blurt_ai_complete(system.as_ptr(), user.as_ptr(), &mut ok) });
            if ok == 1 {
                Ok(out)
            } else {
                Err(anyhow::anyhow!(out))
            }
        })
        .await?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (system, user);
        anyhow::bail!("Apple Intelligence is only on Mac. Pick another AI engine in Settings.")
    }
}

/// "allowed", "denied", "restricted" or "unknown" (not asked yet). Windows manages this in
/// its own privacy settings, so we report "allowed" there and surface errors when recording.
pub fn mic_status() -> &'static str {
    #[cfg(target_os = "macos")]
    return match unsafe { ffi::blurt_mic_status() } {
        3 => "allowed",
        2 => "denied",
        1 => "restricted",
        _ => "unknown",
    };
    #[cfg(not(target_os = "macos"))]
    return "allowed";
}

/// Shows the system prompt the first time; blocks until answered.
pub fn request_mic() -> bool {
    #[cfg(target_os = "macos")]
    return unsafe { ffi::blurt_mic_request() } == 1;
    #[cfg(not(target_os = "macos"))]
    return true;
}
