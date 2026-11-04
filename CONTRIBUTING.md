# Contributing

These rules are mandatory. CI enforces them wherever it can.

## Git flow

| Branch | Comes from | Merges into |
|---|---|---|
| `main`: released code, tagged `vX.Y.Z` | | |
| `develop`: next release | `main` | |
| `feature/<topic>`, `bugfix/<topic>` | `develop` | `develop` |
| `release/<version>` | `develop` | `main`, then `develop` |
| `hotfix/<topic>` | `main` | `main`, then `develop` |

- `main` and `develop` are protected: everything enters through a pull request with green CI.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/), small and on a single
  topic.
- Versions follow SemVer, and `CHANGELOG.md` follows Keep a Changelog.
- Important decisions are recorded as ADRs in `docs/adr/`.
- No personal data in the repository: no paths with user names, tokens or data from one machine.

## Language

Code, commits and documentation are in English. Every visible string goes through gettext, and the
interface ships in at least English and Spanish.

## Tests

- Every new function comes with tests, and every bug fix with a test that fails without the fix.
- **Tests never touch the real session.** They use a temporary `HOME` and `XDG_*`, never restart or
  kill user processes, and never change the user's configuration.
- The test pyramid:
  - **Unit:** model logic, without GTK. These are the majority.
  - **Integration:** fake D-Bus services and simulated compositors, never the real services.
  - **Interface:** one test per panel that confirms it opens without errors, with no real screen
    (`scripts/ui-tests.sh`).
  - **Installation:** build and install the package on a clean Arch and open it without Hyprland or
    Waybar (the `package` CI job).
- Coverage: at least 80 % of the core and of the lines changed by each pull request. A module that
  falls short needs a written justification.

## Quality

Before opening a pull request:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo deny --locked check
cargo test --workspace --locked
scripts/ui-tests.sh
```

- Logic goes in `arch-setting-core`, which must not depend on GTK.
- No `unsafe` code.
- External processes run with an argument list, never a shell command built from user text.
- Files with secrets are created with mode 600, and secrets never reach the logs.
- Privileges go through polkit: never `sudo` and never a custom password dialog.
- Everything is usable with the keyboard and has labels for screen readers.
- Configuration writes are atomic, with a backup before the first change and a way to restore it.
