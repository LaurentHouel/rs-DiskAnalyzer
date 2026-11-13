use rayon::prelude::*;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Instant, SystemTime};
#[cfg(unix)]
use std::{collections::HashSet, sync::Mutex};

/// A file or directory. Children are sorted by decreasing size.
pub struct Node {
    pub name: String,
    pub size: u64,
    /// Number of descendants for directories, 1 for files.
    pub items: u64,
    pub modified: Option<SystemTime>,
    pub is_dir: bool,
    pub children: Vec<Node>,
}

impl Node {
    pub fn get(&self, path: &[usize]) -> Option<&Node> {
        let mut node = self;
        for &i in path {
            node = node.children.get(i)?;
        }
        Some(node)
    }

    /// Removes the node reached through `path` and updates sizes and order of its ancestors.
    pub fn remove(&mut self, path: &[usize]) -> Option<()> {
        let (&first, rest) = path.split_first()?;
        let (size, items) = if rest.is_empty() {
            if first >= self.children.len() {
                return None;
            }
            let c = self.children.remove(first);
            (c.size, if c.is_dir { c.items + 1 } else { 1 })
        } else {
            let child = self.children.get_mut(first)?;
            let before = (child.size, child.items);
            child.remove(rest)?;
            (before.0 - child.size, before.1 - child.items)
        };
        self.size = self.size.saturating_sub(size);
        self.items = self.items.saturating_sub(items);
        sort_children(&mut self.children);
        Some(())
    }

    /// Inverse of [`Node::path_of`]: finds the index path of a filesystem path.
    pub fn resolve(&self, root_path: &Path, target: &Path) -> Option<Vec<usize>> {
        let rel = target.strip_prefix(root_path).ok()?;
        let mut node = self;
        let mut out = Vec::new();
        for comp in rel.components() {
            let name = comp.as_os_str().to_string_lossy();
            let i = node.children.iter().position(|c| c.name == name)?;
            out.push(i);
            node = &node.children[i];
        }
        Some(out)
    }

    /// Filesystem path of the node reached through `path`, given the scanned root's path.
    pub fn path_of(&self, root_path: &Path, path: &[usize]) -> PathBuf {
        let mut out = root_path.to_path_buf();
        let mut node = self;
        for &i in path {
            let Some(child) = node.children.get(i) else {
                break;
            };
            out.push(&child.name);
            node = child;
        }
        out
    }
}

#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
}

pub struct ScanHandle {
    pub path: PathBuf,
    pub progress: Arc<Progress>,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<Node>,
    pub started: Instant,
}

struct Shared {
    progress: Arc<Progress>,
    cancel: Arc<AtomicBool>,
    #[cfg(unix)]
    seen_inodes: Mutex<HashSet<(u64, u64)>>,
    #[cfg_attr(not(unix), allow(dead_code))]
    root_dev: u64,
}

pub fn start(path: PathBuf, ctx: egui::Context) -> ScanHandle {
    let progress = Arc::new(Progress::default());
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();

    let handle = ScanHandle {
        path: path.clone(),
        progress: progress.clone(),
        cancel: cancel.clone(),
        rx,
        started: Instant::now(),
    };

    std::thread::spawn(move || {
        let meta = fs::symlink_metadata(&path);
        let shared = Shared {
            progress,
            cancel: cancel.clone(),
            #[cfg(unix)]
            seen_inodes: Mutex::new(HashSet::new()),
            root_dev: meta.as_ref().map(device_id).unwrap_or(0),
        };
        let name = path.to_string_lossy().into_owned();
        let root = match meta {
            Ok(meta) if meta.is_dir() => scan_dir(&path, name, &meta, &shared),
            Ok(meta) => file_node(name, &meta, &shared),
            Err(_) => Node {
                name,
                size: 0,
                items: 0,
                modified: None,
                is_dir: true,
                children: Vec::new(),
            },
        };
        if !cancel.load(Ordering::Relaxed) {
            let _ = tx.send(root);
        }
        ctx.request_repaint();
    });

    handle
}

