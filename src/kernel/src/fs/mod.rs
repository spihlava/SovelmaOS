//! Filesystem Traits and Types.

use alloc::vec::Vec;
use alloc::string::String;

/// Error type for filesystem operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    PermissionDenied,
    InvalidHandle,
}

/// A handle to an open file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileHandle(pub u32);

/// Trait for a filesystem.
pub trait FileSystem {
    /// Open a file by path.
    fn open(&self, path: &str) -> Result<FileHandle, FsError>;
    
    /// Read from an open file.
    fn read(&self, handle: FileHandle, buffer: &mut [u8], offset: usize) -> Result<usize, FsError>;
    
    /// Get file size.
    fn size(&self, handle: FileHandle) -> Result<usize, FsError>;
    
    /// Close a file handle.
    fn close(&self, handle: FileHandle);
}


// Global FS instance
use lazy_static::lazy_static;
use self::ramfs::RamFs;

pub mod ramfs;

lazy_static! {
    /// The root filesystem.
    pub static ref ROOT_FS: RamFs = RamFs::new();
}

