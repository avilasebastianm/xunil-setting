#!/usr/bin/env bash
# Runs the interface tests on a display server without a screen.
set -euo pipefail

export XUNIL_SETTING_HEADLESS=1
exec xvfb-run --auto-servernum -- cargo test -p xunil-setting --test smoke -- --ignored "$@"
