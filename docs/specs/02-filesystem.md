# SovelmaOS Filesystem Specification
## Version 0.1.0 - Draft

---

## 1. Overview

### 1.1 Purpose
This document specifies the Virtual Filesystem (VFS) layer and filesystem implementations for SovelmaOS OS.

### 1.2 Design Goals
| Priority | Goal |
|----------|------|
| 1 | Capability-based access control |
| 2 | Flash-friendly (wear leveling, power-loss safe) |
| 3 | Minimal kernel footprint |
| 4 | Consistent API across platforms |

---

## 2. Architecture

### 2.1 Layer Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                      WASM MODULES                               │
│                                                                 │
│   sp_fs_open() / sp_fs_read() / sp_fs_write() / sp_fs_close()  │
│                          │                                      │
├──────────────────────────┼──────────────────────────────────────┤
│                          ▼                                      │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │                 CAPABILITY CHECK                          │ │
│  │         Verify caller has FileCapability for path         │ │
│  └───────────────────────────────────────────────────────────┘ │
│                          │                                      │
│                          ▼                                      │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │                    VFS LAYER                              │ │
│  │                                                           │ │
│  │  • Path normalization (/foo/../bar → /bar)               │ │
│  │  • Mount table lookup                                    │ │
│  │  • File descriptor management                            │ │
│  │  • Route to filesystem driver                            │ │
│  └───────────────────────────────────────────────────────────┘ │
│                          │                                      │
│         ┌────────────────┼────────────────┐                    │
│         ▼                ▼                ▼                    │
│  ┌───────────┐    ┌───────────┐    ┌───────────┐              │
│  │  ramfs    │    │ littlefs  │    │  devfs    │              │
│  │ (memory)  │    │ (flash)   │    │ (devices) │              │
│  └───────────┘    └───────────┘    └───────────┘              │
│                          │                                      │
│                          ▼                                      │
│                   Flash Driver (HAL)                           │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. VFS Layer Specification

### 3.1 Data Structures

```rust
/// Maximum path length
pub const MAX_PATH: usize = 256;

/// Maximum open files per module
pub const MAX_OPEN_FILES: usize = 16;

/// Maximum mount points
pub const MAX_MOUNTS: usize = 8;

/// Virtual Filesystem
pub struct Vfs {
    /// Mount table
    mounts: [Option<Mount>; MAX_MOUNTS],
    /// Open file descriptors (global pool)
    files: Slab<OpenFile, 64>,
}

/// A mounted filesystem
pub struct Mount {
    /// Mount path (e.g., "/var")
    pub path: String<MAX_PATH>,
    /// Filesystem implementation
    pub fs: FsType,
    /// Read-only flag
    pub read_only: bool,
}

/// Supported filesystem types
pub enum FsType {
    /// RAM-based tmpfs
    Ramfs(Ramfs),
    /// Flash-based littlefs
    Littlefs(Littlefs),
    /// Device filesystem
    Devfs(Devfs),
}

/// An open file
pub struct OpenFile {
    /// Which mount this belongs to
    pub mount_idx: u8,
    /// Handle within the filesystem
    pub fs_handle: u32,
    /// Current offset
    pub offset: u64,
    /// Open flags
    pub flags: OpenFlags,
    /// Owning module
    pub owner: ModuleId,
}

bitflags! {
    pub struct OpenFlags: u32 {
        const READ      = 0x0001;
        const WRITE     = 0x0002;
        const CREATE    = 0x0004;
        const TRUNCATE  = 0x0008;
        const APPEND    = 0x0010;
        const EXCLUSIVE = 0x0020;
    }
}

/// File metadata
#[derive(Clone)]
pub struct FileStat {
    /// File size in bytes
    pub size: u64,
    /// File type
    pub kind: FileKind,
    /// Last modification time (ms since boot)
    pub mtime: u64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum FileKind {
    File,
    Directory,
    Device,
    Symlink,
}

/// Directory entry
#[derive(Clone)]
pub struct DirEntry {
    pub name: String<64>,
    pub kind: FileKind,
}
```

