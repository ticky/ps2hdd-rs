//! PlayStation®2 file system driver-specific functionality

use std::io;
use std::path::Path;

use crate::ffi_utils::{ok_on_nonnegative_or_strerror, ok_on_zero_or_strerror};
use crate::fs::DirEntry;

fn create_dir_impl(device_root: &str, path: &Path) -> Result<(), String> {
    let path = match std::ffi::CString::new(format!("{}/{}", device_root, path.display())) {
        Ok(path) => path,
        Err(error) => return Err(format!("couldn't convert path: {}", error)),
    };

    ok_on_nonnegative_or_strerror(
        unsafe { ps2hdd_sys::iomanx_mkdir(path.as_ptr(), 0o777) },
        "failed to create directory",
    )?;

    Ok(())
}

fn create_dir_all_impl(device_root: &str, path: &Path) -> Result<(), String> {
    match create_dir_impl(device_root, path) {
        Ok(()) => return Ok(()),
        Err(ref e) if e == "failed to create directory: -2, No such file or directory" => {}
        Err(_) if path.is_dir() => return Ok(()),
        Err(e) => return Err(e),
    }

    match path.parent() {
        Some(p) => create_dir_all_impl(device_root, p)?,
        None => return Err("failed to create whole tree".to_string()),
    }

    match create_dir_impl(device_root, path) {
        Ok(()) => Ok(()),
        Err(_) if path.is_dir() => Ok(()),
        Err(e) => Err(e),
    }
}

pub trait Driver {
    /// Retrieves the root of the given device's file system
    fn get_device_root(&self) -> &str;

    /// Creates a new, empty directory at the provided path
    fn create_dir<P: std::fmt::Display + AsRef<Path>>(&self, path: P) -> Result<(), String> {
        create_dir_impl(self.get_device_root(), path.as_ref())
    }

    /// Recursively create a directory and all of its parent components if they
    /// are missing.
    fn create_dir_all<P: std::fmt::Display + AsRef<Path>>(&self, path: P) -> Result<(), String> {
        create_dir_all_impl(self.get_device_root(), path.as_ref())
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
        let cPath = match std::ffi::CString::new(format!("{}/{}", self.get_device_root(), path)) {
            Ok(cPath) => cPath,
            Err(error) => return Err(format!("couldn't convert path: {}", error)),
        };

        let mut temp_dirent: ps2hdd_sys::iox_dirent_t = unsafe { std::mem::zeroed() };
        let mut dirents = Vec::new();

        let directory_handle = ok_on_nonnegative_or_strerror(
            unsafe { ps2hdd_sys::iomanx_dopen(cPath.as_ptr()) },
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
            match unsafe { std::ffi::CStr::from_ptr(temp_dirent.name.as_ptr()) }.to_str() {
                Ok(name) => {
                    // Based on Rust's unix ReadDir implementation:
                    // https://github.com/rust-lang/rust/blob/19e1aac6ea9879c6d10eed7106b3bc883e5bf9a5/library/std/src/sys/unix/fs.rs#L488
                    if name != "." && name != ".." {
                        dirents.push(DirEntry::new(
                            temp_dirent.clone(),
                            path.as_ref().to_path_buf(),
                        ));
                    }
                }
                Err(error) => {
                    return Err(format!(
                        "could not convert the directory name to a String: {}",
                        error
                    ))
                }
            }
        }

        ok_on_zero_or_strerror(
            unsafe { ps2hdd_sys::iomanx_close(directory_handle) },
            "Failed to close directory handle",
        )?;

        Ok(dirents)
    }

    /// Removes an empty directory.
    fn remove_dir<P: std::fmt::Display + AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let path = match std::ffi::CString::new(format!("{}/{}", self.get_device_root(), path)) {
            Ok(path) => path,
            Err(error) => return Err(format!("couldn't convert path: {}", error)),
        };

        ok_on_nonnegative_or_strerror(
            unsafe { ps2hdd_sys::iomanx_rmdir(path.as_ptr()) },
            "failed to delete directory",
        )?;

