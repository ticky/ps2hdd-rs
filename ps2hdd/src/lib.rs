//! A Rust wrapper for `libps2hdd`, the library version of
//! [`pfsshell`](https://github.com/ps2homebrew/pfsshell), providing utilities
//! for reading and writing PlayStation®2 format hard disks and disk images.

use std::convert::TryInto;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

pub mod driver;
use crate::driver::{HDLFS, PFS};

pub mod fs;
use crate::fs::PartEntry;

pub mod partition_kind;
use crate::partition_kind::{FormattablePartitionKind, PartitionKind};

mod ffi_utils;
use ffi_utils::{ok_on_nonnegative_or_strerror, ok_on_zero_or_strerror};

// Only one device may be active at a time per process,
// so we keep track of it via this atomic boolean
static IS_DEVICE_ACTIVE: AtomicBool = AtomicBool::new(false);

static PFS_ZONE_SIZE: i32 = 8192;
static PFS_FRAGMENT: i32 = 0x0000_0000;

/// Represents a PlayStation®2-formatted hard disk device or disk image,
/// and permits APA partition, PFS file system, file and metadata reading
/// and writing.
///
/// This is a Rust interface for the colossal work put into the
/// [`pfsshell`](https://github.com/ps2homebrew/pfsshell) project, an ongoing
/// work by various contributors to the PlayStation®2 homebrew scene.
///
/// Much respect to Wizard of Oz, the originator of the project, uyjulian,
/// its current steward, and everyone who's helped out with it over the years.
#[derive(Debug)]
pub struct PS2HDD {
    path: PathBuf,
    pfs: Option<PFS>,
    hdlfs: Option<HDLFS>,
}