### 3.2 Filesystem Trait

```rust
/// Trait implemented by all filesystem drivers
pub trait Filesystem {
    /// Open a file
    fn open(&mut self, path: &str, flags: OpenFlags) -> Result<u32, FsError>;
    
    /// Read from an open file
    fn read(&mut self, handle: u32, offset: u64, buf: &mut [u8]) -> Result<usize, FsError>;
    
    /// Write to an open file
    fn write(&mut self, handle: u32, offset: u64, data: &[u8]) -> Result<usize, FsError>;
    
    /// Close a file
    fn close(&mut self, handle: u32) -> Result<(), FsError>;
    
    /// Get file metadata
    fn stat(&self, path: &str) -> Result<FileStat, FsError>;
    
    /// List directory contents
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry, 32>, FsError>;
    
    /// Create a directory
    fn mkdir(&mut self, path: &str) -> Result<(), FsError>;
    
    /// Remove a file
    fn remove(&mut self, path: &str) -> Result<(), FsError>;
    
    /// Remove a directory
    fn rmdir(&mut self, path: &str) -> Result<(), FsError>;
    
    /// Rename/move a file
    fn rename(&mut self, from: &str, to: &str) -> Result<(), FsError>;
    
    /// Sync all pending writes
    fn sync(&mut self) -> Result<(), FsError>;
    
    /// Get filesystem statistics
    fn statvfs(&self) -> Result<FsStats, FsError>;
}

/// Filesystem statistics
pub struct FsStats {
    /// Total size in bytes
    pub total_bytes: u64,
    /// Free bytes
    pub free_bytes: u64,
    /// Block size
    pub block_size: u32,
}
```

### 3.3 VFS Operations

