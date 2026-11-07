# 9. Talk to system services through gio

Status: accepted (2026-11-07)

## Context

ADR 0008 makes system services the only way to change privileged settings, so the core needs a D-Bus
client. The options were gio, which the GTK bindings already bring, and `zbus`, a D-Bus
implementation in pure Rust.

A call that needs permission waits while polkit shows its dialog, which can take minutes. The window
must keep responding meanwhile.

## Decision

`xunil-setting-core` uses gio for D-Bus. gio is not GTK, so the core still builds and runs its tests
without a window.

- Calls are asynchronous and run on the GLib main loop, the same one the interface uses.
- Clients take a `DBusConnection`, so tests connect them straight to a fake service through a
  private socket, with no bus and no real service involved.

## Consequences

- No new dependencies: gio and GLib are already part of the package.
- The core is tied to the GLib main loop. A helper process without GLib would need another client.
- Fake services are written with the same gio API that the real clients use.
