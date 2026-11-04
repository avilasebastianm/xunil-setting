# 5. Scope of the first version

Status: proposed, pending the maintainer's confirmation

## Decision

- **Compositor:** Hyprland only, behind a common interface that a fake compositor also implements in
  tests.
- **System panels:** network, Bluetooth, sound, date and time, region, about, and power.
- **Hyprland panels:** display, background, mouse, keyboard, and Waybar as a component.

`search`, `online-accounts` and `wellbeing` stay out because they are tied to GNOME services.

## Consequences

Power, mouse and keyboard move forward from roadmap phase 5, to phases 2 and 3. Several compositor
panels exercise the common interface before a second compositor exists.