        Ok(())
    }

    /// Removes a directory at this path, after removing all its contents. Use
    /// carefully!
    fn remove_dir_all<P: std::fmt::Display + AsRef<Path>>(&self, path: P) -> Result<(), String> {
        unimplemented!()
        // for child in self.list_dir(path)? {
        //     if child.file_type()?.is_dir() {
        //         self.remove_dir_all(&child.path())?;
        //     } else {
        //         self.remove_file(&child.path())?;
        //     }
        // }

        // self.remove_dir(path)
    }

    /// Removes a file from the filesystem.
    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partition_kind::FormattablePartitionKind;
    use crate::PS2HDD;
    use serial_test::serial;

    // A disk image needs to be at least 6GB in size for APA to work
    // more is better, and 20GB is the "normal" minimum, but this is
    // just enough  to fit any of the minimum-size 128MB partitions in
    static DEMO_FILE_SIZE: u64 = 6 * 1024 * 1024 * 1024;

    fn get_directory_entry_names(dirents: Vec<DirEntry>) -> Vec<String> {
        dirents
            .iter()
            .map(|dirent| {
                dirent
                    .file_name()
                    .to_str()
                    .expect("could not convert partition entry name")
                    .to_owned()
            })
            .collect()
    }

    #[test]
    #[serial(atad_device_path)]
    fn pfs_mount_create_and_read_dir() {
        let demo_file_path = "hdd.img";

        let mut ps2hdd = match PS2HDD::create(demo_file_path, DEMO_FILE_SIZE) {
            Ok(ps2hdd) => ps2hdd,
            Err(message) => panic!(message),
        };

        if let Err(message) = ps2hdd.initialize() {
            panic!(message);
        }

        if let Err(message) =
            ps2hdd.create_partition("TESTPART", FormattablePartitionKind::PFS, 128)
        {
            panic!(message);
        }

        let pfs = match ps2hdd.mount_pfs("TESTPART") {
            Ok(pfs) => pfs,
            Err(message) => panic!(message),
        };

        assert_eq!(
            pfs.partition_name, "TESTPART",
            "Unexpected partition reference"
        );

        pfs.create_dir("testdir").expect("Could not create testdir");

        let direntries = pfs.list_dir("/").expect("Could not list directory");

        assert_eq!(
            get_directory_entry_names(direntries),
            vec!["testdir"],
            "Unexpected directory list"
        );

        std::fs::remove_file(demo_file_path).expect("could not delete demo file");
    }

    #[test]
    #[serial(atad_device_path)]
    fn pfs_mount_create_dir_all() {
        let demo_file_path = "hdd.img";

        let mut ps2hdd = match PS2HDD::create(demo_file_path, DEMO_FILE_SIZE) {
            Ok(ps2hdd) => ps2hdd,
            Err(message) => panic!(message),
        };

        if let Err(message) = ps2hdd.initialize() {
            panic!(message);
        }

        if let Err(message) =
            ps2hdd.create_partition("TESTPART", FormattablePartitionKind::PFS, 128)
        {
            panic!(message);
        }

        let pfs = match ps2hdd.mount_pfs("TESTPART") {
            Ok(pfs) => pfs,
            Err(message) => panic!(message),
        };

        assert_eq!(
            pfs.partition_name, "TESTPART",
            "Unexpected partition reference"
        );

        pfs.create_dir_all("a/b/c/d")
            .expect("Could not create path");

        let direntries = pfs.list_dir("/").expect("Could not list directory");

        assert_eq!(
            get_directory_entry_names(direntries),
            vec!["a"],
            "Unexpected directory list"
        );

        let direntries = pfs.list_dir("/a").expect("Could not list directory");

        assert_eq!(
            get_directory_entry_names(direntries),
            vec!["b"],
            "Unexpected directory list"
        );

        let direntries = pfs.list_dir("/a/b").expect("Could not list directory");

        assert_eq!(
            get_directory_entry_names(direntries),
            vec!["c"],
            "Unexpected directory list"
        );

        let direntries = pfs.list_dir("/a/b/c").expect("Could not list directory");

        assert_eq!(
            get_directory_entry_names(direntries),
            vec!["d"],
            "Unexpected directory list"
        );

        std::fs::remove_file(demo_file_path).expect("could not delete demo file");
    }
}
