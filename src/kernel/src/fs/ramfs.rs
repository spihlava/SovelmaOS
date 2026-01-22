//! RAM Filesystem implementation (Hierarchical).

use super::{FileHandle, FileSystem, FsError};
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::{Mutex, RwLock}; // Use RwLock for nodes

#[derive(Clone)]
enum Node {
    File(Vec<u8>),
    Directory(BTreeMap<String, Arc<RwLock<Node>>>),
}

/// A hierarchical in-memory filesystem.
pub struct RamFs {
    root: Arc<RwLock<Node>>,
    open_handles: Mutex<BTreeMap<FileHandle, Arc<RwLock<Node>>>>,
}

impl RamFs {
    /// Create a new empty RAM filesystem.
    pub fn new() -> Self {
        Self {
            root: Arc::new(RwLock::new(Node::Directory(BTreeMap::new()))),
            open_handles: Mutex::new(BTreeMap::new()),
        }
    }

    /// Add a file at a specific path (mkdir -p logic included).
    pub fn add_file(&self, path: &str, content: &[u8]) {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return;
        }

        let mut current = self.root.clone();

        // Traverse/Create directories
        for part in &parts[..parts.len() - 1] {
            let next_node = {
                let mut guard = current.write();
                if let Node::Directory(ref mut map) = *guard {
                    map.entry((*part).to_string())
                        .or_insert_with(|| Arc::new(RwLock::new(Node::Directory(BTreeMap::new()))))
                        .clone()
                } else {
                    return; // Error: Path component is not a directory
                }
            };
            current = next_node;
        }

        // Create File
        // Safety: parts is non-empty (checked above), so last() always succeeds
        let Some(filename) = parts.last() else {
            return; // Unreachable due to early return above, but satisfies no-unwrap rule
        };
        let mut guard = current.write();
        if let Node::Directory(ref mut map) = *guard {
            map.insert(
                (*filename).to_string(),
                Arc::new(RwLock::new(Node::File(content.to_vec()))),
            );
        }
    }

    fn resolve_path(&self, path: &str) -> Result<Arc<RwLock<Node>>, FsError> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = self.root.clone();
        for part in parts {
            let next = {
                let guard = current.read();
                match *guard {
                    Node::Directory(ref map) => map.get(part).cloned(),
                    _ => return Err(FsError::NotFound),
                }
            };
            if let Some(node) = next {
                current = node;
            } else {
                return Err(FsError::NotFound);
            }
        }
        Ok(current)
    }
}

impl Default for RamFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystem for RamFs {
    fn open(&self, path: &str) -> Result<FileHandle, FsError> {
        let node = self.resolve_path(path)?;

        static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);
        let handle = FileHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));

        self.open_handles.lock().insert(handle, node);
        Ok(handle)
    }

    fn open_at(&self, base: FileHandle, path: &str) -> Result<FileHandle, FsError> {
        let handles = self.open_handles.lock();
        let base_node = handles.get(&base).ok_or(FsError::InvalidHandle)?.clone();

        // Drop lock before traversing to avoid deadlocks if resolve_relative locks?
        // Actually resolve_relative only locks nodes, not open_handles.
        drop(handles);

        // Resolve relative
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = base_node;

        for part in parts {
            let next = {
                let guard = current.read();
                match *guard {
                    Node::Directory(ref map) => map.get(part).cloned(),
                    _ => return Err(FsError::NotFound),
                }
            };
            if let Some(node) = next {
                current = node;
            } else {
                return Err(FsError::NotFound);
            }
        }

        static NEXT_HANDLE_AT: AtomicU32 = AtomicU32::new(10000); // offset to distinguish?
        let handle = FileHandle(NEXT_HANDLE_AT.fetch_add(1, Ordering::Relaxed));

        self.open_handles.lock().insert(handle, current);
        Ok(handle)
    }

    fn mkdir(&self, path: &str) -> Result<(), FsError> {
        self.mkdir_at(FileHandle(0), path) // FileHandle(0) doesn't exist but we can handle root as base
    }

    fn mkdir_at(&self, base: FileHandle, path: &str) -> Result<(), FsError> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return Err(FsError::PermissionDenied);
        }

        let parent_parts = &parts[..parts.len() - 1];
        // Safety: parts is non-empty (checked above), so last() always succeeds
        let Some(dirname) = parts.last() else {
            return Err(FsError::PermissionDenied); // Unreachable, but satisfies no-unwrap rule
        };

        // Resolve parent
        let mut current = if base.0 == 0 {
            self.root.clone()
        } else {
            let handles = self.open_handles.lock();
            handles.get(&base).ok_or(FsError::InvalidHandle)?.clone()
        };

        for part in parent_parts {
            let next = {
                let guard = current.read();
                match *guard {
                    Node::Directory(ref map) => map.get(*part).cloned(),
                    _ => return Err(FsError::NotFound),
                }
            };
            if let Some(node) = next {
                current = node;
            } else {
                return Err(FsError::NotFound);
            }
        }

        // Create dir in parent
        let mut guard = current.write();
        if let Node::Directory(ref mut map) = *guard {
            if map.contains_key(*dirname) {
                return Err(FsError::PermissionDenied); // Already exists
            }
            map.insert(
                dirname.to_string(),
                Arc::new(RwLock::new(Node::Directory(BTreeMap::new()))),
            );
            Ok(())
        } else {
            Err(FsError::InvalidHandle) // Parent is not dir
        }
    }

    fn read(&self, handle: FileHandle, buffer: &mut [u8], offset: usize) -> Result<usize, FsError> {
        let handles = self.open_handles.lock();
        if let Some(node) = handles.get(&handle) {
            let guard = node.read();
            if let Node::File(ref content) = *guard {
                if offset >= content.len() {
                    return Ok(0);
                }
                let end = core::cmp::min(offset + buffer.len(), content.len());
                let bytes_read = end - offset;
                buffer[..bytes_read].copy_from_slice(&content[offset..end]);
                Ok(bytes_read)
            } else {
                Err(FsError::InvalidHandle) // Is a directory
            }
        } else {
            Err(FsError::InvalidHandle)
        }
    }

    fn size(&self, handle: FileHandle) -> Result<usize, FsError> {
        let handles = self.open_handles.lock();
        if let Some(node) = handles.get(&handle) {
            let guard = node.read();
            match *guard {
                Node::File(ref content) => Ok(content.len()),
                Node::Directory(_) => Ok(0), // Dirs have size 0 for now
            }
        } else {
            Err(FsError::InvalidHandle)
        }
    }

    fn is_dir(&self, handle: FileHandle) -> bool {
        let handles = self.open_handles.lock();
        if let Some(node) = handles.get(&handle) {
            let guard = node.read();
            matches!(*guard, Node::Directory(_))
        } else {
            false
        }
    }

    fn close(&self, handle: FileHandle) {
        self.open_handles.lock().remove(&handle);
    }
}
