use rayon::prelude::*;
use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

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
                if device_id(&md) != shared.root_dev {
                    return None;
                }
                Some(scan_dir(&entry.path(), name, &md, shared))
            } else {
                Some(file_node(name, &md, shared))
            }
        })
        .collect();

    children.sort_unstable_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));

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
        let _ = fs::remove_dir_all(&dir);
    }
}