```rust
impl Vfs {
    /// Initialize VFS with default mounts
    pub fn init() -> Self {
        let mut vfs = Self {
            mounts: [None; MAX_MOUNTS],
            files: Slab::new(),
        };
        
        // Mount ramfs at root
        vfs.mount("/", FsType::Ramfs(Ramfs::new()), false).unwrap();
        
        // Mount devfs at /dev
        vfs.mount("/dev", FsType::Devfs(Devfs::new()), false).unwrap();
        
        // Mount littlefs at /etc and /var (from flash)
        // Done later after flash driver init
        
        vfs
    }
    
    /// Mount a filesystem
    pub fn mount(&mut self, path: &str, fs: FsType, read_only: bool) -> Result<(), FsError> {
        let slot = self.mounts.iter_mut()
            .find(|m| m.is_none())
            .ok_or(FsError::TooManyMounts)?;
        
        *slot = Some(Mount {
            path: String::try_from(path).map_err(|_| FsError::PathTooLong)?,
            fs,
            read_only,
        });
        
        Ok(())
    }
    
    /// Resolve path to mount and relative path
    fn resolve(&self, path: &str) -> Result<(usize, &str), FsError> {
        // Normalize path
        let normalized = self.normalize_path(path)?;
        
        // Find longest matching mount
        let mut best_match: Option<(usize, usize)> = None; // (mount_idx, prefix_len)
        
        for (idx, mount) in self.mounts.iter().enumerate() {
            if let Some(m) = mount {
                if normalized.starts_with(m.path.as_str()) {
                    let prefix_len = m.path.len();
                    if best_match.map_or(true, |(_, l)| prefix_len > l) {
                        best_match = Some((idx, prefix_len));
                    }
                }
            }
        }
        
        let (mount_idx, prefix_len) = best_match.ok_or(FsError::NotFound)?;
        let relative = &normalized[prefix_len..];
        let relative = if relative.is_empty() { "/" } else { relative };
        
        Ok((mount_idx, relative))
    }
    
    /// Open a file (called from host function)
    pub fn open(
        &mut self,
        path: &str,
        flags: OpenFlags,
        caller: ModuleId,
        caps: &CapabilitySet,
    ) -> Result<Fd, FsError> {
        // Check capability
        self.check_capability(path, flags, caps)?;
        
        // Resolve mount
        let (mount_idx, relative) = self.resolve(path)?;
        let mount = self.mounts[mount_idx].as_mut().unwrap();
        
        // Check read-only
        if mount.read_only && flags.intersects(OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE) {
            return Err(FsError::ReadOnly);
        }
        
        // Open in filesystem
        let fs_handle = match &mut mount.fs {
            FsType::Ramfs(fs) => fs.open(relative, flags)?,
            FsType::Littlefs(fs) => fs.open(relative, flags)?,
            FsType::Devfs(fs) => fs.open(relative, flags)?,
        };
        
        // Allocate file descriptor
        let fd = self.files.insert(OpenFile {
            mount_idx: mount_idx as u8,
            fs_handle,
            offset: 0,
            flags,
            owner: caller,
        }).map_err(|_| FsError::TooManyOpenFiles)?;
        
        Ok(Fd(fd as u32))
    }
    
    /// Read from file
    pub fn read(&mut self, fd: Fd, buf: &mut [u8], caller: ModuleId) -> Result<usize, FsError> {
        let file = self.files.get_mut(fd.0 as usize).ok_or(FsError::BadFd)?;
        
        if file.owner != caller {
            return Err(FsError::PermissionDenied);
        }
        
        if !file.flags.contains(OpenFlags::READ) {
            return Err(FsError::PermissionDenied);
        }
        
        let mount = self.mounts[file.mount_idx as usize].as_mut().unwrap();
        let offset = file.offset;
        
        let bytes_read = match &mut mount.fs {
            FsType::Ramfs(fs) => fs.read(file.fs_handle, offset, buf)?,
            FsType::Littlefs(fs) => fs.read(file.fs_handle, offset, buf)?,
            FsType::Devfs(fs) => fs.read(file.fs_handle, offset, buf)?,
        };
        
        file.offset += bytes_read as u64;
        Ok(bytes_read)
    }
    
    /// Write to file
    pub fn write(&mut self, fd: Fd, data: &[u8], caller: ModuleId) -> Result<usize, FsError> {
        let file = self.files.get_mut(fd.0 as usize).ok_or(FsError::BadFd)?;
        
        if file.owner != caller {
            return Err(FsError::PermissionDenied);
        }
        
        if !file.flags.contains(OpenFlags::WRITE) {
            return Err(FsError::PermissionDenied);
        }
        
        let mount = self.mounts[file.mount_idx as usize].as_mut().unwrap();
        
        let offset = if file.flags.contains(OpenFlags::APPEND) {
            // Get file size for append
            u64::MAX // Filesystem handles this
        } else {
            file.offset
        };
        
        let bytes_written = match &mut mount.fs {
            FsType::Ramfs(fs) => fs.write(file.fs_handle, offset, data)?,
            FsType::Littlefs(fs) => fs.write(file.fs_handle, offset, data)?,
            FsType::Devfs(fs) => fs.write(file.fs_handle, offset, data)?,
        };
        
        file.offset += bytes_written as u64;
        Ok(bytes_written)
    }
    
    /// Close file
    pub fn close(&mut self, fd: Fd, caller: ModuleId) -> Result<(), FsError> {
        let file = self.files.get(fd.0 as usize).ok_or(FsError::BadFd)?;
        
        if file.owner != caller {
            return Err(FsError::PermissionDenied);
        }
        
        let mount_idx = file.mount_idx;
        let fs_handle = file.fs_handle;
        
        self.files.remove(fd.0 as usize);
        
        let mount = self.mounts[mount_idx as usize].as_mut().unwrap();
        match &mut mount.fs {
            FsType::Ramfs(fs) => fs.close(fs_handle),
            FsType::Littlefs(fs) => fs.close(fs_handle),
            FsType::Devfs(fs) => fs.close(fs_handle),
        }
    }
    
    /// Check file capability
    fn check_capability(&self, path: &str, flags: OpenFlags, caps: &CapabilitySet) -> Result<(), FsError> {
        for cap in caps.file_caps() {
            match cap {
                FileCapability::ReadPath(prefix) => {
                    if path.starts_with(prefix) && flags == OpenFlags::READ {
                        return Ok(());
                    }
                }
                FileCapability::WritePath(prefix) => {
                    if path.starts_with(prefix) && !flags.contains(OpenFlags::CREATE) {
                        return Ok(());
                    }
                }
                FileCapability::CreatePath(prefix) => {
                    if path.starts_with(prefix) {
                        return Ok(());
                    }
                }
                FileCapability::FullAccess => {
                    return Ok(());
                }
            }
        }
        
        Err(FsError::PermissionDenied)
    }
    
    /// Normalize path (resolve . and ..)
    fn normalize_path(&self, path: &str) -> Result<String<MAX_PATH>, FsError> {
        // Must be absolute
        if !path.starts_with('/') {
            return Err(FsError::InvalidPath);
        }
        
        let mut result = String::new();
        
        for component in path.split('/') {
            match component {
                "" | "." => continue,
                ".." => {
                    // Remove last component
                    if let Some(pos) = result.rfind('/') {
                        result.truncate(pos);
                    }
                }
                c => {
                    result.push('/').map_err(|_| FsError::PathTooLong)?;
                    result.push_str(c).map_err(|_| FsError::PathTooLong)?;
                }
            }
        }
        
        if result.is_empty() {
            result.push('/').unwrap();
        }
        
        Ok(result)
    }
}
```

