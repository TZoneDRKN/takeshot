# Testing on Linux

Two scripts check the Linux packages in Docker, from Windows, macOS or Linux:

```sh
scripts/linux-packages.sh   # builds the .deb, .AppImage and .tar.gz in the CI image (scripts/ci-linux)
scripts/linux-smoke.sh      # opens them in clean Ubuntu 24.04 containers and saves what they drew
```

`linux-packages.sh` puts the packages in `target/packages-linux` and prints the shared libraries the binary needs (`dpkg-shlibdeps`), which is where the `.deb`'s `depends` in `Cargo.toml` comes from. `linux-smoke.sh` then:

- installs the `.deb` with `apt` (which pulls its dependencies) and runs `takeshot --screenshot` on **X11** (Xvfb) and on **Wayland** (sway with no screen, `WLR_BACKENDS=headless`);
- runs the AppImage on X11 in a container with no GTK installed, so it must carry its own libraries;
- checks the resident's window (`scripts/container/resident-check.sh`, `scripts/linux-smoke.sh resident` for just this part): first `takeshot --version` must print `takeshot x.y.z`, exit 0, write no log and leave nothing running; then, started in the tray, there is no window, and three times a second launch shows the window and the X (with "Minimize to tray" chosen) closes it, the resident outliving each one. On **Wayland** (sway, whose bar hosts the tray icons) it does that and closes the window with `swaymsg kill`; it then takes a capture on the sample screen and checks that the "Capture saved" notice reaches mako, the notification daemon, and opens no window. On **X11** (Xvfb with openbox) there is no tray host: the hidden start gives up after 30 s and shows the window, the X (`wmctrl -c`) only minimizes it, and a second launch brings it back. Both end with `takeshot --quit`: it must answer 0 and the resident must be gone (on Wayland the resident has no window at that point, on X11 its window is minimized), and a second `--quit` with none running answers 0 as well.
- checks the launcher entry (`scripts/container/desktop-check.sh`, `scripts/linux-smoke.sh desktop` for just this part): the installed `takeshot.desktop` passes `desktop-file-validate`, lists its three actions, and on **X11** (Xvfb with openbox) each action's `Exec` line, run the way a launcher runs it, does what its name says: Capture area puts the capture window on screen, Capture full screen saves a picture, Quit closes a running resident; the main window's class matches the entry's `StartupWMClass`.
- checks that the "System" mode follows the desktop (`scripts/container/appearance-check.sh`, `scripts/linux-smoke.sh appearance` for just this part), on **X11** and on **Wayland** (sway): a stand-in portal (`scripts/container/fake-portal.py`) answers `org.freedesktop.appearance color-scheme` on the session bus and each case renders the window with `--mode system`: no portal gives dark, a portal that says light gives light, one that says dark gives dark, one that has no preference (0, what GNOME's default style reports) gives light, one too old for `ReadOne` (only `Read`) is understood, a portal that answers dark but then emits `SettingChanged` with light turns the window light (only the signal can say it), and signals for another key or namespace change nothing. On X11 one more case keeps the window open while the portal changes from light to dark and reads the screen: the window follows with no input at all, which `--screenshot` (it repaints every frame) cannot show.

The pictures land in `target/packages-linux/smoke`. Look at each one: the window drawn whole, in the theme asked for.

## What the containers cannot check

These need a desktop session and go through by hand before a release (a VM or a Linux machine):

1. The tray icon and its menu (an AppIndicator panel: GNOME with the AppIndicator extension, KDE, XFCE with the StatusNotifier plugin). Settings > General > Window and tray has no "Clicking the tray icon" row here, since a click on the icon never reaches the app (the Windows and macOS builds show it).
2. `PrtSc` and `Shift+PrtSc` on X11; on Wayland, a desktop shortcut running `takeshot --capture`, and the capture through the desktop portal.
3. Settings > General > "Start with the system": log out and in; the entry is `~/.config/autostart/Takeshot.desktop`. With the app installed under a folder with a space (an AppImage in `~/My Apps`), its `Exec` line holds the path in double quotes and the app does start. Turning the entry off in GNOME Tweaks (Startup Applications, `X-GNOME-Autostart-enabled=false`) or with `Hidden=true` makes the switch read off the next time the page opens, and turning it on writes a fresh entry. `cargo test` checks the quoting against a reader written from the Desktop Entry specification and the reading of the file in a scratch home (`the_entry_that_is_written_is_read_back_as_the_desktop_would`, which only runs on Linux); GLib read every quoted form of the test paths as the program plus `--minimized`, except a path with a `%`, which GLib cannot launch either way.
4. Quit from the tray (or Settings > General when there is no tray) closes the editors and pinned pictures too.
5. The launcher's right-click menu (GNOME's dash, KDE's task manager and application menu) shows Capture area, Capture full screen and Quit, in the language of the desktop, and each one works; windows of the running app group under the launcher's icon.
6. Settings > General > Appearance set to the system mode follows the desktop: switch GNOME's or KDE's light and dark style with the app open and the window follows within a moment, on X11 and on Wayland.
7. On Wayland (KDE, GNOME): "Minimize to tray" leaves no entry in the taskbar, the tray's "Open" (and a second launch) brings the window back to the workspace you are on, and Copy or Save in a capture shows a notification in the desktop's corner while the screen goes back to normal at once.

## Last run

2026-11-08, v0.1.0, Docker Desktop on Windows 10, `ubuntu:24.04`:

| Package | Session | Result |
|---|---|---|
| `.deb` (apt pulled GTK, Mesa and the rest from its `depends`) | X11, Xvfb 1280 x 800 | Home and the editor (Coral UI) drawn whole |
| `.deb` | Wayland, sway headless (pixman) | Home drawn whole; Mesa falls back to software rendering, with EGL warnings |
| `.AppImage` | X11, Xvfb, no GTK installed (only the AppImage excludelist's libraries) | Style drawn whole: GTK and its libraries come from the AppImage |

The AppImage, like every AppImage, expects the libraries of AppImageCommunity's excludelist (glibc, GL, X11, fontconfig, freetype, harfbuzz, fribidi and a few more) from the system: any desktop has them, a bare container does not, so the script installs exactly those.

### The resident's window

2026-11-08, the branch that fixed [takeflow-oficial/takeshot#4](https://github.com/takeflow-oficial/takeshot/issues/4) and [#5](https://github.com/takeflow-oficial/takeshot/issues/5), the same containers (`scripts/linux-smoke.sh resident`):

| Session | Result |
|---|---|
| X11, Xvfb with openbox, no tray host | The hidden start showed the window after 30 s; the X only minimized it; a second launch brought it back |
| Wayland, sway with swaybar as the tray host, mako | The hidden start had no window. Three times: a second launch showed the window, the X destroyed it, the resident stayed. The saved notice reached mako and opened no window |

By hand on Kubuntu (Plasma 6.6.6, Wayland), with the `.deb` of that branch: minimizing to the tray leaves nothing in the taskbar, the window comes back from the tray and from a second launch, and Copy and Save show a desktop notification.

### The launcher entry

2026-11-14, the branch that added the launcher's right-click actions, the same containers (`scripts/linux-smoke.sh desktop`):

| Session | Result |
|---|---|
| X11, Xvfb with openbox, the `.deb` installed | The entry passed `desktop-file-validate`. Capture area put the capture window on screen, Capture full screen saved a picture, Quit closed the resident, and the main window's class was `takeshot`, the entry's `StartupWMClass`. The AppImage carries the same entry |
