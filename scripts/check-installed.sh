#!/usr/bin/env bash
# Installs the package and opens it. Run it only inside a disposable container or VM.
set -euo pipefail

if [[ ! -e /.dockerenv && ! -e /run/.containerenv ]]; then
  echo "Refusing to run outside a container: this script installs packages." >&2
  exit 1
fi

pacman -U --noconfirm "$@"
pacman -S --noconfirm --needed xorg-server-xvfb xorg-xauth ttf-dejavu

for forbidden in Hyprland waybar; do
  if command -v "$forbidden" >/dev/null; then
    echo "$forbidden must not be installed for this check." >&2
    exit 1
  fi
done

home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT

output="$(
  env -i PATH="$PATH" HOME="$home" XDG_CONFIG_HOME="$home/config" GDK_BACKEND=x11 \
    GSETTINGS_BACKEND=memory GTK_A11Y=none DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent \
    ARCH_SETTING_SMOKE_TEST=1 \
    timeout 30 xvfb-run --auto-servernum -- /usr/bin/arch-setting 2>&1
)"
echo "$output"

if grep -E 'CRITICAL|WARNING|ERROR' <<<"$output"; then
  echo "The installed application reported errors." >&2
  exit 1
fi