impl PS2HDD {
    /// Attempts to open a PS2 HDD.
    ///
    /// `path` may refer to a block device or a raw disk image file.
    ///
    /// Importantly, it is not currently possible to have more than one HDD
    /// open per process. Open HDDs will be automatically tracked by the PS2HDD
    /// subsystem and errors returned if it is not possible to open due to an
    /// open HDD existing already within the process.
    ///
    /// # Errors
    ///
    /// This function will return an error if a PS2 HDD is already open in
    /// this process, if `path` does not already exist or is not a file, if
    /// `path` is longer than 255 characters, or if there is any error
    /// initialising the subsystems which read and write the PS2 HDD.
    pub fn open<P: std::fmt::Debug + AsRef<Path>>(path: P) -> Result<Self, String> {
        if IS_DEVICE_ACTIVE.swap(true, std::sync::atomic::Ordering::Relaxed) {
            return Err("Only one PS2HDD instance may be mounted at a time".to_string());
        }

        // IMPORTANT: In every case that this function can return an Err or
        // panic, EXCEPT the initial check, it MUST reset IS_DEVICE_ACTIVE

        if !path.as_ref().is_file() {
            IS_DEVICE_ACTIVE.swap(false, std::sync::atomic::Ordering::Relaxed);
            return Err(format!("{}: No such file", path.as_ref().display()));
        }

        let path_str_result = path.as_ref().to_str();

        if path_str_result.is_none() {
            IS_DEVICE_ACTIVE.swap(false, std::sync::atomic::Ordering::Relaxed);
            return Err("Path could not be converted to a C String".to_string());
        }

        // TODO: Make this return an Err
        let path_str = path_str_result.expect("path_str is none but unwrapping failed?");

        let name_slice = std::ffi::CString::new(path_str)
            // TODO: Make this return an Err
            .expect("could not convert atad_device_path slice to C String");

        let length = name_slice.as_bytes().len();

        if length > 255 {
            IS_DEVICE_ACTIVE.swap(false, std::sync::atomic::Ordering::Relaxed);
            return Err(format!(
                "Path of length {} is too long to be null-terminated",
                length
            ));
        }

        name_slice
            .as_bytes()
            .iter()
            .enumerate()
            .for_each(|(index, byte)| unsafe {
                ps2hdd_sys::atad_device_path[index] = *byte as i8;
                if index == length - 1 {
                    ps2hdd_sys::atad_device_path[index + 1] = 0x00;
                }
            });

        let after_path = unsafe { std::ffi::CStr::from_ptr(ps2hdd_sys::atad_device_path.as_ptr()) }
            .to_str()
            // TODO: Make this return an Err
            .expect("could not convert the updated device path to a String");
        // TODO: Make this return an Err
        assert_eq!(
            after_path, path_str,
            "updating the device path variable didn't work, weird!"
        );

        // _init_apa can theoretically return nonzero in these cases:
        //
        // • invalid arguments
        //   (we don't control them, though)
        // • dev9RegisterShutdownCb failure
        //   (dev9 is stubbed and always succeeds here, though)
        // • apaGetTime failure
        //   (apaGetTime seemingly cannot ever fail?)
        // • ata_get_devinfo failure
        //   (ata_get_devinfo always returns at least one hdd device, though)
        // • apaAllocMem failure initialising file slot memory
        // • apaCacheInit failure initialising cache memory
        // • apaJournalRestore failure
        //   (not entirely sure what the failure cases are)
        // • AddDrv failure, due to too many (32+) devices,
        //   or init failure (though the APA driver's init cannot fail)
        //
        // So, while it seems exceptionally unlikely we can synthesise
        // conditions to make this particular case fail, at least this is easy
        if let Err(message) = ok_on_zero_or_strerror(
            unsafe { ps2hdd_sys::_init_apa(0, std::ptr::null_mut()) },
            "Unable to initialize APA partition driver",
        ) {
            // We run atad_close to ensure no file is open if this fails
            unsafe { ps2hdd_sys::atad_close() };
            IS_DEVICE_ACTIVE.swap(false, std::sync::atomic::Ordering::Relaxed);
            return Err(message);
        };

        // WARNING: _init_apa will just up and exit the program if
        // atad_device_path refers to a file that does not exist!
        // We attempt to work around this by ensuring the file exists
        // on our end, but this isn't guaranteed nor foolproof.
        // TODO: Fix that upstream.

        // _init_pfs can theoretically return nonzero in these cases:
        //
        // • invalid arguments
        //   (we don't control them, though)
        // • allocateMountBuffer failure initialising buffer memory
        // • pfsAllocMem failure initialising file slot memory
        // • pfsCacheInit failure initialising cache memory
        // • AddDrv failure, due to too many (32+) devices,
        //   or init failure (though the PFS driver's init cannot fail)
        //
        // So, once again, pretty unlikely we can synthesise that
        if let Err(message) = ok_on_zero_or_strerror(
            unsafe { ps2hdd_sys::_init_pfs(0, std::ptr::null_mut()) },
            "Unable to initialize PFS filesystem driver",
        ) {
            // We run atad_close to ensure no file is open if this fails
            unsafe { ps2hdd_sys::atad_close() };
            IS_DEVICE_ACTIVE.swap(false, std::sync::atomic::Ordering::Relaxed);
            return Err(message);
        };

        // TODO: _init_hdlfs

        Ok(PS2HDD {
            path: path.as_ref().to_path_buf(),
            pfs: None,
            hdlfs: None,
        })
    }

    /// Attempts to create and subsequently open a new PS2 HDD image file.
    ///
    /// # Errors
    ///
    /// This function will return an error if a PS2 HDD is already open in
    /// this process, if `path` already exists, if `path` is longer than 255
    /// characters, if the file could not be extended to the desired size, or
    /// if there is any error initialising the subsystems which read and write
    /// the PS2 HDD image.
    pub fn create<P: std::fmt::Debug + AsRef<Path>>(path: P, size: u64) -> Result<Self, String> {
        match std::fs::File::create(&path) {
            Err(error) => return Err(error.to_string()),
            Ok(file) => {
                if let Err(error) = file.set_len(size) {
                    return Err(error.to_string());
                }
                drop(file);
            }
        }

        Self::open(path)
    }

