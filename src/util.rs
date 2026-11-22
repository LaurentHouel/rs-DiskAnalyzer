use cap_std::ambient_authority;
use cap_std::fs::Dir;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

/// Opens the system file manager with `path` selected.
///
/// # Parameters
/// - `path`: File or directory to reveal.
///
/// # Returns
/// Nothing; starts the platform-specific file manager process when supported.
pub fn reveal_in_file_manager(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();
}

/// Removes a file or folder without going through the trash. Symlinks are removed, not followed.
/// `on_path` is called with each file or directory as it is removed.
///
/// # Parameters
/// - `path`: File or directory to remove.
/// - `cancel`: Atomic flag checked between filesystem operations.
/// - `on_path`: Callback invoked with each path as deletion progresses.
///
/// # Returns
/// `Ok(())` when deletion completes, or an I/O error if deletion fails or is cancelled.
pub fn delete_permanently(
    path: &Path,
    cancel: &AtomicBool,
    mut on_path: impl FnMut(&Path),
) -> std::io::Result<()> {
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name")
    })?;
    let parent = Dir::open_ambient_dir(parent_path, ambient_authority())?;
    remove_entry(&parent, Path::new(name), path, cancel, &mut on_path)
}

/// Removes one directory entry, recursively visiting directory contents without following symlinks.
///
/// # Parameters
/// - `parent`: Capability directory containing the entry.
/// - `name`: Entry name relative to `parent`.
/// - `display_path`: Full path supplied to the progress callback.
/// - `cancel`: Atomic cancellation flag checked between operations.
/// - `on_path`: Callback invoked before entries are removed.
///
/// # Returns
/// `Ok(())` when the entry is removed, or the first I/O or cancellation error.
fn remove_entry(
    parent: &Dir,
    name: &Path,
    display_path: &Path,
    cancel: &AtomicBool,
    on_path: &mut impl FnMut(&Path),
) -> std::io::Result<()> {
    check_delete_cancelled(cancel)?;
    if parent.symlink_metadata(name)?.is_dir() {
        on_path(display_path);
        check_delete_cancelled(cancel)?;
        let dir = parent.open_dir(name)?;
        check_delete_cancelled(cancel)?;
        for entry in dir.read_dir(".")? {
            let entry = entry?;
            let file_name = entry.file_name();
            let child_name = Path::new(&file_name);
            let child_path = display_path.join(child_name);
            remove_entry(&dir, child_name, &child_path, cancel, on_path)?;
        }
        on_path(display_path);
        check_delete_cancelled(cancel)?;
        dir.remove_open_dir()
    } else {
        on_path(display_path);
        check_delete_cancelled(cancel)?;
        parent.remove_file(name)
    }
}

/// Converts a set cancellation flag into an interrupted I/O error.
///
/// # Parameters
/// - `cancel`: Atomic flag indicating whether deletion should stop.
///
/// # Returns
/// `Ok(())` if deletion may continue, otherwise an `Interrupted` error.
fn check_delete_cancelled(cancel: &AtomicBool) -> std::io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "deletion cancelled",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    /// Checks that progress callbacks include nested files and that the tree is removed.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if progress omits a nested file or deletion is incomplete.
    fn permanent_delete_reports_nested_files_as_it_removes_them() {
        let root = std::env::temp_dir().join(format!(
            "rs-da-delete-progress-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let file = nested.join("file.bin");
        std::fs::write(&file, b"test").unwrap();

        let mut removed = Vec::new();
        delete_permanently(&root, &AtomicBool::new(false), |path| {
            removed.push(path.to_path_buf())
        })
        .unwrap();

        assert!(removed.contains(&file));
        assert!(!root.exists());
    }

    #[test]
    /// Checks that deletion stops between filesystem entries after cancellation is requested.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if cancellation is not reported or both files have the same state.
    fn permanent_delete_stops_between_items_when_cancelled() {
        let root = std::env::temp_dir().join(format!(
            "rs-da-delete-cancel-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let first = root.join("first.bin");
        let second = root.join("second.bin");
        std::fs::write(&first, b"first").unwrap();
        std::fs::write(&second, b"second").unwrap();
        let cancel = AtomicBool::new(false);
        let mut files_seen = 0;

        let error = delete_permanently(&root, &cancel, |path| {
            if path.extension().is_some_and(|ext| ext == "bin") {
                files_seen += 1;
                if files_seen == 2 {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        })
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
        assert_ne!(first.exists(), second.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    /// Checks that deleting a symlink removes the link without deleting its target.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if the link target is followed or removed.
    fn permanent_delete_removes_symlinks_without_following_them() {
        let base = std::env::temp_dir().join(format!(
            "rs-da-delete-symlink-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = base.join("root");
        let outside = base.join("outside.bin");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&outside, b"preserve").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();

        delete_permanently(&root, &AtomicBool::new(false), |_| {}).unwrap();

        assert!(outside.exists());
        std::fs::remove_file(outside).unwrap();
        std::fs::remove_dir(base).unwrap();
    }
}
