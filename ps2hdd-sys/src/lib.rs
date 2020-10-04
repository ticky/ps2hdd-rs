#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

extern "C" {
    // The header for this is misleading about the
    // second argument's type so we fix it manually
    pub fn iomanx_dread(
        fd: std::os::raw::c_int,
        iox_dirent: *mut iox_dirent_t,
    ) -> std::os::raw::c_int;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn format_list_partitions_and_write() {
        unsafe {
            let demo_file_path = std::ffi::CStr::from_ptr(atad_device_path.as_ptr())
                .to_str()
                .expect("could not convert the default device path to a String");
            assert_eq!(
                demo_file_path, "hdd.img",
                "not using the default hdd.img file (are you doing multiple threads?)"
            );

            // Create hdd.img, and make it 6GB of zeroes
            // NOTE: This is way bigger than I'd hoped, but it's the minimum
            // required to fit any of the minimum-size partitions in.
            File::create(demo_file_path)
                .expect("couldn't create demo file")
                .set_len(6 * 1024 * 1024 * 1024)
                .expect("couldn't make demo file the right size");

            assert_eq!(_init_apa(0, std::ptr::null_mut()), 0, "_init_apa failed");

            assert_eq!(_init_pfs(0, std::ptr::null_mut()), 0, "_init_pfs failed");

            assert_eq!(
                _init_hdlfs(0, std::ptr::null_mut()),
                0,
                "_init_hdlfs failed"
            );

            let format_path = std::ffi::CString::new("hdd0:").expect("couldn't convert string");

            assert_eq!(
                iomanx_format(
                    format_path.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0
                ),
                0,
                "iomanx_format failed"
            );

            // Creating a partition named TESTPART

            let mkpart_path = std::ffi::CString::new("hdd0:TESTPART,,,128M,PFS")
                .expect("couldn't convert string");

            let fd = iomanx_open(
                mkpart_path.as_ptr(),
                IOMANX_O_RDWR as i32 | IOMANX_O_CREAT as i32,
            );

            if fd < 0 {
                let err = std::ffi::CStr::from_ptr(libc::strerror(-fd))
                    .to_str()
                    .expect("could not convert error message a String");
                panic!("iomanx_open failed: {}, {}", fd, err);
            }

            assert_eq!(iomanx_close(fd), 0, "iomanx_close failed");

            let testpart_path = std::ffi::CString::new("hdd0:TESTPART")
                .expect("couldn't convert mount path string");
            let pfs_path =
                std::ffi::CString::new("pfs0:").expect("couldn't convert pfs path string");

            // Formatting the TESTPART partition

            let PFS_ZONE_SIZE = 8192;
            let PFS_FRAGMENT = 0x0000_0000;
            let mut format_arg: [i32; 3] = [PFS_ZONE_SIZE, 0x2d66, PFS_FRAGMENT];

            let format_result = iomanx_format(
                pfs_path.as_ptr(),
                testpart_path.as_ptr(),
                format_arg.as_mut_ptr() as *mut core::ffi::c_void,
                std::mem::size_of::<[i32; 3]>() as u64,
            );

            if format_result < 0 {
                let err = std::ffi::CStr::from_ptr(libc::strerror(-format_result))
                    .to_str()
                    .expect("could not convert error message a String");
                panic!("iomanx_format failed: {}, {}", format_result, err);
            }

            let mount_result = iomanx_mount(
                pfs_path.as_ptr(),
                testpart_path.as_ptr(),
                0,
                std::ptr::null_mut(),
                0,
            );

            if mount_result < 0 {
                let err = std::ffi::CStr::from_ptr(libc::strerror(-mount_result))
                    .to_str()
                    .expect("could not convert error message a String");
                panic!("iomanx_mount failed: {}, {}", mount_result, err);
            }

            // Making a test directory

            let mkdir_path = std::ffi::CString::new("pfs0:/testdir")
                .expect("couldn't convert mkdir path string");

            let mkdir_result = iomanx_mkdir(mkdir_path.as_ptr(), 0o777);

            if mkdir_result < 0 {
                let err = std::ffi::CStr::from_ptr(libc::strerror(-mkdir_result))
                    .to_str()
                    .expect("could not convert error message a String");
                panic!("iomanx_mkdir failed: {}, {}", mkdir_result, err);
            }

            // Opening the root directory so we can list it

            let ls_path =
                std::ffi::CString::new("pfs0:/").expect("couldn't convert ls path string");

            let root_dh = iomanx_dopen(ls_path.as_ptr());

            if root_dh < 0 {
                let err = std::ffi::CStr::from_ptr(libc::strerror(-root_dh))
                    .to_str()
                    .expect("could not convert error message a String");
                panic!("iomanx_dopen failed: {}, {}", root_dh, err);
            }

            // Grab all of the directory entries and put them in a vec

            let mut dirent: iox_dirent_t = std::mem::zeroed();
            let mut dirents = Vec::new();
            while {
                let result = iomanx_dread(root_dh, &mut dirent);

                // aaaaAAAAaaa this took me SO long to work out;
                // - less than zero means there was an error,
                // - zero means there are no more directory entries
                // - greater than zero means a file handle number
                if result < 0 {
                    let name = std::ffi::CStr::from_ptr(dirent.name.as_ptr())
                        .to_str()
                        .expect(
                            "looping iomanx_dread, could not convert the file name to a String",
                        );
                    println!("looping iomanx_dread, result: {} {}", result, name);
                }

                result > 0
            } {
                dirents.push(dirent);
            }

            // Check the directory listing

            assert_eq!(dirents.len(), 3, "unexpected number of directory entries");

            assert_eq!(
                std::ffi::CStr::from_ptr(dirents[0].name.as_ptr())
                    .to_str()
                    .expect("could not convert directory entry name 0"),
                "."
            );
            assert_eq!(
                std::ffi::CStr::from_ptr(dirents[1].name.as_ptr())
                    .to_str()
                    .expect("could not convert directory entry name 1"),
                ".."
            );
            assert_eq!(
                std::ffi::CStr::from_ptr(dirents[2].name.as_ptr())
                    .to_str()
                    .expect("could not convert directory entry name 2"),
                "testdir"
            );
            assert_eq!(
                dirents[2].stat.mode & FIO_S_IFMT,
                FIO_S_IFDIR,
                "expected item at index 2 to be a directory"
            );

            iomanx_close(root_dh);

            assert_eq!(iomanx_umount(pfs_path.as_ptr()), 0, "iomanx_umount failed");

            std::fs::remove_file(demo_file_path).expect("could not delete demo file");
        }
    }
}
