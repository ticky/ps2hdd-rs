#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

//! Bindings for `libps2hdd`
//! (the library version of [`pfsshell`](https://github.com/ps2homebrew/pfsshell)),
//! primarily generated using [`bindgen`](https://crates.io/crates/bindgen)

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

extern "C" {
    // The header for this is misleading about the
    // second argument's type so we fix it manually
    pub fn iomanx_dread(
        fd: std::os::raw::c_int,
        iox_dirent: *mut iox_dirent_t,
    ) -> std::os::raw::c_int;
}

impl std::fmt::Debug for iox_dirent_t {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = unsafe { std::ffi::CStr::from_ptr(self.name.as_ptr()) };
        formatter.debug_struct("iox_dirent_t")
            .field("name", &format_args!("{:?}", name))
            .field("stat", &self.stat)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    // Fun problems; we need to definitely absolutely not run these in parallel
    // which makes sense, I was figuring I'd need to do mutexing later, so,
    // now I know for sure. lmao.
    use serial_test::serial;

    /// Utility function which sets the `atad_device_path` variable
    /// to the content of a given Rust str value.
    ///
    /// Handles conversion to a CString, and null-termination.
    ///
    /// Needed because the underlying type of bytes of a CString and
    /// those of a c_char are, sadly, different (u8 vs i8, damn it C)
    fn util_set_atad_device_path(path: &str) {
        let name_slice = std::ffi::CString::new(path)
            .expect("could not convert atad_device_path slice to C String");

        let length = name_slice.as_bytes().len();

        if length > 255 {
            panic!(
                "util_set_atad_device_path: string length {} is too long to be null-terminated",
                length
            )
        }

        name_slice
            .as_bytes()
            .iter()
            .enumerate()
            .for_each(|(index, byte)| unsafe {
                atad_device_path[index] = *byte as i8;
                if index == length - 1 {
                    atad_device_path[index + 1] = 0x00;
                }
            });

        unsafe {
            let after_path = std::ffi::CStr::from_ptr(atad_device_path.as_ptr())
                .to_str()
                .expect("could not convert the updated device path to a String");
            assert_eq!(
                after_path, path,
                "util_set_atad_device_path: updating the variable didn't work, weird!"
            );
        }
    }

    /// Utility function which, when passed a CString path,
    /// reads the list of the items within that directory.
    fn util_iomanx_dopen_dread(path: &std::ffi::CString) -> Vec<iox_dirent_t> {
        let mut temp_dirent: iox_dirent_t = unsafe { std::mem::zeroed() };

        let mut dirents = Vec::new();

        let device_handle = unsafe { iomanx_dopen(path.as_ptr()) };

        if device_handle < 0 {
            let err = unsafe { std::ffi::CStr::from_ptr(libc::strerror(-device_handle)) }
                .to_str()
                .expect("util_iomanx_dopen_dread: could not convert error message a String");
            panic!(
                "util_iomanx_dopen_dread: dopen failed: {}, {}",
                device_handle, err
            );
        }

        while {
            let result = unsafe { iomanx_dread(device_handle, &mut temp_dirent) };

            // aaaaAAAAaaa this took me SO long to work out;
            // - less than zero means there was an error,
            // - zero means there are no more directory entries
            // - greater than zero means a file handle number
            if result < 0 {
                let name = unsafe { std::ffi::CStr::from_ptr(temp_dirent.name.as_ptr()) }
                    .to_str()
                    .expect(
                        "util_iomanx_dopen_dread: could not convert the partition name to a String",
                    );
                println!("util_iomanx_dopen_dread: result: {} {}", result, name);
            }

            result > 0
        } {
            dirents.push(temp_dirent);
        }

        unsafe {
            iomanx_close(device_handle);
        }

        dirents
    }

    // A disk image needs to be at least 6GB in size for APA to work
    // more is better, and 20GB is the "normal" minimum, but this is
    // just enough  to fit any of the minimum-size 128MB partitions in
    static DEMO_FILE_SIZE: u64 = 6 * 1024 * 1024 * 1024;
    static DEFAULT_FILE_PATH: &str = "hdd.img";

