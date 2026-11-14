//! Start with the system: the OS launches `takeshot --minimized` at login (Run key, LaunchAgent, XDG autostart).

use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use std::path::PathBuf;

/// The program the system should start for an entry it keeps (the login entry, a desktop
/// shortcut): the running file, except inside an AppImage, whose running file lives in a mount
/// that goes when it closes; then the AppImage itself (`$APPIMAGE`).
pub fn launcher(running: PathBuf, appimage: Option<std::ffi::OsString>) -> PathBuf {
    appimage
        .filter(|a| !a.is_empty())
        .map(PathBuf::from)
        .unwrap_or(running)
}

/// `launcher` for this process.
pub fn program() -> Result<PathBuf, String> {
    let running = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(launcher(running, std::env::var_os("APPIMAGE")))
}

/// A program path as the `Exec` line of a desktop entry takes it. A path with no reserved
/// character comes out as it is; otherwise it is wrapped in double quotes, with `"`, `` ` ``, `$`
/// and `\` escaped by a backslash and every backslash doubled again, because the file format
/// unescapes string values before the line is split. A `%` starts a field code, so a literal one
/// is written `%%`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn desktop_exec_path(path: &str) -> String {
    const RESERVED: &[char] = &[
        ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(',
        ')', '`',
    ];
    let path = path.replace('%', "%%");
    if !path.contains(RESERVED) {
        return path;
    }
    let mut quoted = String::from("\"");
    for c in path.chars() {
        match c {
            '"' | '`' | '$' => {
                quoted.push_str(r"\\");
                quoted.push(c);
            }
            '\\' => quoted.push_str(r"\\\\"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// Whether the text of an autostart entry would start at login. The desktop (and tools such as
/// GNOME Tweaks) turn an entry off and leave the file, with `Hidden=true` or
/// `X-GNOME-Autostart-enabled=false` in the `[Desktop Entry]` group.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn desktop_entry_is_on(text: &str) -> bool {
    let mut in_entry = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if let Some((key, value)) = line.split_once('=').filter(|_| in_entry) {
            let value = value.trim().to_ascii_lowercase();
            match key.trim() {
                "Hidden" if value == "true" => return false,
                "X-GNOME-Autostart-enabled" if value == "false" => return false,
                _ => {}
            }
        }
    }
    true
}

/// The login entry for this executable; nothing changes on the system until `set_autostart`.
pub fn autostart() -> Result<AutoLaunch, String> {
    let exe = program()?;
    // The Run value is "path args": quote the path, which may have spaces ("C:\\Program Files").
    #[cfg(windows)]
    let path = format!("\"{}\"", exe.display());
    // The desktop entry's `Exec` line splits on spaces unless the path is quoted its own way.
    #[cfg(target_os = "linux")]
    let path = desktop_exec_path(&exe.to_string_lossy());
    #[cfg(not(any(windows, target_os = "linux")))]
    let path = exe.to_string_lossy().into_owned();
    let mut b = AutoLaunchBuilder::new();
    b.set_app_name("Takeshot")
        .set_app_path(&path)
        .set_args(&["--minimized"]);
    #[cfg(windows)]
    b.set_windows_enable_mode(auto_launch::WindowsEnableMode::CurrentUser);
    #[cfg(target_os = "macos")]
    b.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
    #[cfg(target_os = "linux")]
    b.set_linux_launch_mode(auto_launch::LinuxLaunchMode::XdgAutostart);
    b.build().map_err(|e| e.to_string())
}

/// Adds or removes the login entry.
pub fn set_autostart(on: bool) -> Result<(), String> {
    let a = autostart()?;
    if on { a.enable() } else { a.disable() }.map_err(|e| e.to_string())
}

/// Whether the login entry is on, read from the system each time (the switch shows the truth). On
/// Linux the file existing is not enough: the desktop can turn an entry off and leave it there.
pub fn autostart_enabled() -> bool {
    #[cfg(target_os = "linux")]
    return entry_file().is_some_and(|file| {
        std::fs::read_to_string(file).is_ok_and(|text| desktop_entry_is_on(&text))
    });
    #[cfg(not(target_os = "linux"))]
    autostart().is_ok_and(|a| a.is_enabled().unwrap_or(false))
}

/// Where `auto-launch` writes the entry: `~/.config/autostart/<name>.desktop`.
#[cfg(target_os = "linux")]
fn entry_file() -> Option<PathBuf> {
    let name = autostart().ok()?.get_app_name().to_owned();
    let dir = dirs::home_dir()?.join(".config/autostart");
    Some(dir.join(format!("{name}.desktop")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_appimage_hands_the_system_the_appimage_itself() {
        let mounted = PathBuf::from("/tmp/.mount_takeshAbc/usr/bin/takeshot");
        let image = std::ffi::OsString::from("/home/ana/Apps/takeshot.AppImage");
        assert_eq!(
            launcher(mounted.clone(), Some(image)),
            PathBuf::from("/home/ana/Apps/takeshot.AppImage")
        );
        assert_eq!(launcher(mounted.clone(), None), mounted);
        assert_eq!(launcher(mounted.clone(), Some("".into())), mounted);
    }

    /// What a desktop entry reader makes of an `Exec` value, from the Desktop Entry Specification:
    /// the file's string escapes first (`\\` is one backslash), then the arguments, split on spaces
    /// outside double quotes, where a backslash escapes the next character, and `%%` is a percent.
    /// It is the oracle for `desktop_exec_path`, which must not be checked against itself.
    fn exec_arguments(value: &str) -> Vec<String> {
        let value = value.replace("\\\\", "\\");
        let (mut args, mut current, mut quoted, mut started) =
            (vec![], String::new(), false, false);
        let mut chars = value.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => (quoted, started) = (!quoted, true),
                '\\' if quoted => {
                    current.push(chars.next().expect("a character after the backslash"))
                }
                '%' => {
                    assert_eq!(chars.next(), Some('%'), "a field code in {value}");
                    current.push('%');
                    started = true;
                }
                ' ' if !quoted => {
                    if started {
                        args.push(std::mem::take(&mut current));
                    }
                    started = false;
                }
                c => (current.push(c), started = true).1,
            }
        }
        assert!(!quoted, "an unclosed quote in {value}");
        if started {
            args.push(current);
        }
        args
    }

    #[test]
    fn the_exec_path_survives_the_reader_of_a_desktop_entry() {
        // The common case stays what it was: no quotes around a path that needs none.
        assert_eq!(desktop_exec_path("/usr/bin/takeshot"), "/usr/bin/takeshot");
        for path in [
            "/usr/bin/takeshot",
            "/home/ana/My Apps/takeshot.AppImage",
            "/opt/cost$/take`shot",
            "/opt/\"quoted\"/takeshot",
            "/opt/back\\slash/takeshot",
            "/opt/100%/takeshot",
            "/opt/100% sure/takeshot",
            "/opt/it's mine/ta keshot (1)",
            "/opt/tab\there/takeshot",
        ] {
            let line = format!("{} --minimized", desktop_exec_path(path));
            assert_eq!(exec_arguments(&line), [path, "--minimized"], "{line}");
        }
    }

    #[test]
    fn an_entry_the_desktop_turned_off_is_not_on() {
        let entry = |extra: &str| {
            format!(
                "[Desktop Entry]\nType=Application\nName=Takeshot\nExec=/opt/takeshot --minimized\n{extra}"
            )
        };
        assert!(desktop_entry_is_on(&entry("")));
        assert!(desktop_entry_is_on(&entry(
            "Hidden=false\nX-GNOME-Autostart-enabled=true\n"
        )));
        assert!(!desktop_entry_is_on(&entry("Hidden=true\n")));
        assert!(!desktop_entry_is_on(&entry("Hidden = True\n")));
        assert!(!desktop_entry_is_on(&entry(
            "X-GNOME-Autostart-enabled=false\n"
        )));
        assert!(!desktop_entry_is_on(&entry(
            "X-GNOME-Autostart-enabled=FALSE\n"
        )));
        // Only the `[Desktop Entry]` group counts.
        assert!(desktop_entry_is_on(&entry(
            "[Desktop Action x]\nHidden=true\n"
        )));
    }

    /// The entry on disk, in a scratch home: written with the quoted path, read back as on, off
    /// once the desktop hides it, and on again when the switch is turned on. It runs as a child of
    /// itself because `HOME` cannot be set for a test thread.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_entry_that_is_written_is_read_back_as_the_desktop_would() {
        const ME: &str =
            "system::autostart::tests::the_entry_that_is_written_is_read_back_as_the_desktop_would";
        let image = "/opt/My Apps/takeshot.AppImage";
        let Some(home) = std::env::var_os("TAKESHOT_TEST_HOME") else {
            let home =
                std::env::temp_dir().join(format!("takeshot-autostart-{}", std::process::id()));
            std::fs::create_dir_all(&home).unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", ME, "--test-threads=1"])
                .env("TAKESHOT_TEST_HOME", &home)
                .env("HOME", &home)
                .env("APPIMAGE", image)
                .status()
                .unwrap();
            std::fs::remove_dir_all(&home).unwrap();
            assert!(status.success(), "the child run failed");
            return;
        };
        let file = std::path::Path::new(&home).join(".config/autostart/Takeshot.desktop");
        assert!(!autostart_enabled(), "nothing is written yet");
        set_autostart(true).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        let exec = text.lines().find_map(|l| l.strip_prefix("Exec=")).unwrap();
        assert_eq!(exec_arguments(exec), [image, "--minimized"], "{exec}");
        assert!(autostart_enabled());
        std::fs::write(&file, format!("{text}\nHidden=true\n")).unwrap();
        assert!(!autostart_enabled(), "the desktop turned the entry off");
        set_autostart(true).unwrap();
        assert!(
            autostart_enabled(),
            "turning the switch on writes a fresh entry"
        );
        set_autostart(false).unwrap();
        assert!(!file.exists() && !autostart_enabled());
    }

    #[test]
    fn autostart_starts_minimized_as_takeshot() {
        let a = autostart().expect("autostart entry");
        assert_eq!(a.get_app_name(), "Takeshot");
        assert_eq!(a.get_args(), ["--minimized".to_string()]);
    }

    /// The whole Run entry: the current user only (no admin), and a quoted path, since an install
    /// folder such as "C:\\Program Files" has spaces.
    #[cfg(windows)]
    #[test]
    fn autostart_is_the_current_users_quoted_run_entry() {
        let exe = std::env::current_exe().unwrap();
        let want = AutoLaunch::new(
            "Takeshot",
            &format!("\"{}\"", exe.display()),
            auto_launch::WindowsEnableMode::CurrentUser,
            &["--minimized"],
        );
        assert_eq!(autostart().unwrap(), want);
    }
}
