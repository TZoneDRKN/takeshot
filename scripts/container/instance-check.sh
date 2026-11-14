#!/usr/bin/env bash
# Runs INSIDE a container with the .deb installed (scripts/linux-smoke.sh starts it):
#
#   instance-check.sh
#
# The resident's socket must belong to its user. Two users share this host: alice runs the resident,
# and bob, who knows her name, asks it to quit (`USER=alice takeshot --quit`, the name the socket
# used to take from USER). It has to find nothing to quit, and alice's resident must keep running.
# Then the folder the socket lives in: `$XDG_RUNTIME_DIR/takeshot`, mode 0700 and hers, or
# `/tmp/takeshot-<uid>` with no XDG_RUNTIME_DIR; and a folder bob made there first, open to
# everyone, is not used (the resident runs with no single instance and says so on standard error).
set -eu

fail() {
  echo "FAIL: $1"
  for f in /tmp/alice.log /home/alice/.local/state/takeshot/logs/takeshot-resident.log; do
    echo "---- $f"; cat "$f" 2> /dev/null || true
  done
  exit 1
}

Xvfb :99 -ac -screen 0 1280x800x24 > /tmp/xvfb.log 2>&1 &
for _ in $(seq 50); do [ -S /tmp/.X11-unix/X99 ] && break; sleep 0.2; done

useradd -m alice
useradd -m bob
alice=$(id -u alice)
bob=$(id -u bob)
for u in alice bob; do
  mkdir -p -m 700 "/run/user/$(id -u $u)"
  chown "$u" "/run/user/$(id -u $u)"
done

as() { # as <user> <extra env...> -- <command...>
  local user=$1
  shift
  runuser -u "$user" -- env HOME="/home/$user" XDG_RUNTIME_DIR="/run/user/$(id -u "$user")" "$@"
}
alive() { pgrep -u alice -x takeshot > /dev/null; }

as alice DISPLAY=:99 takeshot --minimized --lang en > /tmp/alice.log 2>&1 &
for _ in $(seq 50); do alive && break; sleep 0.2; done
alive || fail "alice's resident did not start"
sleep 2

# bob knows alice's name: the socket used to take it from USER.
as bob USER=alice timeout 40 takeshot --quit > /tmp/bob-quit.log 2>&1 && code=0 || code=$?
sleep 2
alive || fail "bob closed alice's resident (his --quit exited $code)"
[ "$code" = 0 ] || { cat /tmp/bob-quit.log; fail "bob's --quit exited with $code: it should find nothing to quit"; }
echo "ok: bob's --quit with alice's name did not reach her resident"

dir=/run/user/$alice/takeshot
[ -d "$dir" ] || fail "no socket folder at $dir"
[ "$(stat -c '%a %U' "$dir")" = "700 alice" ] || fail "$dir is '$(stat -c '%a %U' "$dir")', not '700 alice'"
ls "$dir" | grep -q '\.sock$' || fail "no socket in $dir"
# The folder refuses bob himself, not only his own environment: he cannot enter it.
! as bob test -x "$dir" || fail "bob can enter $dir"
echo "ok: the socket is in $dir, 0700 and alice's; bob cannot enter it"

# alice's own --quit still works. A process that exits does not unlink its socket file, so the next
# resident finds a dead one in the folder and takes its place.
as alice timeout 40 takeshot --quit > /tmp/alice-quit.log 2>&1 && code=0 || code=$?
[ "$code" = 0 ] || { cat /tmp/alice-quit.log; fail "alice's --quit exited with $code"; }
for _ in $(seq 50); do alive || break; sleep 0.2; done
! alive || fail "alice's --quit answered but her resident still runs"
echo "ok: alice's own --quit closes her resident"
as alice DISPLAY=:99 takeshot --minimized --lang en > /tmp/alice.log 2>&1 &
for _ in $(seq 50); do alive && break; sleep 0.2; done
alive || fail "the next resident did not start over the file the last one left"
sleep 2
as alice timeout 40 takeshot --quit > /dev/null 2>&1 || true
for _ in $(seq 50); do alive || break; sleep 0.2; done
! alive || fail "the next resident did not answer its --quit: it was not the one on the socket"
echo "ok: the next resident takes over a socket file left behind, and answers"

# No XDG_RUNTIME_DIR: the folder is /tmp/takeshot-<uid>, again 0700 and hers.
runuser -u alice -- env -u XDG_RUNTIME_DIR HOME=/home/alice DISPLAY=:99 takeshot --minimized --lang en > /tmp/alice.log 2>&1 &
for _ in $(seq 50); do alive && break; sleep 0.2; done
alive || fail "alice's resident did not start with no XDG_RUNTIME_DIR"
sleep 2
dir=/tmp/takeshot-$alice
[ "$(stat -c '%a %U' "$dir")" = "700 alice" ] || fail "$dir is '$(stat -c '%a %U' "$dir" 2>&1)', not '700 alice'"
echo "ok: with no XDG_RUNTIME_DIR the folder is $dir, 0700 and alice's"
runuser -u alice -- env -u XDG_RUNTIME_DIR HOME=/home/alice timeout 40 takeshot --quit > /dev/null 2>&1 || true
for _ in $(seq 50); do alive || break; sleep 0.2; done
! alive || fail "alice's resident did not quit"

# A folder bob made first, open to everyone, is not used.
rm -rf "/tmp/takeshot-$alice"
mkdir -m 755 "/tmp/takeshot-$alice"
chown bob "/tmp/takeshot-$alice"
runuser -u alice -- env -u XDG_RUNTIME_DIR HOME=/home/alice DISPLAY=:99 takeshot --minimized --lang en > /tmp/alice.log 2>&1 &
for _ in $(seq 50); do alive && break; sleep 0.2; done
alive || fail "alice's resident did not start beside bob's folder"
sleep 2
[ -z "$(ls -A "/tmp/takeshot-$alice")" ] || fail "alice put something in bob's folder"
# The claim comes before the log file is opened (a second launch never touches the resident's file),
# so the line is on standard error.
grep -q 'running without single instance: the socket folder is not private' /tmp/alice.log ||
  fail "the resident does not say it runs without single instance"
echo "ok: a folder alice does not own is left alone, and the resident says so"
pkill -u alice -x takeshot || true
echo "ALL OK"
