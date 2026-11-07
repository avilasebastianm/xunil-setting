# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Empty application window, translated into Spanish.
- Package, CI and project documentation.
- Module registry with detection of binaries and environment variables.
- Sidebar with the panels of the active modules, starting with a placeholder for Hyprland.
- Safe changes to configuration files: preview as a diff, atomic writes, a backup before the first
  change, and a restore that brings back the same bytes and permissions.
- Recognition of the answers system services give when they ask polkit for permission.
- Client of the date and time service, which reads the settings and asks for permission to
  change the time zone, the network time and the clock.
