#!/usr/bin/env bash
# Runs INSIDE a container with the .deb installed (scripts/linux-smoke.sh starts it):
#
#   desktop-check.sh
#
# The launcher entry the deb installs (packaging/linux/takeshot.desktop, filled in by cargo-packager):
# it validates, and each of its right-click actions does what its name says when a launcher runs the
# Exec line it carries, on X11 (Xvfb with openbox): Capture area puts the capture window on screen,
# Capture full screen saves a picture, Quit closes a running resident. The main window's class is
# the StartupWMClass the entry names.
set -eu
export HOME=/root
mkdir -p "$HOME/.config/takeshot" /tmp/caps
echo '{"save_dir":"/tmp/caps"}' > "$HOME/.config/takeshot/settings.json"
export XDG_RUNTIME_DIR=/tmp/xdg
mkdir -p -m 700 "$XDG_RUNTIME_DIR"
entry=/usr/share/applications/takeshot.desktop

fail() {
  echo "FAIL: $1"
  echo "---- entry"; cat "$entry" 2> /dev/null || true
  echo "---- takeshot.log"; cat /tmp/takeshot.log 2> /dev/null || true
  exit 1
}

desktop-file-validate "$entry" || fail "desktop-file-validate"
echo "ok: the entry validates"
if grep -q '{{' "$entry"; then fail "a template placeholder is left in the entry"; fi
grep -qx 'Actions=capture-area;capture-full;quit;' "$entry" || fail "the entry does not list the three actions"
grep -qx 'Exec=takeshot' "$entry" || fail "the entry does not run takeshot"
echo "ok: the entry is filled in and lists its actions"

# The command line of an action, the way a launcher reads it from the entry.
exec_of() { sed -n "/^\[Desktop Action $1\]/,/^\[/s/^Exec=//p" "$entry"; }

Xvfb :99 -ac -screen 0 1280x800x24 > /tmp/xvfb.log 2>&1 &
for _ in $(seq 50); do [ -S /tmp/.X11-unix/X99 ] && break; sleep 0.2; done
export DISPLAY=:99
openbox > /tmp/openbox.log 2>&1 &
sleep 1
windows() { wmctrl -l | grep -c . || true; }
alive() { pgrep -x takeshot > /dev/null; }

# Capture area: the capture window comes up and nothing else (no main window).
sh -c "$(exec_of capture-area)" > /tmp/takeshot.log 2>&1 &
for _ in $(seq 50); do [ "$(windows)" -ge 1 ] && break; sleep 0.2; done
[ "$(windows)" -ge 1 ] || fail "Capture area opened no window"
pkill -x takeshot || true
for _ in $(seq 25); do alive || break; sleep 0.2; done
echo "ok: Capture area puts the capture window on screen"

# Capture full screen: the screen is taken at once and saved to the folder in the settings.
rm -f /tmp/caps/*.png
sh -c "$(exec_of capture-full)" > /tmp/takeshot.log 2>&1 &
for _ in $(seq 50); do ls /tmp/caps/*.png > /dev/null 2>&1 && break; sleep 0.2; done
ls /tmp/caps/*.png > /dev/null 2>&1 || fail "Capture full screen saved no picture"
pkill -x takeshot || true
for _ in $(seq 25); do alive || break; sleep 0.2; done
echo "ok: Capture full screen saves a picture"

# The resident's window carries the class the entry names, and Quit closes the resident.
takeshot > /tmp/takeshot.log 2>&1 &
for _ in $(seq 50); do [ "$(windows)" -ge 1 ] && break; sleep 0.2; done
[ "$(windows)" -ge 1 ] || fail "the main window did not come up"
class=$(wmctrl -lx | awk '{print $3}' | head -1)
case "$class" in
  *takeshot*) ;;
  *) fail "the main window's class is \"$class\", not the entry's StartupWMClass" ;;
esac
want=$(sed -n 's/^StartupWMClass=//p' "$entry")
[ "$want" = takeshot ] || fail "StartupWMClass is $want"
echo "ok: the main window's class ($class) matches StartupWMClass=$want"
timeout 40 sh -c "$(exec_of quit)" > /tmp/quit.log 2>&1 && code=0 || code=$?
[ "$code" = 0 ] || { cat /tmp/quit.log; fail "the Quit action exited with $code"; }
for _ in $(seq 50); do alive || break; sleep 0.2; done
if alive; then fail "the Quit action left the resident running"; fi
echo "ok: the Quit action closes the resident"
echo "ALL OK (desktop entry)"
