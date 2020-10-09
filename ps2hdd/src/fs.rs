//! File system objects for APA partition mapped disks, intended to mimic the
//! `std::fs` library in as manu ways as possible

use std::convert::TryFrom;

use crate::partition_kind::PartitionKind;

/// Represents a directory entry present on a partition
#[derive(Debug, PartialEq)]
pub struct DirEntry {
    /// The name of the file or directory
    pub name: String,
}

impl TryFrom<ps2hdd_sys::iox_dirent_t> for DirEntry {
    type Error = String;

    fn try_from(value: ps2hdd_sys::iox_dirent_t) -> std::result::Result<Self, Self::Error> {
        let name = match unsafe { std::ffi::CStr::from_ptr(value.name.as_ptr()) }.to_str() {
            Ok(name) => name.to_owned(),
            Err(error) => return Err(error.to_string()),
        };

        Ok(Self { name })
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
