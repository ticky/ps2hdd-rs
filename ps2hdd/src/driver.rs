//! PlayStation®2 file system driver-specific functionality

pub trait Driver {}

#[derive(Debug)]
pub struct PFS {
    pub partition_name: String,
}

impl Driver for PFS {}

#[derive(Debug)]
pub struct HDLFS {
    pub partition_name: String,
}

impl Driver for HDLFS {}
