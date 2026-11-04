# 7. Compile modules into the application

Status: accepted (2026-11-04)

## Context

The application needs modules for desktops and components that only appear when they apply. Rust has
no stable ABI, so the options were modules compiled into the application, helper processes speaking
D-Bus, or plugins loaded through a C ABI.

## Decision

Modules are compiled into the application. Each one declares what it requires and which panels it
provides, and it is active only when its requirements are detected.

## Consequences

- There is one package and no `unsafe` code. Modules cannot get out of step with the application.
- Supporting a new compositor or component requires a new release of the application.
- Packages such as `xunil-setting-hyprland` and installing modules separately are out of scope for
  now. This changes the original project brief.
- `xunil-setting-core` must not depend on any concrete module's internals, so that helper processes
  remain possible later without rewriting the core.
