# 3. License: GPL-3.0-or-later

Status: accepted (2026-11-04)

## Context

This kind of program is usually under the GPL. GNOME Settings, the functional reference, is
GPL-2.0-or-later, so its code could be reused under GPL-3.0-or-later if its notices are kept.

## Decision

Use GPL-3.0-or-later.

## Consequences

Derived versions stay free. `cargo deny` checks that dependency licenses are on an allowed list.