### 3.4 Error Codes

```rust
#[derive(Debug, Clone, Copy)]
pub enum FsError {
    /// File or directory not found
    NotFound,
    /// Already exists
    AlreadyExists,
    /// Permission denied (capability check failed)
    PermissionDenied,
    /// Invalid file descriptor
    BadFd,
    /// Not a directory
    NotADirectory,
    /// Not a file
    NotAFile,
    /// Directory not empty
    DirectoryNotEmpty,
    /// Filesystem is full
    NoSpace,
    /// Path too long
    PathTooLong,
    /// Invalid path
    InvalidPath,
    /// Too many open files
    TooManyOpenFiles,
    /// Too many mounts
    TooManyMounts,
    /// Filesystem is read-only
    ReadOnly,
    /// I/O error
    IoError,
    /// Filesystem corrupt
    Corrupt,
}
```

---

## 4. File Capability System

### 4.1 Capability Types

```rust
/// File access capabilities
#[derive(Clone)]
pub enum FileCapability {
    /// Can read files under this path prefix
    ReadPath(String<MAX_PATH>),
    
    /// Can read and write files under this path prefix
    WritePath(String<MAX_PATH>),
    
    /// Can create, read, write, delete under this path prefix
    CreatePath(String<MAX_PATH>),
    
    /// Full access (for system modules only)
    FullAccess,
}

impl FileCapability {
    /// Parse from manifest string
    pub fn parse(s: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        match parts[0] {
            "read" => Ok(Self::ReadPath(parts[1].into())),
            "write" => Ok(Self::WritePath(parts[1].into())),
            "create" => Ok(Self::CreatePath(parts[1].into())),
            "full" => Ok(Self::FullAccess),
            _ => Err(ParseError::InvalidCapability),
        }
    }
}
```

### 4.2 Manifest Examples

```toml
# OTA Manager - needs to write downloaded modules
[capabilities]
required = [
    "fs:read:/etc/",
    "fs:create:/var/modules/",
    "fs:write:/var/ota/",
]

# Application - sandboxed to its own directory
[capabilities]
required = [
    "fs:read:/app/myapp/config/",
    "fs:create:/app/myapp/data/",
]

# Logger - append-only to log directory
[capabilities]
required = [
    "fs:write:/var/log/",
]
```

---