    /// Format the entire disk, creating the APA partition map, and
    /// default set of partitions `__mbr`, `__net`, `__system`, `__sysconf` and
    /// `__common`.
    ///
    /// Note that this is a destructive process, and data *will* be destroyed.
    ///
    /// # Errors
    ///
    /// This function will return an error if partitions could not be created.
    pub fn initialize(&self) -> Result<(), String> {
        let device = match std::ffi::CString::new("hdd0:") {
            Ok(device_path) => device_path,
            Err(error) => return Err(error.to_string()),
        };

        ok_on_zero_or_strerror(
            unsafe {
                ps2hdd_sys::iomanx_format(
                    device.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                )
            },
            "HDD formatting failed",
        )?;

        Ok(())
    }

    /// List all the partitions on the disk.
    ///
    /// Note that unlike `std::fs::read_dir` or the like, which return an
    /// iterator, all entries are fetched upfront, due to the underlying
    /// driver involving internal state we can't fully rely on.
    pub fn list_partitions(&self) -> Result<Vec<PartEntry>, String> {
        let path = std::ffi::CString::new("hdd0:").expect("couldn't convert string");
        let mut temp_dirent: ps2hdd_sys::iox_dirent_t = unsafe { std::mem::zeroed() };
        let mut dirents = Vec::new();

        let device_handle = ok_on_nonnegative_or_strerror(
            unsafe { ps2hdd_sys::iomanx_dopen(path.as_ptr()) },
            "Failed to list partitions",
        )?;

        while {
            let result = unsafe { ps2hdd_sys::iomanx_dread(device_handle, &mut temp_dirent) };

            if result < 0 {
                let name = unsafe { std::ffi::CStr::from_ptr(temp_dirent.name.as_ptr()) }
                    .to_str()
                    .expect("list_partitions: could not convert the partition name to a String");
                return Err(format!("Failed to list partitions: {} {}", result, name));
            }

            result > 0
        } {
            dirents.push(temp_dirent.try_into()?);
        }

        ok_on_zero_or_strerror(
            unsafe { ps2hdd_sys::iomanx_close(device_handle) },
            "Failed to close root device handle",
        )?;

        Ok(dirents)
    }

    /// Create a new, formatted partition within the APA partition map.
    ///
    /// Partitions can be formatted from here using any `PartitionKind` for
    /// which a suitable driver is available, except for [`MBR`]. The `MBR`
    /// partition is initialised as [`PFS`], and treated specially.
    ///
    /// The size is specified in mebibytes, and must be a power of two.
    /// Valid partition sizes are 128MiB, 256MiB, 512MiB, 1GiB, 2GiB, 4GiB,
    /// 8GiB, 16GiB, and 32GiB.
    ///
    /// The maximum partition size is determined by the number of sectors on
    /// the disk. For disks below roughly 6GiB in size, this means that
    /// creating any properly-sized partitions is impossible. For reference,
    /// the smallest hard drive released for the PlayStation®2 was 20GiB, so
    /// it's probably not unexpected that smaller sizes cause odd behaviour!
    ///
    /// [`MBR`]: enum.PartitionKind.html#variant.MBR
    /// [`PFS`]: enum.PartitionKind.html#variant.PFS
    ///
    /// # Errors
    ///
    /// This function will return an error if the specified partition size is
    /// invalid, the requested format type is invalid (`MBR`), or if the
    /// partition creation otherwise failed.
    pub fn create_partition(
        &self,
        partition_name: &str,
        kind: FormattablePartitionKind,
        size: u64,
    ) -> Result<(), String> {
        if !size.is_power_of_two() {
            return Err("Partition size must be a power of 2".to_string());
        }

        let partition_kind: PartitionKind = match kind {
            FormattablePartitionKind::MBR => FormattablePartitionKind::PFS,
            v => v,
        }
        .into();

        let size_str = match size {
            mb if mb >= 1024 => format!("{}G", mb / 1024),
            mb => format!("{}M", mb),
        };

        let mkpart_strpath = format!(
            "hdd0:{},,,{},{}",
            partition_name,
            size_str,
            partition_kind.as_apa_fs_type()
        );

        let mkpart_path = std::ffi::CString::new(mkpart_strpath).expect("couldn't convert string");
        let open_flags = ps2hdd_sys::IOMANX_O_RDWR as i32 | ps2hdd_sys::IOMANX_O_CREAT as i32;

        let partition_handle = ok_on_nonnegative_or_strerror(
            unsafe { ps2hdd_sys::iomanx_open(mkpart_path.as_ptr(), open_flags) },
            "Partition creation failed",
        )?;

        ok_on_zero_or_strerror(
            unsafe { ps2hdd_sys::iomanx_close(partition_handle) },
            "Failed to close partition handle",
        )?;

        self.format_partition(partition_name, kind)
    }

