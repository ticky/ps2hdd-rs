extern crate bindgen;
extern crate meson;

use std::env;
use std::path::PathBuf;

fn main() {
    let build_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("build");
    let build_path = build_path.to_str().unwrap();

    // Tell cargo to tell rustc to link the ps2hdd shared library.
    println!("cargo:rustc-link-lib=ps2hdd");
    println!("cargo:rustc-link-search={}", build_path);

    meson::build("vendor/pfsshell", build_path);

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        .derive_debug(true)
        .derive_partialeq(true)
        .derive_eq(true)
        // Stuff from `hl.h`
        // NOTE some of these appear to be slightly-wonky
        // wrappers of underlying iomanX functionality
        // .whitelist_function("copyfrom")
        // .whitelist_function("copyto")
        // .whitelist_function("initialize")
        // .whitelist_function("list_dir_objects")
        // .whitelist_function("ls")
        // .whitelist_function("lspart")
        // .whitelist_function("mkfs")
        // .whitelist_function("mkpart")
        // Stuff from `iomanX_port.h`
        .whitelist_var("IOMANX_O_APPEND")
        .whitelist_var("IOMANX_O_CREAT")
        .whitelist_var("IOMANX_O_DIROPEN")
        .whitelist_var("IOMANX_O_EXCL")
        .whitelist_var("IOMANX_O_NBLOCK")
        .whitelist_var("IOMANX_O_NOWAIT")
        .whitelist_var("IOMANX_O_RDONLY")
        .whitelist_var("IOMANX_O_RDWR")
        .whitelist_var("IOMANX_O_TRUNC")
        .whitelist_var("IOMANX_O_WRONLY")
        .whitelist_var("IOMANX_SEEK_CUR")
        .whitelist_var("IOMANX_SEEK_END")
        .whitelist_var("IOMANX_SEEK_SET")
        .whitelist_var("FIO_CST_AT")
        .whitelist_var("FIO_CST_ATTR")
        .whitelist_var("FIO_CST_CT")
        .whitelist_var("FIO_CST_MODE")
        .whitelist_var("FIO_CST_MT")
        .whitelist_var("FIO_CST_PRVT")
        .whitelist_var("FIO_CST_SIZE")
        .whitelist_var("FIO_MT_RDONLY")
        .whitelist_var("FIO_MT_RDWR")
        .whitelist_var("FIO_S_IFDIR")
        .whitelist_var("FIO_S_IFLNK")
        .whitelist_var("FIO_S_IFMT")
        .whitelist_var("FIO_S_IFREG")
        .whitelist_var("FIO_S_IRGRP")
        .whitelist_var("FIO_S_IROTH")
        .whitelist_var("FIO_S_IRUSR")
        .whitelist_var("FIO_S_IRWXG")
        .whitelist_var("FIO_S_IRWXO")
        .whitelist_var("FIO_S_IRWXU")
        .whitelist_var("FIO_S_ISGID")
        .whitelist_var("FIO_S_ISUID")
        .whitelist_var("FIO_S_ISVTX")
        .whitelist_var("FIO_S_IWGRP")
        .whitelist_var("FIO_S_IWOTH")
        .whitelist_var("FIO_S_IWUSR")
        .whitelist_var("FIO_S_IXGRP")
        .whitelist_var("FIO_S_IXOTH")
        .whitelist_var("FIO_S_IXUSR")
        .whitelist_type("ps2fs_datetime_type")
        .whitelist_type("iox_stat_t")
        .whitelist_type("iox_dirent_t")
        .whitelist_function("_init_apa")
        .whitelist_function("_init_pfs")
        .whitelist_function("_init_hdlfs")
        .whitelist_function("iomanx_open")
        .whitelist_function("iomanx_close")
        .whitelist_function("iomanx_read")
        .whitelist_function("iomanx_write")
        .whitelist_function("iomanx_lseek")
        .whitelist_function("iomanx_ioctl")
        .whitelist_function("iomanx_remove")
        .whitelist_function("iomanx_mkdir")
        .whitelist_function("iomanx_rmdir")
        .whitelist_function("iomanx_dopen")
        .whitelist_function("iomanx_dclose")
        // NOTE: We do not include iomanx_dread,
        // as its public type declarations are a lie
        .whitelist_function("iomanx_getstat")
        .whitelist_function("iomanx_chstat")
        .whitelist_function("iomanx_format")
        .whitelist_function("iomanx_rename")
        .whitelist_function("iomanx_chdir")
        .whitelist_function("iomanx_sync")
        .whitelist_function("iomanx_mount")
        .whitelist_function("iomanx_umount")
        .whitelist_function("iomanx_lseek64")
        .whitelist_function("iomanx_devctl")
        .whitelist_function("iomanx_symlink")
        .whitelist_function("iomanx_readlink")
        .whitelist_function("iomanx_ioctl2")
        // Stuff from `fakeps2sdk/atad.c`
        .whitelist_var("hdd_length")
        .whitelist_var("atad_device_path")
        .whitelist_function("atad_close")
        // Stuff from `hdlfs/hdlfs.h`
        .whitelist_var("HDL_FS_MAGIC")
        .whitelist_var("APA_FLAG_SUB")
        .whitelist_var("HDL_INFO_MAGIC")
        .whitelist_var("HDL_GAME_DATA_OFFSET")
        .whitelist_type("hdl_game_info")
        .whitelist_type("part_specs_t")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