## 5. Filesystem Implementations

### 5.1 Ramfs (RAM Filesystem)

In-memory filesystem for `/` root and temporary files.

```rust
/// RAM filesystem
pub struct Ramfs {
    /// Inode table
    inodes: Slab<RamInode, 128>,
    /// Root inode
    root: u32,
    /// Open file handles
    handles: Slab<RamHandle, 32>,
}

struct RamInode {
    kind: FileKind,
    /// For files: inline data or pointer
    data: RamData,
    /// For directories: children
    children: Vec<(String<64>, u32), 16>,
    mtime: u64,
}

enum RamData {
    /// Small file stored inline
    Inline(Vec<u8, 256>),
    /// Larger file in allocated buffer
    Allocated { ptr: *mut u8, len: usize, cap: usize },
}

struct RamHandle {
    inode: u32,
    offset: u64,
}

impl Filesystem for Ramfs {
    fn open(&mut self, path: &str, flags: OpenFlags) -> Result<u32, FsError> {
        let inode = self.lookup(path)?;
        
        if flags.contains(OpenFlags::CREATE) && inode.is_err() {
            // Create new file
            let parent_path = parent_of(path);
            let name = basename(path);
            let parent = self.lookup(parent_path)?;
            
            let new_inode = self.inodes.insert(RamInode {
                kind: FileKind::File,
                data: RamData::Inline(Vec::new()),
                children: Vec::new(),
                mtime: now_ms(),
            })?;
            
            self.inodes[parent].children.push((name.into(), new_inode))?;
            
            let handle = self.handles.insert(RamHandle {
                inode: new_inode,
                offset: 0,
            })?;
            
            return Ok(handle as u32);
        }
        
        let inode = inode?;
        
        if flags.contains(OpenFlags::TRUNCATE) {
            self.inodes[inode].data = RamData::Inline(Vec::new());
        }
        
        let handle = self.handles.insert(RamHandle {
            inode,
            offset: 0,
        })?;
        
        Ok(handle as u32)
    }
    
    fn read(&mut self, handle: u32, offset: u64, buf: &mut [u8]) -> Result<usize, FsError> {
        let h = self.handles.get(handle as usize).ok_or(FsError::BadFd)?;
        let inode = &self.inodes[h.inode as usize];
        
        let data = match &inode.data {
            RamData::Inline(v) => v.as_slice(),
            RamData::Allocated { ptr, len, .. } => unsafe {
                core::slice::from_raw_parts(*ptr, *len)
            },
        };
        
        let start = offset as usize;
        if start >= data.len() {
            return Ok(0);
        }
        
        let available = data.len() - start;
        let to_read = buf.len().min(available);
        buf[..to_read].copy_from_slice(&data[start..start + to_read]);
        
        Ok(to_read)
    }
    
    fn write(&mut self, handle: u32, offset: u64, data: &[u8]) -> Result<usize, FsError> {
        // ... implementation
    }
    
    // ... other methods
}
```

### 5.2 LittleFS (Flash Filesystem)

Uses the `littlefs2` crate for flash storage.

