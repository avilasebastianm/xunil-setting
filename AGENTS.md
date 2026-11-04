# Instructions for AI agents

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/adr](docs/adr) first. Those rules apply to you too.

The ones that matter most:

- Never run the tests or the application in a way that touches the real session. Use a temporary
  `HOME` and `XDG_*`, and no real services.
- Never kill processes by name, and never run a program you have not read, not even with `--help`.
- Never overwrite a user's configuration without showing the change and offering a way to undo it.
- Assume nothing about the machine: no fixed paths, monitors, users, themes or dotfiles.
- Keep logic in `xunil-setting-core`, without GTK.
- Work on a `feature/*` or `bugfix/*` branch from `develop`, with Conventional Commits.
- Add tests with every change, and run the commands listed in `CONTRIBUTING.md`.
