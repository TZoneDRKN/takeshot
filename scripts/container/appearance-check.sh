#!/usr/bin/env bash
# Runs INSIDE a container with the .deb installed (scripts/linux-smoke.sh starts it):
#
#   appearance-check.sh x11|wayland
#
# The "System" mode follows the desktop's light or dark setting on Linux, which winit does not report
# (None on X11, an echo of the app's own setting on Wayland): the app reads the settings portal. A
# stand-in portal (fake-portal.py) answers on the session bus and each case renders the window with
# `--mode system` and asks whether the picture is light or dark:
#   no portal at all  -> dark (the fallback)
#   portal says light -> light         portal says dark -> dark
#   the older Read    -> light         (a portal too old for ReadOne)
#   ReadOne says dark, the signal says light a second later -> light (only the signal can say it)
set -eu
mode=${1:?x11 or wayland}
export HOME=/root
mkdir -p "$HOME/.config/takeshot"
# Pin everything but the mode, so the picture differs only by what the desktop said.
echo '{}' > "$HOME/.config/takeshot/settings.json"
export XDG_RUNTIME_DIR=/tmp/xdg
mkdir -p -m 700 "$XDG_RUNTIME_DIR"

case $mode in
  x11)
    Xvfb :99 -ac -screen 0 1280x800x24 > /tmp/xvfb.log 2>&1 &
    for _ in $(seq 50); do [ -S /tmp/.X11-unix/X99 ] && break; sleep 0.2; done
    export DISPLAY=:99
    ;;
  wayland)
    WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 sway -c /dev/null > /tmp/sway.log 2>&1 &
    for _ in $(seq 50); do [ -S "$XDG_RUNTIME_DIR/wayland-1" ] && break; sleep 0.2; done
    export WAYLAND_DISPLAY=wayland-1
    ;;
  *) echo "unknown mode $mode"; exit 2 ;;
esac

fail() { echo "FAIL: $1"; cat /tmp/takeshot.log 2> /dev/null || true; exit 1; }

# Prints "light" or "dark" for a picture, by the mean brightness of its pixels.
brightness() {
  python3 - "$1" << 'PY'
import sys
from PIL import Image, ImageStat
im = Image.open(sys.argv[1]).convert("L")
print("light" if ImageStat.Stat(im).mean[0] > 128 else "dark")
PY
}

# render <name> <portal args...>: the window with --mode system beside a stand-in portal (none when
# no args are given); prints what the picture is.
render() {
  name=$1; shift
  portal=
  if [ $# -gt 0 ]; then
    python3 /t/fake-portal.py "$@" > /tmp/portal.log 2>&1 &
    portal=$!
    sleep 1
  fi
  rm -f "/tmp/$name.png"
  takeshot --theme takeui --mode system --lang en --page estilo --screenshot "/tmp/$name.png" > /tmp/takeshot.log 2>&1 || fail "$name: takeshot exited with $?"
  [ -z "$portal" ] || { kill "$portal" 2> /dev/null || true; wait "$portal" 2> /dev/null || true; }
  [ -s "/tmp/$name.png" ] || fail "$name: no picture"
  brightness "/tmp/$name.png"
}

expect() { # expect <name> <light|dark> <portal args...>
  name=$1; want=$2; shift 2
  got=$(render "$name" "$@")
  [ "$got" = "$want" ] || { cp "/tmp/$name.png" "/pk/smoke/appearance-$mode-$name.png" 2> /dev/null || true; fail "$name: the window is $got, wanted $want"; }
  cp "/tmp/$name.png" "/pk/smoke/appearance-$mode-$name.png" 2> /dev/null || true
  echo "ok: $name -> $got"
}

expect no-portal dark
expect portal-light light 2
expect portal-dark dark 1
expect portal-read-only light 2 --legacy
expect signal-only light 1 --then 2 1.2
echo "ALL OK ($mode)"
