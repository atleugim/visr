# visr

A native desktop app for the terminals that herdr-core runs.

Visr does not own the terminals. herdr-core does: it starts the programs, keeps their output and
history, and reports their status (working, blocked, done). Visr is the window onto them. It draws
the terminals, lays them out in tabs and splits, and shows at a glance which ones need a person.
Closing a pane only stops showing a terminal; it keeps running in herdr-core until it ends or is
killed.

Visr is built with Rust and [GPUI](https://github.com/zed-industries/zed), and runs on macOS and
Windows.

## herdr-core

Visr is not an official herdr project. It is an independent app that talks to herdr-core.

herdr-core is required. When the app opens and herdr-core is missing or out of date, Visr offers
to install it.

## Themes

Visr follows the system appearance by default, and you can pick another theme in `config.toml`. See
[docs/themes.md](docs/themes.md) for the format and how to write your own.

## Download

Builds are published on the
[GitHub releases](https://github.com/atleugim/visr/releases) page. Every release has:

- `visr-vX.Y.Z-macos-universal.dmg`, for Apple Silicon and Intel Macs
- `visr-vX.Y.Z-windows-x64.exe` and `visr-vX.Y.Z-windows-arm64.exe`, the Windows installers
- `visr-vX.Y.Z-licenses.zip`, with the licenses of Visr, its fonts and its icons
- `SHA256SUMS`, with the checksum of each file

### Open it on macOS

The app is signed ad hoc and not notarized, so macOS asks for confirmation the first time. Open the
dmg, drag Visr to Applications, then right-click Visr and choose Open, and confirm in the dialog.
After that it opens normally.

### Open it on Windows

The installers are not signed, so Windows SmartScreen may warn you. To run it
anyway, choose More info, then Run anyway.

## Build

You need [rustup](https://rustup.rs). The Rust version, components and targets are pinned in
`rust-toolchain.toml`, and rustup installs them on the first `cargo` call. You can also run
`rustup toolchain install`, which is what CI does.

```sh
cargo build --release -p visr
```

The binary is `target/release/visr` (`visr.exe` on Windows). To run it straight from the source:

```sh
cargo run -p visr
```

On macOS, `scripts/bundle-macos.sh` builds `target/release/Visr.app`, and `scripts/dmg-macos.sh`
packs it into a disk image.

See [CONTRIBUTING.md](CONTRIBUTING.md) if you want to send a change.

## License

Licensed under the GNU General Public License, version 3 or (at your option) any later version
([LICENSE](LICENSE) or <https://www.gnu.org/licenses/gpl-3.0.html>).

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
the work by you shall be licensed as above, without any additional terms or conditions.
