# rs-DiskAnalyzer

Cross-platform (macOS, Windows, Linux) disk usage analyzer written in Rust with [egui](https://github.com/emilk/egui).

- Fast parallel scan of any folder, started only when you choose one: a mounted drive (internal SSD/HDD, removable, network, optical, with free space), the home folder, a folder picker or a recent folder
- Folder tree with size, item count and modification date; click a column header to sort, drag the edge of a header to resize columns (double-click to reset)
- Treemap and rings charts, synchronized with the tree
- Move files and folders to the trash (context menu, toolbar button, `Delete` key) or delete them permanently (context menu, with confirmation); an information window shows the full path, size, shares and modification date
- Languages: English, Français, Español, Deutsch (English by default, switchable in the toolbar; your choice is remembered)

## Download

Pre-built packages for macOS (Apple Silicon / Intel), Windows and Linux (x86_64 / aarch64) are attached to each [release](../../releases):

- **macOS:** a `Disk Analyzer.app` bundle (double-click, no Terminal window). It is not notarized: on first launch, right-click → Open. To build it yourself: `cargo build --release && scripts/bundle-macos.sh target/release/rs-disk-analyzer`.
- **Windows:** `rs-disk-analyzer.exe`, a GUI executable without console window.
- **Linux:** the binary plus a `.desktop` entry (`Terminal=false`).

Linux x86_64 packages are built on Ubuntu 22.04 (glibc 2.35); aarch64 packages
are built on Ubuntu 24.04 (glibc 2.39). A graphical desktop session is required.
The **v0.1.3 x86_64** package was built on Ubuntu 26.04 and requires glibc 2.43,
so it cannot run on Ubuntu 22.04. The corrected build configuration applies to
subsequent builds, not the existing v0.1.3 download.

If double-clicking does nothing, open a terminal in the extracted directory and run:

```sh
./rs-disk-analyzer
```

This displays startup errors, including missing libraries or an incompatible
glibc version. Do not manually replace the system glibc to run the application.

## Build

```sh
cargo run --release
```

On Linux, the build needs `libxkbcommon-dev libwayland-dev libxcb1-dev libgl1-mesa-dev`.

## Debugging (VS Code)

Install the recommended extensions (`rust-analyzer`, `CodeLLDB`), set a breakpoint, then press `F5` and pick a configuration from [.vscode/launch.json](.vscode/launch.json):

- *start screen (no scan)*, *scan home folder*, *scan ./src (small, fast)* or *scan a folder (prompt)*
- *unit tests*
- *Windows, MSVC debugger* (uses the C/C++ extension)

The app does not scan anything at startup; only the explicit option `--scan <folder>` starts a scan at launch (`cargo run -- --scan /some/folder`); any other argument is ignored. Dependencies are compiled with optimizations in dev builds so the UI stays responsive, while this crate stays fully debuggable.

## Shortcuts

| Action | Shortcut |
| --- | --- |
| Parent folder | `Backspace` or right-click on an empty area of a chart |
| Item menu (show in file manager, more information, move to trash, delete permanently) | right-click on a chart item or a list row |
| Zoom into a folder | double-click |
| Move to trash | `Delete` (`⌘ Backspace` on macOS) |

## Releasing

Push a tag such as `v0.2.0`: the *Release* workflow builds every platform and publishes the archives.
Keep the Linux x86_64 build pinned to Ubuntu 22.04 to preserve its glibc baseline.
The workflow rejects x86_64 binaries requiring a glibc version newer than 2.35.

## Adding a language

Add a variant to `Lang` and one column to each entry of `tr` in [src/i18n.rs](src/i18n.rs).
