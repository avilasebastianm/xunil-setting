#!/usr/bin/env bash
# Runs the interface tests on a display server without a screen.
set -euo pipefail

export ARCH_SETTING_HEADLESS=1
exec xvfb-run --auto-servernum -- cargo test -p arch-setting --test smoke -- --ignored "$@"
