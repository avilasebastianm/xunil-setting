# 8. Get privileges through system services

Status: accepted (2026-11-05)

## Context

Some settings need administrator rights, and these must go through polkit. The options were to ask
the system service that owns each setting, which checks polkit itself, or to ship our own helper that
runs as root through `pkexec`.

The system panels of the first version are covered by services: `timedated` for date and time,
`localed` for region, `logind` for power and NetworkManager for network. BlueZ does not use polkit.

## Decision

The application never runs code as root. It calls the system service over D-Bus, and the service asks
polkit for permission.

- Every call that may need permission sets the D-Bus flag that allows interactive authorization.
  Without the flag, a service answers as if no polkit agent were running.
- `xunil-setting-core` turns the error names of these services into a failure the interface can
  explain: denied, cancelled, or no polkit agent to ask for the password.

## Consequences

- There is no privileged code of ours to audit, and no polkit policy file to ship.
- A setting that only exists as a file in `/etc`, with no service behind it, is out of scope until a
  new ADR adds a helper.
- A polkit agent may be missing, because Hyprland does not start one. The interface must explain this
  instead of showing a generic error.
- Some services report a closed password dialog as denied, so the interface must not word a denial as
  a wrong password or as a decision by an administrator.
- The package gains the `polkit` dependency together with the first panel that needs it.