    #[test]
    #[serial(atad_device_path)]
    fn format_list_partitions_and_write() {
        unsafe {
            // NOTE: Because the str would be pointing at the same data as the
            // C pointer, later writes would cause problems if we don't turn
            // this into a real String.
            let demo_file_path = std::ffi::CStr::from_ptr(atad_device_path.as_ptr())
                .to_str()
                .expect("could not convert the default device path to a String")
                .to_owned();
            assert_eq!(
                demo_file_path, DEFAULT_FILE_PATH,
                "not using the default hdd.img file (are you doing multiple threads?)"
            );

            File::create(&demo_file_path)
                .expect("couldn't create demo file")
                .set_len(DEMO_FILE_SIZE)
                .expect("couldn't make demo file the right size");

            // NOTE: The `_init_*` methods initialise each driver, adding it to
            // the pool of "drives" available to iomanX, with each one stored
            // in `pfsshell/subprojects/iomanX/iomanX.c`'s `dev_list` variable.

            // `_init_apa` internally opens the file at
            // `atad_device_path`, and provides the "hdd0" device,
            // which is implicitly mounted once this has been called.
            //
            // The `hdd0` device allows listing and manipulating partitions.
            assert_eq!(_init_apa(0, std::ptr::null_mut()), 0, "_init_apa failed");

            // `_init_pfs` provides the "pfs0" device,
            // which is not automatically mounted.
            //
            // The `pfs0` device allows access to a PFS file system
            // of a given partition within the `hdd0` structure.
            assert_eq!(_init_pfs(0, std::ptr::null_mut()), 0, "_init_pfs failed");

            // TODO: Turns out this isn't necessary for anything pfsshell can
            // do, (no code path mounts to hdl0?) but perhaps it's still worth
            // keeping? Seems like HDLFS could possibly be browsed by this.

            // `_init_hdlfs` provides the `hdl0` device,
            // which is not automatically mounted.
            //
            // The `hdl0` device allows access to an HDLFS file system
            // of a given partition within the `hdd0` structure.
            // assert_eq!(
            //     _init_hdlfs(0, std::ptr::null_mut()),
            //     0,
            //     "_init_hdlfs failed"
            // );

            let hdd0_path = std::ffi::CString::new("hdd0:").expect("couldn't convert string");

            assert_eq!(
                iomanx_format(
                    hdd0_path.as_ptr(),
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

            // Check the partition listing

            let partitions: Vec<_> = util_iomanx_dopen_dread(&hdd0_path)
                .iter()
                .map(|dirent| {
                    std::ffi::CStr::from_ptr(dirent.name.as_ptr())
                        .to_str()
                        .expect("could not convert partition entry name")
                        .to_owned()
                })
                .collect();

            assert_eq!(
                partitions,
                vec![
                    "__mbr",
                    "__net",
                    "__system",
                    "__sysconf",
                    "__common",
                    "TESTPART"
                ],
                "unexpected partition list"
            );

            let testpart_path = std::ffi::CString::new("hdd0:TESTPART")
                .expect("couldn't convert mount path string");
            let pfs0_root_path =
                std::ffi::CString::new("pfs0:").expect("couldn't convert pfs path string");

            // Formatting the TESTPART partition

            let PFS_ZONE_SIZE = 8192;
            let PFS_FRAGMENT = 0x0000_0000;
            let mut format_arg: [i32; 3] = [PFS_ZONE_SIZE, 0x2d66, PFS_FRAGMENT];

            let format_result = iomanx_format(
                pfs0_root_path.as_ptr(),
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
                pfs0_root_path.as_ptr(),
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

            let pfs0_testdir_path = std::ffi::CString::new("pfs0:/testdir")
                .expect("couldn't convert mkdir path string");

            let mkdir_result = iomanx_mkdir(pfs0_testdir_path.as_ptr(), 0o777);

            if mkdir_result < 0 {
                let err = std::ffi::CStr::from_ptr(libc::strerror(-mkdir_result))
                    .to_str()
                    .expect("could not convert error message a String");
                panic!("iomanx_mkdir failed: {}, {}", mkdir_result, err);
            }

            // Opening the root directory so we can list it

            let pfs0_ls_path =
                std::ffi::CString::new("pfs0:/").expect("couldn't convert ls path string");

            // Grab all of the directory entries and put them in a vec

            let pfs0_dirents = util_iomanx_dopen_dread(&pfs0_ls_path);

            // Check the directory listing

            let pfs0_dirnames: Vec<_> = pfs0_dirents
                .iter()
                .map(|dirent| {
                    std::ffi::CStr::from_ptr(dirent.name.as_ptr())
                        .to_str()
                        .expect("could not convert directory entry name")
                        .to_owned()
                })
                .collect();

            assert_eq!(
                pfs0_dirnames,
                vec![".", "..", "testdir"],
                "unexpected directory list"
            );

            assert_eq!(
                pfs0_dirents[2].stat.mode & FIO_S_IFMT,
                FIO_S_IFDIR,
                "expected item at index 2 to be a directory"
            );

            // This cleans up ATAD's real file pointer, but nothing else;
            // technically between these phases the rest of the infrastructure
            // is still prepared to run. Alas.
            atad_close();

            assert_eq!(
                iomanx_umount(pfs0_root_path.as_ptr()),
                0,
                "iomanx_umount failed"
            );

            let demo_1_path = "hdd1.img";

            File::create(demo_1_path)
                .expect("couldn't create 1st demo file")
                .set_len(DEMO_FILE_SIZE)
                .expect("couldn't make 1st demo file the right size");

            util_set_atad_device_path(demo_1_path);

            assert_eq!(
                _init_apa(0, std::ptr::null_mut()),
                0,
                "second _init_apa failed"
            );

            assert_eq!(
                iomanx_format(
                    hdd0_path.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0
                ),
                0,
                "second iomanx_format failed"
            );

            // Check the partition listing

            let partitions: Vec<_> = util_iomanx_dopen_dread(&hdd0_path)
                .iter()
                .map(|dirent| {
                    std::ffi::CStr::from_ptr(dirent.name.as_ptr())
                        .to_str()
                        .expect("could not convert partition entry name")
                        .to_owned()
                })
                .collect();

            assert_eq!(
                partitions,
                vec![
                    "__mbr",
                    "__net",
                    "__system",
                    "__sysconf",
                    "__common"
                ],
                "unexpected partition list"
            );

            std::fs::remove_file(demo_file_path).expect("could not delete demo file");
            std::fs::remove_file(demo_1_path).expect("could not delete demo 1 file");

            // Always re-set this after each test
            util_set_atad_device_path(DEFAULT_FILE_PATH);
        }
    }
}