fn scan_dir(path: &Path, name: String, meta: &Metadata, shared: &Shared) -> Node {
    shared.progress.dirs.fetch_add(1, Ordering::Relaxed);

    let entries: Vec<fs::DirEntry> = match fs::read_dir(path) {
        Ok(rd) => rd.flatten().collect(),
        Err(_) => {
            shared.progress.errors.fetch_add(1, Ordering::Relaxed);
            Vec::new()
        }
    };

    let mut children: Vec<Node> = entries
        .par_iter()
        .filter_map(|entry| {
            if shared.cancel.load(Ordering::Relaxed) {
                return None;
            }
            // DirEntry::metadata does not follow symlinks.
            let Ok(md) = entry.metadata() else {
                shared.progress.errors.fetch_add(1, Ordering::Relaxed);
                return None;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if md.file_type().is_dir() {
                let child = entry.path();
                if device_id(&md) != shared.root_dev || is_mirrored_mount(&child) {
                    return None;
                }
                Some(scan_dir(&child, name, &md, shared))
            } else {
                Some(file_node(name, &md, shared))
            }
        })
        .collect();

    sort_children(&mut children);

    let size = disk_size(meta, shared) + children.iter().map(|c| c.size).sum::<u64>();
    let items = children
        .iter()
        .map(|c| if c.is_dir { c.items + 1 } else { 1 })
        .sum();

    Node {
        name,
        size,
        items,
        modified: meta.modified().ok(),
        is_dir: true,
        children,
    }
}

/// Canonical order: decreasing size. Charts and index paths rely on it.
fn sort_children(children: &mut [Node]) {
    children.sort_unstable_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
}

fn file_node(name: String, meta: &Metadata, shared: &Shared) -> Node {
    let size = disk_size(meta, shared);
    shared.progress.files.fetch_add(1, Ordering::Relaxed);
    shared.progress.bytes.fetch_add(size, Ordering::Relaxed);
    Node {
        name,
        size,
        items: 1,
        modified: meta.modified().ok(),
        is_dir: false,
        children: Vec::new(),
    }
}

/// On macOS the data volume is reachable twice: through the firmlinks (/Users, /Applications…)
/// and under /System/Volumes, on the same device id. Counting both doubles the totals.
fn is_mirrored_mount(path: &Path) -> bool {
    cfg!(target_os = "macos") && path == Path::new("/System/Volumes")
}

#[cfg(unix)]
fn device_id(meta: &Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::dev(meta)
}

#[cfg(not(unix))]
fn device_id(_meta: &Metadata) -> u64 {
    0
}

/// Space actually used on disk; hard-linked files are only counted once.
#[cfg(unix)]
fn disk_size(meta: &Metadata, shared: &Shared) -> u64 {
    use std::os::unix::fs::MetadataExt;
    if !meta.is_dir() && meta.nlink() > 1 {
        let mut seen = shared.seen_inodes.lock().unwrap();
        if !seen.insert((meta.dev(), meta.ino())) {
            return 0;
        }
    }
    meta.blocks() * 512
}

#[cfg(not(unix))]
fn disk_size(meta: &Metadata, _shared: &Shared) -> u64 {
    meta.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_system_volumes_mirror_is_skipped() {
        assert_eq!(
            is_mirrored_mount(Path::new("/System/Volumes")),
            cfg!(target_os = "macos")
        );
        assert!(!is_mirrored_mount(Path::new("/Users")));
    }

    #[test]
    fn scans_sizes_counts_and_sorts() {
        let dir = std::env::temp_dir().join(format!("da-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("small.bin"), vec![1u8; 10]).unwrap();
        fs::write(dir.join("sub/big.bin"), vec![1u8; 200_000]).unwrap();

        let ctx = egui::Context::default();
        let handle = start(dir.clone(), ctx);
        let root = handle.rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();

        assert_eq!(root.items, 3);
        assert_eq!(root.children[0].name, "sub");
        assert_eq!(root.children[0].children[0].name, "big.bin");
        assert!(root.size >= 200_000);

        let big = root.resolve(&dir, &dir.join("sub/big.bin")).unwrap();
        assert_eq!(big, vec![0, 0]);
        let (total, big_size) = (root.size, root.get(&big).unwrap().size);
        let mut root = root;
        root.remove(&big).unwrap();
        assert_eq!(root.size, total - big_size);
        assert_eq!(root.items, 2);
        assert_eq!(root.children[0].name, "small.bin");
        assert!(root.resolve(&dir, &dir.join("sub/big.bin")).is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
