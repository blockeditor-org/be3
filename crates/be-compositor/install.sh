#!/bin/sh
# Installs be-compositor and its wayland-sessions entry under a prefix, so that
# a display manager (GDM, SDDM) offers it as a session on its login screen. The
# entry runs the installed binary by its absolute path, in session mode.
#
#   ./scripts/buck run //crates/be-compositor:install [-- <prefix>]
#
# The prefix is /usr/local unless given. GDM and SDDM 0.20 or later read
# <prefix>/share/wayland-sessions for /usr/local; an older SDDM reads only
# /usr/share/wayland-sessions, so give it /usr.
set -eu
binary="$1"
entry="$2"
prefix="${3:-/usr/local}"
bin="$prefix/bin/be-compositor"
sessions="$prefix/share/wayland-sessions"
staged="$(mktemp)"
trap 'rm -f "$staged"' EXIT
sed "s|^Exec=be-compositor |Exec=$bin |" "$entry" > "$staged"
sudo=""
if [ "$(id -u)" -ne 0 ]; then
    sudo="sudo"
fi
$sudo install -D -m 755 "$binary" "$bin"
$sudo install -D -m 644 "$staged" "$sessions/be-compositor.desktop"
echo "Installed $bin and $sessions/be-compositor.desktop."
echo 'Log out and pick BE Compositor from the session menu on the login screen.'