```rust
use littlefs2::{
    fs::Filesystem as Lfs,
    driver::Storage,
    io::Result as LfsResult,
};

/// Flash storage driver
pub struct FlashStorage {
    /// Flash partition offset
    base: usize,
    /// Partition size
    size: usize,
}

impl Storage for FlashStorage {
    const READ_SIZE: usize = 1;
    const WRITE_SIZE: usize = 256;      // ESP32 flash page
    const BLOCK_SIZE: usize = 4096;     // ESP32 flash sector
    const BLOCK_COUNT: usize = 256;     // 1MB partition
    const BLOCK_CYCLES: i32 = 500;      // Wear leveling hint
    
    fn read(&self, off: usize, buf: &mut [u8]) -> LfsResult<usize> {
        hal::flash_read(self.base + off, buf)
            .map_err(|_| littlefs2::io::Error::Io)?;
        Ok(buf.len())
    }
    
    fn write(&mut self, off: usize, data: &[u8]) -> LfsResult<usize> {
        hal::flash_write(self.base + off, data)
            .map_err(|_| littlefs2::io::Error::Io)?;
        Ok(data.len())
    }
    
    fn erase(&mut self, off: usize, len: usize) -> LfsResult<usize> {
        hal::flash_erase(self.base + off, len)
            .map_err(|_| littlefs2::io::Error::Io)?;
        Ok(len)
    }
}

/// LittleFS wrapper
pub struct Littlefs {
    fs: Lfs<FlashStorage>,
    handles: Slab<LfsHandle, 16>,
}

struct LfsHandle {
    file: littlefs2::fs::File<FlashStorage>,
}

impl Filesystem for Littlefs {
    fn open(&mut self, path: &str, flags: OpenFlags) -> Result<u32, FsError> {
        let lfs_flags = convert_flags(flags);
        
        let file = self.fs.open(path, lfs_flags)
            .map_err(convert_error)?;
        
        let handle = self.handles.insert(LfsHandle { file })
            .map_err(|_| FsError::TooManyOpenFiles)?;
        
        Ok(handle as u32)
    }
    
    fn read(&mut self, handle: u32, offset: u64, buf: &mut [u8]) -> Result<usize, FsError> {
        let h = self.handles.get_mut(handle as usize)
            .ok_or(FsError::BadFd)?;
        
        h.file.seek(littlefs2::io::SeekFrom::Start(offset))
            .map_err(convert_error)?;
        
        h.file.read(buf).map_err(convert_error)
    }
    
    fn write(&mut self, handle: u32, offset: u64, data: &[u8]) -> Result<usize, FsError> {
        let h = self.handles.get_mut(handle as usize)
            .ok_or(FsError::BadFd)?;
        
        if offset != u64::MAX {
            h.file.seek(littlefs2::io::SeekFrom::Start(offset))
                .map_err(convert_error)?;
        }
        
        h.file.write(data).map_err(convert_error)
    }
    
    fn close(&mut self, handle: u32) -> Result<(), FsError> {
        let h = self.handles.remove(handle as usize)
            .ok_or(FsError::BadFd)?;
        
        // File dropped, automatically closed
        drop(h);
        Ok(())
    }
    
    fn sync(&mut self) -> Result<(), FsError> {
        self.fs.sync().map_err(convert_error)
    }
    
    fn statvfs(&self) -> Result<FsStats, FsError> {
        let info = self.fs.info().map_err(convert_error)?;
        
        Ok(FsStats {
            total_bytes: (FlashStorage::BLOCK_SIZE * FlashStorage::BLOCK_COUNT) as u64,
            free_bytes: (info.available_blocks * FlashStorage::BLOCK_SIZE) as u64,
            block_size: FlashStorage::BLOCK_SIZE as u32,
        })
    }
    
    // ... other methods
}
```

### 5.3 Devfs (Device Filesystem)

Virtual filesystem for device access.

