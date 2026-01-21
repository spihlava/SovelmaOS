//! RAM Filesystem implementation.

use super::{FileHandle, FileSystem, FsError};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;
use core::sync::atomic::{AtomicU32, Ordering};

/// A simple in-memory filesystem.
pub struct RamFs {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    open_handles: Mutex<BTreeMap<FileHandle, (String, usize)>>, // Handle -> (Path, CurrentOffset)
}

impl RamFs {
    /// Create a new empty RAM filesystem.
    pub fn new() -> Self {
        Self {
            files: Mutex::new(BTreeMap::new()),
            open_handles: Mutex::new(BTreeMap::new()),
        }
    }

    /// Add a file to the filesystem.
    pub fn add_file(&self, path: &str, content: &[u8]) {
        self.files.lock().insert(String::from(path), content.to_vec());
    }
}

impl Default for RamFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystem for RamFs {
    fn open(&self, path: &str) -> Result<FileHandle, FsError> {
        let files = self.files.lock();
        if files.contains_key(path) {
            static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);
            let handle = FileHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
            
            self.open_handles.lock().insert(handle, (String::from(path), 0));
            Ok(handle)
        } else {
            Err(FsError::NotFound)
        }
    }

    fn read(&self, handle: FileHandle, buffer: &mut [u8], offset: usize) -> Result<usize, FsError> {
        let handles = self.open_handles.lock();
        if let Some((path, _)) = handles.get(&handle) {
            let files = self.files.lock();
            if let Some(content) = files.get(path) {
                if offset >= content.len() {
                    return Ok(0);
                }
                let end = core::cmp::min(offset + buffer.len(), content.len());
                let bytes_read = end - offset;
                buffer[..bytes_read].copy_from_slice(&content[offset..end]);
                Ok(bytes_read)
            } else {
                Err(FsError::NotFound) // Should not happen if handle exists
            }
        } else {
            Err(FsError::InvalidHandle)
        }
    }

    fn size(&self, handle: FileHandle) -> Result<usize, FsError> {
        let handles = self.open_handles.lock();
        if let Some((path, _)) = handles.get(&handle) {
            let files = self.files.lock();
            files.get(path).map(|f| f.len()).ok_or(FsError::NotFound)
        } else {
            Err(FsError::InvalidHandle)
        }
    }

    fn close(&self, handle: FileHandle) {
        self.open_handles.lock().remove(&handle);
    }
}
