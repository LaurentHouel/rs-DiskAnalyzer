# rs-DiskAnalyzer

Cross-platform (macOS, Windows, Linux) disk usage analyzer written in Rust with [egui](https://github.com/emilk/egui).

- Fast parallel scan of any folder, started only when you choose one: a mounted drive (internal SSD/HDD, removable, network, optical, with free space), the home folder, a folder picker or a recent folder
- Folder tree with size, item count and modification date; click a column header to sort, drag the edge of a header to resize columns (double-click to reset)
- Treemap and rings charts, synchronized with the tree
- Session activity log in its own window, including a diagnostic after 10 seconds without scan progress
- Move files and folders to the trash (context menu, toolbar button, `Delete` key) or delete them permanently (context menu, with confirmation); an information window shows the full path, size, shares and modification date
- Languages: English, Français, Español, Deutsch (English by default, switchable in the toolbar; your choice is remembered)

## Download

Pre-built packages for macOS (Apple Silicon / Intel), Windows and Linux (x86_64 / aarch64) are attached to each [release](../../releases):

- **macOS:** a `.dmg` containing `Disk Analyzer.app` and an Applications shortcut (double-click, no Terminal window). It is not notarized: on first launch, right-click → Open.
- **Windows:** a standalone `rs-disk-analyzer.exe` GUI executable without a console window, plus a zip archive. The archive includes PowerShell scripts to add or remove a File Explorer context-menu action for folders and drives.
- **Linux:** `.deb`, `.AppImage` and `.flatpak` packages, plus a tarball. The Flatpak requests full filesystem access so scans and file removal work.

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

### Windows Explorer context menu

Extract the Windows zip, open PowerShell in the extracted folder, and run:

```powershell
powershell -ExecutionPolicy Bypass -File .\install-context-menu.ps1
```

This registers **Scan with Disk Analyzer** for folders, drives, and the background of an open folder for the current Windows user; no administrator access is required. On Windows 11, the action may be under **Show more options**. To remove it, run:

```powershell
powershell -ExecutionPolicy Bypass -File .\uninstall-context-menu.ps1
```

## Shortcuts

| Action | Shortcut |
| --- | --- |
| Parent folder | `Backspace` or right-click on an empty area of a chart |
| Item menu (show in file manager, more information, move to trash, delete permanently) | right-click on a chart item or a list row |
| Zoom into a folder | double-click |
| Move to trash | `Delete` (`⌘ Backspace` on macOS) |

## Releasing

Push a tag such as `v0.2.0`: the *Release* workflow builds every platform and publishes the archives.

## Adding a language

Add a variant to `Lang` and one column to each entry of `tr` in [src/i18n.rs](src/i18n.rs).
