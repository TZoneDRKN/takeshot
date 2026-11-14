#!/usr/bin/env bash
# Opens the Linux packages from target/packages-linux (scripts/linux-packages.sh) in clean Ubuntu
# 24.04 containers and saves what each window drew into target/packages-linux/smoke:
#   - the .deb installed with apt (its dependencies pulled in), on X11 (Xvfb) and on Wayland (sway
#     with no screen);
#   - the AppImage on X11, in a container with no GTK installed (it must carry its own libraries).
# Each run is `takeshot --screenshot`: the window draws ~120 frames and saves its picture.
# Then the resident's window is checked (scripts/container/resident-check.sh): it is made when
# wanted, destroyed to hide it to the tray, and made again, on X11 and on Wayland.
# Last, the launcher entry the deb installs is validated and its right-click actions run
# (scripts/container/desktop-check.sh), and the "System" mode follows the setting a stand-in
# desktop portal reports, on X11 and on Wayland (scripts/container/appearance-check.sh).
# And `floor`: the .deb installed on the oldest systems it claims (Ubuntu 22.04 and Debian 12, whose
# glibc is the floor of the files), on X11, and the AppImage on Debian 12 (on Ubuntu 22.04 it does not
# start: the pango it carries needs a HarfBuzz newer than that system's 2.7, which an AppImage never
# bundles, so the README sends that system to the .deb), and the tar.gz and the AppImage on Fedora 36
# (glibc 2.35, the oldest the README promises on the rpm side).
#
# And `tarball`: the commands of the tar.gz's INSTALL.txt run as they are, then the launcher entry
# validates and the program starts from the PATH.
#
#   scripts/linux-smoke.sh [screens|resident|desktop|appearance|floor|tarball]   (default: all)
set -eu
only=${1:-all}
want() { [ "$only" = all ] || [ "$only" = "$1" ]; }
cd "$(git rev-parse --show-toplevel)"
dir=target/packages-linux
deb=$(cd "$dir" && ls takeshot-*-linux-x86_64.deb)
appimage=$(cd "$dir" && ls takeshot-*-linux-x86_64.AppImage)
tgz=$(cd "$dir" && ls takeshot-*-linux-x86_64.tar.gz)
mkdir -p "$dir/smoke"
mount="$(pwd -W 2> /dev/null || pwd)/$dir:/pk"
checks="$(pwd -W 2> /dev/null || pwd)/scripts/container:/t:ro"
# apt in a container now and then fails on the network: once more, then its log.
prelude='
  set -e
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq > /dev/null
  get() {
    apt-get install -y -qq --no-install-recommends "$@" > /tmp/apt.log 2>&1 ||
      apt-get install -y -qq --no-install-recommends "$@" > /tmp/apt.log 2>&1 ||
      { tail -20 /tmp/apt.log; exit 1; }
  }
'
# --init: as PID 1, xvfb-run never hears that Xvfb is ready.
run_on() { MSYS_NO_PATHCONV=1 docker run --rm --init -v "$mount" -v "$checks" "$1" bash -c "$prelude $2"; }
run() { run_on ubuntu:24.04 "$1"; }

if want screens; then
echo "== deb on X11"
run "
  get /pk/$deb xvfb xauth libgl1-mesa-dri
  command -v takeshot
  ls /usr/share/applications | grep -i takeshot
  xvfb-run -a -s '-screen 0 1280x800x24' takeshot --page inicio --lang en --screenshot /pk/smoke/deb-x11-home.png
  xvfb-run -a -s '-screen 0 1280x800x24' takeshot --page editor --lang en --theme coral --screenshot /pk/smoke/deb-x11-editor.png
"

echo "== deb on Wayland (sway)"
run "
  get /pk/$deb sway libgl1-mesa-dri libegl1
  export XDG_RUNTIME_DIR=/tmp/xdg && mkdir -p -m 700 \$XDG_RUNTIME_DIR
  WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 sway -c /dev/null > /tmp/sway.log 2>&1 &
  for i in \$(seq 50); do [ -S \$XDG_RUNTIME_DIR/wayland-1 ] && break; sleep 0.2; done
  ls \$XDG_RUNTIME_DIR
  WAYLAND_DISPLAY=wayland-1 takeshot --page inicio --lang en --mode dark --screenshot /pk/smoke/deb-wayland-home.png
"

echo "== AppImage on X11, no GTK installed"
# Only what every desktop has: the libraries AppImages leave out by design (AppImageCommunity's
# excludelist: glibc, GL, X11, fonts and a few more), plus xkbcommon. GTK and the rest must come
# from the AppImage.
run "
  get xvfb xauth libgl1-mesa-dri libgl1 libegl1 libglx0 libopengl0 libdrm2 libgbm1 libxcb1 libx11-6 \
    libx11-xcb1 libxcb-dri2-0 libxcb-dri3-0 libwayland-client0 libasound2t64 libfontconfig1 libfreetype6 \
    libharfbuzz0b libfribidi0 libcom-err2 libexpat1 libgpg-error0 libice6 libsm6 libusb-1.0-0 libuuid1 \
    zlib1g libgmp10 libpipewire-0.3-0t64 libxkbcommon0 libxkbcommon-x11-0
  if dpkg -s libgtk-3-0t64 > /dev/null 2>&1; then echo 'GTK is installed: the check proves nothing'; exit 1; fi
  cp /pk/$appimage /tmp/takeshot.AppImage && chmod +x /tmp/takeshot.AppImage
  xvfb-run -a -s '-screen 0 1280x800x24' /tmp/takeshot.AppImage --appimage-extract-and-run --page estilo --lang en --screenshot /pk/smoke/appimage-x11-style.png