    /// Initialise a file system on a given partition.
    ///
    /// # Errors
    ///
    /// This function will return an error if the partition does not already
    /// exist, the partition name is invalid, or if the format process fails.
    pub fn format_partition(
        &self,
        partition_name: &str,
        kind: FormattablePartitionKind,
    ) -> Result<(), String> {
        let kind: PartitionKind = match kind {
            FormattablePartitionKind::MBR => FormattablePartitionKind::PFS,
            v => v,
        }
        .into();

        // TODO: How do we check encoding? What encoding is the target?
        // JIS X 0201? US ASCII?

        let device =
            match std::ffi::CString::new(format!("{}0:", kind.as_apa_fs_type().to_lowercase())) {
                Ok(device_path) => device_path,
                Err(error) => return Err(error.to_string()),
            };

        // TODO: Ensure path does not contain invalid characters?

        let partition = match std::ffi::CString::new(format!("hdd0:{}", partition_name)) {
            Ok(partition_path) => partition_path,
            Err(error) => return Err(error.to_string()),
        };

        let mut format_arg: [i32; 3] = [PFS_ZONE_SIZE, 0x2d66, PFS_FRAGMENT];

        ok_on_zero_or_strerror(
            unsafe {
                ps2hdd_sys::iomanx_format(
                    device.as_ptr(),
                    partition.as_ptr(),
                    format_arg.as_mut_ptr() as *mut core::ffi::c_void,
                    std::mem::size_of_val(&format_arg) as u64,
                )
            },
            "PFS partition formatting failed",
        )?;

        Ok(())
    }

    /// Acquire a file I/O object bound to the specified `pfs` partition.
    pub fn mount_pfs(&mut self, partition_name: &str) -> Result<&PFS, String> {
        if self.pfs.is_some() {
            return Err("A PFS partition is already mounted".to_string());
        }

        self.mount("pfs0:", partition_name)?;

        self.pfs = Some(PFS {
            partition_name: partition_name.to_string(),
        });

        self.pfs
            .as_ref()
            .ok_or("Failed to retrieve reference".to_string())
    }

    /// Unmount the currently-mounted PFS device.
    ///
    /// Should not generally be called by user code.
    /// Used internally to keep track of state.
    pub fn umount_pfs(&mut self) -> Result<(), String> {
        if self.pfs.is_none() {
            return Err("No PFS partition is mounted; nothing to unmount".to_string());
        }

        self.pfs = None;

        Ok(())
    }

    /// Acquire a file I/O object bound to the specified `hdlfs` partition.
    pub fn mount_hdlfs(&mut self, partition_name: &str) -> Result<&HDLFS, String> {
        if self.hdlfs.is_some() {
            return Err("A PFS partition is already mounted".to_string());
        }

        self.mount("hdl0:", partition_name)?;

        self.hdlfs = Some(HDLFS {
            partition_name: partition_name.to_string(),
        });

        self.hdlfs
            .as_ref()
            .ok_or("Failed to retrieve reference".to_string())
    }

