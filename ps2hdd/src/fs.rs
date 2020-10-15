//! File system objects for APA partition mapped disks, intended to mimic the
//! `std::fs` library in as manu ways as possible

use std::convert::TryFrom;

use crate::partition_kind::PartitionKind;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct FileType {
    pub mode: std::os::raw::c_uint,
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.is(ps2hdd_sys::FIO_S_IFDIR)
    }

    pub fn is_file(&self) -> bool {
        self.is(ps2hdd_sys::FIO_S_IFREG)
    }

    pub fn is_symlink(&self) -> bool {
        self.is(ps2hdd_sys::FIO_S_IFLNK)
    }

    pub fn is(&self, mode: u32) -> bool {
        self.mode & ps2hdd_sys::FIO_S_IFMT == mode
    }
}

/// Represents a directory entry present on a partition
#[derive(Debug, PartialEq)]
pub struct DirEntry {
    entry: ps2hdd_sys::iox_dirent_t,
    root: std::path::PathBuf,
}

impl DirEntry {
    pub fn new(entry: ps2hdd_sys::iox_dirent_t, root: std::path::PathBuf) -> Self {
        Self { entry, root }
    }

    pub fn path(&self) -> &std::path::PathBuf {
        unimplemented!()
    }

    pub fn file_name(&self) -> std::ffi::OsString {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::OsStr::from_bytes(self.name_bytes()).to_os_string()
    }

    pub fn file_type(&self) -> Result<FileType, String> {
        Ok(FileType { mode: self.entry.stat.mode })
    }

    fn name_bytes(&self) -> &[u8] {
        unsafe { std::ffi::CStr::from_ptr(self.entry.name.as_ptr()).to_bytes() }
    }
}

/// Represents a partition present on the disk
#[derive(Debug, PartialEq)]
pub struct PartEntry {
    /// The partition's name
    pub name: String,
    pub kind: Option<PartitionKind>,
    /// The size of the partition, in bytes
    pub size: u64,
}

impl TryFrom<ps2hdd_sys::iox_dirent_t> for PartEntry {
    type Error = String;

    fn try_from(dirent: ps2hdd_sys::iox_dirent_t) -> std::result::Result<Self, Self::Error> {
        let name = match unsafe { std::ffi::CStr::from_ptr(dirent.name.as_ptr()) }.to_str() {
            Ok(name) => name.to_owned(),
            Err(error) => return Err(error.to_string()),
        };

        let kind = match dirent.stat.mode {
            0x0000 => None,
            mode => Some(PartitionKind::try_from(mode)?),
        };

        Ok(Self {
            name,
            kind,
            // stat size is in sectors, we want it in bytes
            // notably, the sector size can be different per disk,
            // but it's unclear whether the PS2 respects this
            size: (dirent.stat.size as u64) * 512,
        })
    }
}
