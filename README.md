# Xunil Setting

A graphical settings center for Arch Linux, in the spirit of GNOME Settings, that works on any
installation whichever desktop you use.

> **Status:** early development. The application only opens an empty window so far.
> This project is not affiliated with or endorsed by Arch Linux.

## Principles

1. It works without any particular dotfiles.
2. It only shows what applies: a section whose requirement is missing is hidden or explained.
3. Your configuration is yours: every change can be previewed and undone.
4. It is visual and simple, and follows the GNOME Human Interface Guidelines.
5. Nothing is calibrated to one machine.

## Build

You need `rustup`, `gtk4`, `libadwaita` and `gettext`.

```sh
cargo run -p xunil-setting
```

To build and install the package:

```sh
cd packaging
makepkg --syncdeps --install
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Decisions are recorded in [docs/adr](docs/adr).

## License

[GPL-3.0-or-later](LICENSE).
