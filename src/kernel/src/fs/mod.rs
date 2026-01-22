//! Filesystem Traits and Types.

/// Error type for filesystem operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    /// File or directory not found.
    NotFound,
    /// Permission denied.
    PermissionDenied,
    /// Invalid file handle.
    InvalidHandle,
}

/// A handle to an open file or directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileHandle(pub u32);

/// Trait for a filesystem.
pub trait FileSystem {
    /// Open a file by path.
    fn open(&self, path: &str) -> Result<FileHandle, FsError>;

    /// Open a file relative to a directory handle.
    fn open_at(&self, base: FileHandle, path: &str) -> Result<FileHandle, FsError>;

    /// Create a new directory.
    fn mkdir(&self, path: &str) -> Result<(), FsError>;

    /// Create a new directory relative to an existing directory handle.
    fn mkdir_at(&self, base: FileHandle, path: &str) -> Result<(), FsError>;

    /// Read from an open file.
    fn read(&self, handle: FileHandle, buffer: &mut [u8], offset: usize) -> Result<usize, FsError>;

    /// Get file size.
    fn size(&self, handle: FileHandle) -> Result<usize, FsError>;

    /// Check if a handle refers to a directory.
    fn is_dir(&self, handle: FileHandle) -> bool;

    /// Close a file handle.
    fn close(&self, handle: FileHandle);
}

// Global FS instance
use self::ramfs::RamFs;
use lazy_static::lazy_static;

pub mod ramfs;

lazy_static! {
    /// The root filesystem.
    pub static ref ROOT_FS: RamFs = RamFs::new();
}
