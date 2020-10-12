//! PlayStation®2 file system driver-specific functionality

use std::convert::TryInto;
use std::io;
use std::path::Path;

use crate::ffi_utils::{ok_on_nonnegative_or_strerror, ok_on_zero_or_strerror};
use crate::fs::DirEntry;

pub trait Driver {
    /// Retrieves the root of the given device's file system
    fn get_device_root(&self) -> &str;

    /// Creates a new, empty directory at the provided path
    fn create_dir<P: std::fmt::Display + AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let path = match std::ffi::CString::new(format!("{}/{}", self.get_device_root(), path)) {
            Ok(path) => path,
            Err(error) => return Err(format!("couldn't convert path: {}", error)),
        };

        ok_on_nonnegative_or_strerror(
            unsafe { ps2hdd_sys::iomanx_mkdir(path.as_ptr(), 0o777) },
            "failed to create directory",
        )?;

        Ok(())
    }

    /// Recursively create a directory and all of its parent components if they
    /// are missing.
    fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        unimplemented!()
    }

    /// List the entries within a directory.
    ///
    /// Note that unlike `std::fs::read_dir` or the like, which return an
    /// iterator, all entries are fetched upfront, due to the underlying
    /// driver involving internal state we can't fully rely on.
    fn list_dir<P: std::fmt::Display + AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<Vec<DirEntry>, String> {
        let path = match std::ffi::CString::new(format!("{}/{}", self.get_device_root(), path)) {
            Ok(path) => path,
            Err(error) => return Err(format!("couldn't convert path: {}", error)),
        };

        let mut temp_dirent: ps2hdd_sys::iox_dirent_t = unsafe { std::mem::zeroed() };
        let mut dirents = Vec::new();

        let directory_handle = ok_on_nonnegative_or_strerror(
            unsafe { ps2hdd_sys::iomanx_dopen(path.as_ptr()) },
            "Failed to list directory",
        )?;

        while {
            let result = unsafe { ps2hdd_sys::iomanx_dread(directory_handle, &mut temp_dirent) };

            if result < 0 {
                match unsafe { std::ffi::CStr::from_ptr(temp_dirent.name.as_ptr()) }.to_str() {
                    Ok(name) => {
                        return Err(format!("Failed to list directories: {} {}", result, name))
                    }
                    Err(error) => {
                        return Err(format!(
                            "could not convert the directory name to a String: {}",
                            error
                        ))
                    }
                }
            }

            result > 0
        } {
            dirents.push(temp_dirent.try_into()?);
        }

        ok_on_zero_or_strerror(
            unsafe { ps2hdd_sys::iomanx_close(directory_handle) },
            "Failed to close directory handle",
        )?;

        Ok(dirents)
    }

    /// Removes an empty directory.
    fn remove_dir<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        unimplemented!()
    }

    /// Removes a directory at this path, after removing all its contents. Use
    /// carefully!
    fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        unimplemented!()
    }

    /// Removes a file from the filesystem.
    fn remove_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        unimplemented!()
    }

    /// Rename a file or directory to a new name, replacing the original file if
    /// `to` already exists.
    fn rename<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> io::Result<()> {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct PFS {
    pub partition_name: String,
}

impl Driver for PFS {
    fn get_device_root(&self) -> &str {
        "pfs0:"
    }
}

#[derive(Debug)]
pub struct HDLFS {
    pub partition_name: String,
}

impl Driver for HDLFS {
    fn get_device_root(&self) -> &str {
        "hdl0:"
    }
}
