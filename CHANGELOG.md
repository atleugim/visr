# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Cmd+Q quits Visr on macOS, and Ctrl+Shift+Q on Windows
- About Visr in the Visr menu on macOS shows the app and herdr-core versions
- Group tabs and terminals into folders in the sidebar, and collapse, rename or reorder them,
  and a terminal from another folder opens in a tab of its own
- Make a new folder with Cmd+Option+N / Ctrl+Shift+O, and move the active tab into one with
  Cmd+Shift+N / Ctrl+Shift+N
- Collapse or expand the active tab's folder with Cmd+Option+F / Ctrl+Shift+F, and take the tab
  out of its folder with Cmd+Option+U / Ctrl+Shift+U
- A collapsed folder shows how many of its sessions are blocked
- Drag sessions and folders in the sidebar to reorder them, or to move sessions into and out
  of folders

### Fixed

- A tab whose program ends stays where it was in the sidebar, instead of moving to the end
- Renaming a terminal while no tab is open takes your typing right away, without a click
  first

## [0.1.1] - 2026-11-16

### Added

- Install Visr on Windows with a setup that adds it to the Start menu and the Apps list

### Fixed

- Terminals no longer flash when you create a tab or switch between tabs

## [0.1.0] - 2026-11-15

### Added

- Terminals from herdr-core in tabs and splits
- Visr offers to install herdr-core when it is missing or out of date
- File and changes panels for the active terminal's folder, updated when a command runs or finishes
- Open a terminal's folder in an installed editor
- Changes to `config.toml`, including the `theme` line, apply as soon as you save it
- Dark and light built-in themes that follow the system appearance, and a `theme` setting in `config.toml` to pick one
- Reload the theme with a shortcut, so edits to its file show without a restart
- Your own themes in the `themes` folder next to `config.toml`

[Unreleased]: https://github.com/atleugim/visr/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/atleugim/visr/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/atleugim/visr/commits/v0.1.0
