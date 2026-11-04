# 1. Use Rust with GTK4 and libadwaita

Status: accepted (2026-11-04)

## Context

The project needs a native GTK4 and libadwaita application. The candidates were Rust and Python. A
Python prototype exists with reusable logic and tests.

## Decision

Use Rust, with `gtk4-rs` and `libadwaita-rs`. The maintainer chose it, among other reasons, for its
compile-time checks.

## Consequences

- The prototype is a behavioral reference only: its logic and tests must be rewritten.
- Rust has no stable ABI, so separately installed modules cannot simply be loaded as libraries. The
  brief's `xunil-setting-hyprland` style of package needs its own decision in phase 1. The options
  are helper processes, a C ABI, or modules compiled into the application and enabled at runtime.
- Workspace code forbids `unsafe`. Direct use of a C library needs an exception recorded in an ADR.