    /// Unmount the currently-mounted HDLFS device.
    ///
    /// Should not generally be called by user code.
    /// Used internally to keep track of state.
    pub fn umount_hdlfs(&mut self) -> Result<(), String> {
        if self.hdlfs.is_none() {
            return Err("No HDLFS partition is mounted; nothing to unmount".to_string());
        }

        self.hdlfs = None;

        Ok(())
    }

    fn mount(&self, mount_point: &str, partition_name: &str) -> Result<(), String> {
        let mount_path = match std::ffi::CString::new(mount_point) {
            Ok(mount) => mount,
            Err(error) => return Err(error.to_string()),
        };

        // TODO: Ensure path does not contain invalid characters?

        let partition = match std::ffi::CString::new(format!("hdd0:{}", partition_name)) {
            Ok(partition_path) => partition_path,
            Err(error) => return Err(error.to_string()),
        };

        ok_on_zero_or_strerror(
            unsafe {
                ps2hdd_sys::iomanx_mount(
                    mount_path.as_ptr(),
                    partition.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    0,
                )
            },
            "Mounting failed",
        )?;

        Ok(())
    }
}

impl Drop for PS2HDD {
    fn drop(&mut self) {
        let was_active = IS_DEVICE_ACTIVE.swap(false, std::sync::atomic::Ordering::Relaxed);
        assert!(was_active, "PS2HDD dropped while not active");
        unsafe { ps2hdd_sys::atad_close() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // A disk image needs to be at least 6GB in size for APA to work
    // more is better, and 20GB is the "normal" minimum, but this is
    // just enough  to fit any of the minimum-size 128MB partitions in
    static DEMO_FILE_SIZE: u64 = 6 * 1024 * 1024 * 1024;

    #[test]
    #[serial(atad_device_path)]
    fn only_one_instance_allowed() {
        let demo_file_path = "hdd.img";

        let instance_1 = PS2HDD::create(demo_file_path, DEMO_FILE_SIZE);
        let instance_2 = PS2HDD::open(demo_file_path);
        let instance_3 = PS2HDD::open(demo_file_path);

        let instance = match instance_1 {
            Ok(instance) => instance,
            Err(message) => panic!(format!(
                "First construction should've been okay, instead got {:?}",
                message
            )),
        };
        assert_eq!(
            instance_2.unwrap_err(),
            "Only one PS2HDD instance may be mounted at a time",
            "Second construction didn't return an error"
        );
        assert_eq!(
            instance_3.unwrap_err(),
            "Only one PS2HDD instance may be mounted at a time",
            "Third construction didn't return an error"
        );

        drop(instance);

        let instance_4 = PS2HDD::open(demo_file_path);

        assert!(
            instance_4.is_ok(),
            "Fourth construction should've been okay"
        );

        std::fs::remove_file(demo_file_path).expect("could not delete demo file");
    }

    #[test]
    #[serial(atad_device_path)]
    fn initializes_disks_and_lists_partitions() {
        let demo_file_path = "hdd.img";

        let ps2hdd = match PS2HDD::create(demo_file_path, DEMO_FILE_SIZE) {
            Ok(ps2hdd) => ps2hdd,
            Err(message) => panic!(message),
        };

        match ps2hdd.initialize() {
            Ok(_) => (),
            Err(message) => panic!(message),
        }

        let partitions = match ps2hdd.list_partitions() {
            Ok(list) => list,
            Err(message) => panic!(message),
        };

        assert_eq!(
            partitions,
            vec![
                PartEntry {
                    name: "__mbr".to_string(),
                    kind: Some(PartitionKind::MBR),
                    size: 128 * 1024 * 1024
                },
                PartEntry {
                    name: "__net".to_string(),
                    kind: Some(PartitionKind::PFS),
                    size: 128 * 1024 * 1024
                },
                PartEntry {
                    name: "__system".to_string(),
                    kind: Some(PartitionKind::PFS),
                    size: 128 * 1024 * 1024
                },
                PartEntry {
                    name: "__sysconf".to_string(),
                    kind: Some(PartitionKind::PFS),
                    size: 128 * 1024 * 1024
                },
                PartEntry {
                    name: "__common".to_string(),
                    kind: Some(PartitionKind::PFS),
                    size: 128 * 1024 * 1024
                }
            ],
            "unexpected partition list"
        );

        std::fs::remove_file(demo_file_path).expect("could not delete demo file");
    }

    #[test]
    #[serial(atad_device_path)]
    fn initializes_disks_creates_and_formats_partitions() {
        let demo_file_path = "hdd.img";

        let ps2hdd = match PS2HDD::create(demo_file_path, DEMO_FILE_SIZE) {
            Ok(ps2hdd) => ps2hdd,
            Err(message) => panic!(message),
        };

        match ps2hdd.initialize() {
            Ok(_) => (),
            Err(message) => panic!(message),
        };

        match ps2hdd.create_partition("TESTPART", FormattablePartitionKind::PFS, 128) {
            Ok(_) => (),
            Err(message) => panic!(message),
        };

        let partitions = match ps2hdd.list_partitions() {
            Ok(list) => list,
            Err(message) => panic!(message),
        };

        assert_eq!(
            partitions,
            vec![
                PartEntry {
                    name: "__mbr".to_string(),
                    kind: Some(PartitionKind::MBR),
                    size: 128 * 1024 * 1024
                },
                PartEntry {
                    name: "__net".to_string(),
                    kind: Some(PartitionKind::PFS),
                    size: 128 * 1024 * 1024
                },
                PartEntry {
                    name: "__system".to_string(),
                    kind: Some(PartitionKind::PFS),
                    size: 128 * 1024 * 1024
                },
                PartEntry {
                    name: "__sysconf".to_string(),
                    kind: Some(PartitionKind::PFS),
                    size: 128 * 1024 * 1024
                },
                PartEntry {
                    name: "__common".to_string(),
                    kind: Some(PartitionKind::PFS),
                    size: 128 * 1024 * 1024
                },
                PartEntry {
                    name: "TESTPART".to_string(),
                    kind: Some(PartitionKind::PFS),
                    size: 128 * 1024 * 1024
                }
            ],
            "unexpected partition list"
        );

        std::fs::remove_file(demo_file_path).expect("could not delete demo file");
    }

    #[test]
    #[serial(atad_device_path)]
    fn can_mount_created_partition() {
        let demo_file_path = "hdd.img";

        let mut ps2hdd = match PS2HDD::create(demo_file_path, DEMO_FILE_SIZE) {
            Ok(ps2hdd) => ps2hdd,
            Err(message) => panic!(message),
        };

        match ps2hdd.initialize() {
            Ok(_) => (),
            Err(message) => panic!(message),
        };

        match ps2hdd.create_partition("TESTPART", FormattablePartitionKind::PFS, 128) {
            Ok(_) => (),
            Err(message) => panic!(message),
        };

        let pfs = match ps2hdd.mount_pfs("TESTPART") {
            Ok(pfs) => pfs,
            Err(message) => panic!(message),
        };

        assert_eq!(
            pfs.partition_name,"TESTPART",
            "Unexpected partition reference"
        );

        std::fs::remove_file(demo_file_path).expect("could not delete demo file");
    }

    #[test]
    #[serial(atad_device_path)]
    fn should_err_on_missing_file() {
        let nonexistent_file_path = "nonexistent.img";

        let err_message = match PS2HDD::open(nonexistent_file_path) {
            Ok(instance) => {
                drop(instance);
                panic!("First construction should've been okay, instead got an instance");
            }
            Err(message) => message,
        };

        assert_eq!(
            err_message, "nonexistent.img: No such file",
            "Construction without file didn't return an error"
        );
    }
}
