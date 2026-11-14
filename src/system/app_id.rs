//! The identity Windows groups the app's taskbar buttons under: one AppUserModelID for every
//! process (the resident, the editors, the pinned pictures, the capture), the one the setup
//! program writes into the shortcuts it makes.

/// The packager's `identifier` (`[package.metadata.packager]` in `Cargo.toml`). The NSIS setup
/// writes it into its desktop and Start menu shortcuts as their AppUserModelID, and the MSI into
/// its Start menu shortcut only (cargo-packager's template leaves the desktop one without).
#[cfg_attr(not(windows), allow(dead_code))]
pub const APP_ID: &str = "br.com.takeflow.takeshot";

/// Names this process as that app. Without it a process the shell did not start from a shortcut
/// has the id Windows derives from the executable's path, which is not the shortcut's id, so the
/// window of an editor or a pinned picture can get a button apart from the pinned shortcut.
/// Called before any window exists.
#[cfg(windows)]
pub fn set() {
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    let wide: Vec<u16> = APP_ID.encode_utf16().chain(Some(0)).collect();
    // SAFETY: `wide` is a NUL-terminated UTF-16 string that lives through the call.
    let hr = unsafe { SetCurrentProcessExplicitAppUserModelID(wide.as_ptr()) };
    if hr != 0 {
        crate::system::log::line(format_args!("app id not set: {hr:#x}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_id_is_the_identifier_the_installer_writes_into_the_shortcuts() {
        let manifest = include_str!("../../Cargo.toml");
        assert!(
            manifest
                .lines()
                .any(|l| l.trim() == format!("identifier = \"{APP_ID}\"")),
            "`identifier` in [package.metadata.packager] is not {APP_ID}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn the_process_answers_to_the_app_id_once_it_is_set() {
        use windows_sys::Win32::System::Com::CoTaskMemFree;
        use windows_sys::Win32::UI::Shell::GetCurrentProcessExplicitAppUserModelID;
        let current = || -> Option<String> {
            let mut id = std::ptr::null_mut();
            // SAFETY: `id` is a valid out pointer; a returned string is freed below.
            let hr = unsafe { GetCurrentProcessExplicitAppUserModelID(&mut id) };
            if hr != 0 || id.is_null() {
                return None;
            }
            // SAFETY: on success `id` is a NUL-terminated UTF-16 string the shell allocated.
            let text = unsafe {
                let len = (0..).take_while(|&i| *id.add(i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(id, len))
            };
            // SAFETY: the shell allocates it with CoTaskMemAlloc.
            unsafe { CoTaskMemFree(id as *const _) };
            Some(text)
        };
        assert_ne!(current().as_deref(), Some(APP_ID), "the id is not set yet");
        set();
        assert_eq!(current().as_deref(), Some(APP_ID));
    }
}