"
ls -la "$dir/smoke"
fi

if want resident; then
  echo "== deb on X11: the resident's window comes and goes"
  run "
    get /pk/$deb xvfb xauth libgl1-mesa-dri openbox wmctrl x11-utils procps dbus dbus-bin
    dbus-run-session -- bash /t/resident-check.sh x11
  "

  echo "== deb on Wayland (sway): the window comes and goes, the notice is a notification (mako)"
  run "
    get /pk/$deb sway libgl1-mesa-dri libegl1 jq procps mako-notifier dbus dbus-bin
    dbus-run-session -- bash /t/resident-check.sh wayland
  "
fi

if want desktop; then
  echo "== deb on X11: the launcher entry and its actions"
  run "
    get /pk/$deb xvfb xauth libgl1-mesa-dri openbox wmctrl x11-utils procps dbus dbus-bin desktop-file-utils
    dbus-run-session -- bash /t/desktop-check.sh
  "
fi

if want appearance; then
  echo "== deb on X11: the System mode follows the desktop's light or dark setting (stand-in portal)"
  run "
    get /pk/$deb xvfb xauth libgl1-mesa-dri python3-gi python3-pil dbus dbus-bin
    dbus-run-session -- bash /t/appearance-check.sh x11
  "

  echo "== deb on Wayland (sway): the same, where winit only echoes the app's own setting"
  run "
    get /pk/$deb sway libgl1-mesa-dri libegl1 python3-gi python3-pil dbus dbus-bin
    dbus-run-session -- bash /t/appearance-check.sh wayland
  "
fi

if want floor; then
  for image in ubuntu:22.04 debian:12; do
    echo "== deb on X11 in $image: the oldest systems the files claim"
    run_on "$image" "
      get /pk/$deb xvfb xauth libgl1-mesa-dri
      takeshot --version
      xvfb-run -a -s '-screen 0 1280x800x24' takeshot --page inicio --lang en --screenshot /pk/smoke/floor-${image/:/-}-home.png
      xvfb-run -a -s '-screen 0 1280x800x24' takeshot --page editor --lang en --theme coral --screenshot /pk/smoke/floor-${image/:/-}-editor.png
    "
  done

  echo "== AppImage on X11 in debian:12, no GTK installed"
  run_on debian:12 "
    get xvfb xauth libgl1-mesa-dri libgl1 libegl1 libglx0 libopengl0 libdrm2 libgbm1 libxcb1 libx11-6 \
      libx11-xcb1 libxcb-dri2-0 libxcb-dri3-0 libwayland-client0 libasound2 libfontconfig1 libfreetype6 \
      libharfbuzz0b libfribidi0 libcom-err2 libexpat1 libgpg-error0 libice6 libsm6 libusb-1.0-0 libuuid1 \
      zlib1g libgmp10 libpipewire-0.3-0 libxkbcommon0 libxkbcommon-x11-0
    if dpkg -s libgtk-3-0 > /dev/null 2>&1; then echo 'GTK is installed: the check proves nothing'; exit 1; fi
    cp /pk/$appimage /tmp/takeshot.AppImage && chmod +x /tmp/takeshot.AppImage
    xvfb-run -a -s '-screen 0 1280x800x24' /tmp/takeshot.AppImage --appimage-extract-and-run --page estilo --lang en --screenshot /pk/smoke/floor-appimage-debian-12-style.png
  "

  echo "== tar.gz and AppImage on X11 in fedora:36"
  MSYS_NO_PATHCONV=1 docker run --rm --init -v "$mount" fedora:36 bash -c "
    set -e
    dnf install -y -q gtk3 xorg-x11-server-Xvfb xauth mesa-dri-drivers mesa-libEGL mesa-libgbm mesa-libGL \
      libglvnd-opengl libxkbcommon-x11 libXcursor libXi libXrandr libwayland-client libwayland-cursor \
      libwayland-egl libappindicator-gtk3 fuse-libs > /tmp/dnf.log 2>&1 || { tail -20 /tmp/dnf.log; exit 1; }
    mkdir -p /tmp/t && tar -xzf /pk/$tgz -C /tmp/t
    /tmp/t/takeshot --version
    xvfb-run -a -s '-screen 0 1280x800x24' /tmp/t/takeshot --page inicio --lang en --screenshot /pk/smoke/floor-fedora-36-tgz-home.png
    cp /pk/$appimage /tmp/t.AppImage && chmod +x /tmp/t.AppImage
    xvfb-run -a -s '-screen 0 1280x800x24' /tmp/t.AppImage --appimage-extract-and-run --page estilo --lang en --screenshot /pk/smoke/floor-fedora-36-appimage-style.png
  "
fi

if want tarball; then
  echo "== tar.gz: INSTALL.txt as it is, the entry validates, the program starts from the PATH"
  run_on debian:12 "
    get libgtk-3-0 libgbm1 desktop-file-utils
    mkdir -p /tmp/t && tar -xzf /pk/$tgz -C /tmp/t && cd /tmp/t
    grep '^  install' INSTALL.txt | sh -e
    export PATH=\$HOME/.local/bin:\$PATH
    takeshot --version
    desktop-file-validate ~/.local/share/applications/takeshot.desktop
    grep -q '^Exec=takeshot' ~/.local/share/applications/takeshot.desktop
    test -s ~/.local/share/icons/hicolor/256x256/apps/takeshot.png
    ls /tmp/t
  "
fi
