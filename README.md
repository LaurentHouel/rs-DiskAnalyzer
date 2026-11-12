# rs-DiskAnalyzer

Cross-platform (macOS, Windows, Linux) disk usage analyzer written in Rust with [egui](https://github.com/emilk/egui).

- Fast parallel scan of any folder, started only when you choose one (home folder, folder picker or recent folders)
- Folder tree with size, item count and modification date; click a column header to sort
- Treemap and rings charts, synchronized with the tree
- Move files and folders to the trash (context menu, toolbar button, `Delete` key)
- Languages: English, Français, Español, Deutsch (detected from the system, switchable in the toolbar)

## Download

Pre-built binaries for macOS (Apple Silicon / Intel), Windows and Linux (x86_64 / aarch64) are attached to each [release](../../releases).

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

The app does not scan anything at startup; it accepts an optional folder as first argument to scan it right away (`cargo run -- /some/folder`). Dependencies are compiled with optimizations in dev builds so the UI stays responsive, while this crate stays fully debuggable.

## Shortcuts

| Action | Shortcut |
| --- | --- |
| Parent folder | `Backspace` or right-click on a chart |
| Zoom into a folder | double-click |
| Move to trash | `Delete` (`⌘ Backspace` on macOS) |

## Releasing

Push a tag such as `v0.2.0`: the *Release* workflow builds every platform and publishes the archives.

## Adding a language

Add a variant to `Lang` and one column to each entry of `tr` in [src/i18n.rs](src/i18n.rs).