```rust
/// Device filesystem
pub struct Devfs {
    devices: Vec<DeviceEntry, 32>,
}

struct DeviceEntry {
    name: String<32>,
    device: DeviceType,
}

enum DeviceType {
    Gpio(u8),           // /dev/gpio/N
    Uart(u8),           // /dev/uartN
    Spi(u8),            // /dev/spiN
    I2c(u8),            // /dev/i2cN
    Null,               // /dev/null
    Zero,               // /dev/zero
    Random,             // /dev/random
}

impl Filesystem for Devfs {
    fn open(&mut self, path: &str, _flags: OpenFlags) -> Result<u32, FsError> {
        // Parse path like "/gpio/5" or "/uart0"
        let device = self.find_device(path)?;
        
        // Return device type encoded as handle
        Ok(device as u32)
    }
    
    fn read(&mut self, handle: u32, _offset: u64, buf: &mut [u8]) -> Result<usize, FsError> {
        match DeviceType::from(handle) {
            DeviceType::Gpio(pin) => {
                let value = hal::gpio_read(pin)?;
                buf[0] = if value { b'1' } else { b'0' };
                Ok(1)
            }
            DeviceType::Uart(port) => {
                hal::uart_read(port, buf)
            }
            DeviceType::Null => {
                Ok(0)  // EOF
            }
            DeviceType::Zero => {
                buf.fill(0);
                Ok(buf.len())
            }
            DeviceType::Random => {
                hal::fill_random(buf);
                Ok(buf.len())
            }
            _ => Err(FsError::IoError),
        }
    }
    
    fn write(&mut self, handle: u32, _offset: u64, data: &[u8]) -> Result<usize, FsError> {
        match DeviceType::from(handle) {
            DeviceType::Gpio(pin) => {
                let value = data.get(0).map_or(false, |&b| b != b'0');
                hal::gpio_write(pin, value)?;
                Ok(1)
            }
            DeviceType::Uart(port) => {
                hal::uart_write(port, data)
            }
            DeviceType::Null => {
                Ok(data.len())  // Discard
            }
            _ => Err(FsError::IoError),
        }
    }
    
    fn stat(&self, path: &str) -> Result<FileStat, FsError> {
        if self.find_device(path).is_ok() || path == "/" {
            Ok(FileStat {
                size: 0,
                kind: if path == "/" { FileKind::Directory } else { FileKind::Device },
                mtime: 0,
            })
        } else {
            Err(FsError::NotFound)
        }
    }
    
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry, 32>, FsError> {
        if path != "/" {
            return Err(FsError::NotFound);
        }
        
        let mut entries = Vec::new();
        for dev in &self.devices {
            entries.push(DirEntry {
                name: dev.name.clone(),
                kind: FileKind::Device,
            }).ok();
        }
        Ok(entries)
    }
    
    // ... other methods (most return errors for devices)
}
```

---

## 6. Mount Table

### 6.1 Default Mounts

| Mount Point | Filesystem | Description |
|-------------|------------|-------------|
| `/` | ramfs | Root, minimal directories |
| `/dev` | devfs | Device files |
| `/sys` | sysfs | System information (future) |
| `/etc` | littlefs | Configuration (flash) |
| `/var` | littlefs | Variable data (flash) |

### 6.2 Directory Structure

```
/
├── dev/
│   ├── null
│   ├── zero
│   ├── random
│   ├── gpio/
│   │   ├── 0
│   │   ├── 1
│   │   └── ...
│   ├── uart0
│   ├── uart1
│   ├── spi0
│   └── i2c0
├── etc/
│   ├── boot.conf
│   ├── network.conf
│   └── modules/
│       └── *.toml          # Module manifests
├── var/
│   ├── modules/
│   │   └── *.wasm          # Downloaded modules
│   ├── log/
│   │   └── system.log
│   └── tmp/
└── sys/                     # Future: system info
    ├── memory
    ├── modules/
    └── network
```

---

## 7. Host Functions (WASM API)

### 7.1 Function Signatures

