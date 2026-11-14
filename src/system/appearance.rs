//! The desktop's light or dark setting, for the "System" mode. Windows and macOS tell the window
//! (winit's `theme()`); on Linux winit says nothing on X11 and only echoes what the app set on
//! Wayland, so the setting is read from the desktop portal (`org.freedesktop.appearance
//! color-scheme`) by a thread that wakes the window when it changes.

use egui::Theme;
use std::sync::atomic::{AtomicU8, Ordering};

/// The portal's `color-scheme` last heard (0 no preference or never heard, 1 dark, 2 light).
static SCHEME: AtomicU8 = AtomicU8::new(0);

/// What the portal's `color-scheme` value says: 1 prefers dark, 2 prefers light, 0 has no preference.
pub fn scheme_theme(value: u32) -> Option<Theme> {
    match value {
        1 => Some(Theme::Dark),
        2 => Some(Theme::Light),
        _ => None,
    }
}

/// The desktop's own word first (the portal), then the window system's.
fn pick(desktop: Option<Theme>, window: Option<Theme>) -> Option<Theme> {
    desktop.or(window)
}

/// Remembers what the portal said; whether it differs from before.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn remember(value: u32) -> bool {
    let value = u8::try_from(value).unwrap_or(0);
    SCHEME.swap(value, Ordering::Relaxed) != value
}

/// The theme the desktop is set to, if it says: what the "System" mode follows. On Linux the first
/// call starts the thread that listens to the portal; until it answers, and on a desktop with no
/// portal, this is the window system's word, and the "System" mode falls back to dark.
pub fn system_theme(ctx: &egui::Context) -> Option<Theme> {
    #[cfg(target_os = "linux")]
    portal::watch(ctx);
    pick(
        scheme_theme(u32::from(SCHEME.load(Ordering::Relaxed))),
        ctx.system_theme(),
    )
}

/// The desktop portal's Settings (org.freedesktop.portal.Settings), over zbus.
#[cfg(target_os = "linux")]
mod portal {
    use super::remember;
    use std::sync::{Mutex, Once};
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{OwnedValue, Value};

    const NAMESPACE: &str = "org.freedesktop.appearance";
    const KEY: &str = "color-scheme";

    /// The newest window to wake when the setting changes (a window made again carries a new context).
    static WAKE: Mutex<Option<egui::Context>> = Mutex::new(None);
    static STARTED: Once = Once::new();

    /// Starts the listening thread once and keeps `ctx` as the window to wake.
    pub fn watch(ctx: &egui::Context) {
        if let Ok(mut wake) = WAKE.lock()
            && wake.as_ref() != Some(ctx)
        {
            *wake = Some(ctx.clone());
        }
        STARTED.call_once(|| {
            let _ = std::thread::Builder::new()
                .name("appearance".into())
                .spawn(listen);
        });
    }

    fn wake() {
        if let Ok(wake) = WAKE.lock()
            && let Some(ctx) = wake.as_ref()
        {
            ctx.request_repaint();
        }
    }

    /// The `color-scheme` number in a portal value. `ReadOne` and `SettingChanged` carry it in one
    /// variant; the older `Read` wraps it in a second. A value of another kind says nothing (0).
    fn scheme_of(value: &Value) -> u32 {
        match value {
            Value::Value(inner) => scheme_of(inner),
            Value::U32(n) => *n,
            _ => 0,
        }
    }

    /// The setting now: `ReadOne`, or `Read` on a portal too old for it.
    fn read(settings: &Proxy) -> Option<u32> {
        let args = &(NAMESPACE, KEY);
        let value: OwnedValue = settings
            .call("ReadOne", args)
            .or_else(|_| settings.call("Read", args))
            .ok()?;
        Some(scheme_of(&value))
    }

    /// Reads the setting, then follows its changes for the life of the process. A desktop with no
    /// session bus or no portal leaves the setting unheard, and the "System" mode dark.
    fn listen() {
        let Ok(bus) = Connection::session() else {
            return;
        };
        let Ok(settings) = Proxy::new(
            &bus,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        ) else {
            return;
        };
        // Listening before reading, so a change in between is not missed.
        let Ok(changes) = settings.receive_signal("SettingChanged") else {
            return;
        };
        if let Some(scheme) = read(&settings)
            && remember(scheme)
        {
            wake();
        }
        for message in changes {
            let Ok((namespace, key, value)) =
                message.body().deserialize::<(String, String, OwnedValue)>()
            else {
                continue;
            };
            if namespace == NAMESPACE && key == KEY && remember(scheme_of(&value)) {
                wake();
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::{Theme, scheme_theme};
        use super::*;

        #[test]
        fn a_portal_value_is_read_through_one_variant_or_two() {
            assert_eq!(scheme_of(&Value::U32(2)), 2);
            assert_eq!(scheme_of(&Value::Value(Box::new(Value::U32(1)))), 1);
            // No preference, and a value of another kind: nothing to say.
            assert_eq!(scheme_of(&Value::U32(0)), 0);
            assert_eq!(scheme_of(&Value::Str("dark".into())), 0);
            assert_eq!(scheme_theme(scheme_of(&Value::U32(2))), Some(Theme::Light));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_portal_scheme_maps_to_a_theme() {
        assert_eq!(scheme_theme(1), Some(Theme::Dark));
        assert_eq!(scheme_theme(2), Some(Theme::Light));
        // 0 is "no preference" and anything else is a value from the future: say nothing.
        assert_eq!(scheme_theme(0), None);
        assert_eq!(scheme_theme(3), None);
    }

    #[test]
    fn the_desktop_beats_a_window_system_that_only_echoes() {
        let light = Some(Theme::Light);
        let dark = Some(Theme::Dark);
        assert_eq!(pick(light, dark), light);
        assert_eq!(pick(dark, light), dark);
        assert_eq!(pick(None, dark), dark);
        assert_eq!(pick(light, None), light);
        assert_eq!(pick(None, None), None);
    }
}