```rust
// File operations

/// Open a file
/// Returns: file descriptor (>= 0) or error (< 0)
fn sp_fs_open(path_ptr: u32, path_len: u32, flags: u32) -> i32;

/// Read from file
/// Returns: bytes read (>= 0) or error (< 0)
fn sp_fs_read(fd: u32, buf_ptr: u32, buf_len: u32) -> i32;

/// Write to file
/// Returns: bytes written (>= 0) or error (< 0)
fn sp_fs_write(fd: u32, data_ptr: u32, data_len: u32) -> i32;

/// Close file
/// Returns: 0 on success, error (< 0) on failure
fn sp_fs_close(fd: u32) -> i32;

/// Seek within file
/// whence: 0 = SET, 1 = CUR, 2 = END
/// Returns: new offset (>= 0) or error (< 0)
fn sp_fs_seek(fd: u32, offset_lo: u32, offset_hi: u32, whence: u32) -> i64;

/// Get file metadata
/// Writes FileStat to buf_ptr
/// Returns: 0 on success, error (< 0) on failure
fn sp_fs_stat(path_ptr: u32, path_len: u32, buf_ptr: u32) -> i32;

/// List directory
/// Writes array of DirEntry to buf_ptr
/// Returns: entry count (>= 0) or error (< 0)
fn sp_fs_readdir(path_ptr: u32, path_len: u32, buf_ptr: u32, buf_len: u32) -> i32;

/// Create directory
fn sp_fs_mkdir(path_ptr: u32, path_len: u32) -> i32;

/// Remove file
fn sp_fs_remove(path_ptr: u32, path_len: u32) -> i32;

/// Remove directory
fn sp_fs_rmdir(path_ptr: u32, path_len: u32) -> i32;

/// Rename file
fn sp_fs_rename(from_ptr: u32, from_len: u32, to_ptr: u32, to_len: u32) -> i32;

/// Sync filesystem
fn sp_fs_sync() -> i32;
```

### 7.2 WASM-side Wrapper (Rust)

```rust
// In module's support library

pub fn open(path: &str, flags: OpenFlags) -> Result<File, FsError> {
    let fd = unsafe {
        sp_fs_open(path.as_ptr() as u32, path.len() as u32, flags.bits())
    };
    
    if fd < 0 {
        Err(FsError::from(fd))
    } else {
        Ok(File { fd: fd as u32 })
    }
}

pub struct File {
    fd: u32,
}

impl File {
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        let result = unsafe {
            sp_fs_read(self.fd, buf.as_mut_ptr() as u32, buf.len() as u32)
        };
        
        if result < 0 {
            Err(FsError::from(result))
        } else {
            Ok(result as usize)
        }
    }
    
    pub fn write(&self, data: &[u8]) -> Result<usize, FsError> {
        let result = unsafe {
            sp_fs_write(self.fd, data.as_ptr() as u32, data.len() as u32)
        };
        
        if result < 0 {
            Err(FsError::from(result))
        } else {
            Ok(result as usize)
        }
    }
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe { sp_fs_close(self.fd) };
    }
}
```

---

## 8. Flash Partition Layout

### 8.1 ESP32-C6 (4MB Flash)

| Offset | Size | Name | Description |
|--------|------|------|-------------|
| 0x000000 | 64KB | bootloader | ESP32 bootloader |
| 0x010000 | 256KB | kernel | SovelmaOS kernel |
| 0x050000 | 1MB | modules | LittleFS: /var/modules |
| 0x150000 | 512KB | config | LittleFS: /etc |
| 0x1D0000 | 512KB | ota | OTA staging area |
| 0x250000 | 1.7MB | reserved | Future use |

### 8.2 Partition Table

```csv
# ESP-IDF partition table
# Name,   Type, SubType, Offset,  Size, Flags
bootloader, app,  factory, 0x0,     64K,
kernel,     app,  ota_0,   0x10000, 256K,
modules,    data, spiffs,  0x50000, 1M,
config,     data, spiffs,  0x150000,512K,
ota,        data, ota,     0x1D0000,512K,
```

---

## 9. Implementation Checklist

### Phase 1: Core VFS
- [ ] VFS structure and mount table
- [ ] Path normalization
- [ ] File descriptor management
- [ ] Capability checking

### Phase 2: Ramfs
- [ ] Inode management
- [ ] File read/write
- [ ] Directory operations

### Phase 3: LittleFS Integration
- [ ] Flash driver for ESP32
- [ ] LittleFS wrapper
- [ ] Mount at boot

### Phase 4: Devfs
- [ ] GPIO device
- [ ] UART device
- [ ] Null/zero/random

### Phase 5: WASM Integration
- [ ] Host functions
- [ ] WASM-side wrappers
- [ ] Testing

---

*Document Version: 0.1.0-draft*
*Last Updated: 2025-01-21*
