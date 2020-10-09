//! Structs representing the various possible partitions on APA partition
//! mapped disks

use std::convert::TryFrom;

/// Pretty way of representing the kind of APA partition we're talking about.
///
/// Can be turned into a [`FormattablePartitionKind`] by use of the `TryInto`
/// trait:
///
/// ```
/// use ps2hdd::partition_kind::{PartitionKind, FormattablePartitionKind};
/// use std::convert::TryInto;
///
/// let kind = PartitionKind::PFS;
/// let formattable_kind: FormattablePartitionKind = match kind.try_into() {
///     Err(message) => panic!(format!("unexpected error converting: {}", message)),
///     Ok(kind) => kind,
/// };
///
/// assert_eq!(formattable_kind, FormattablePartitionKind::PFS);
/// ```
///
/// Conversion will fail if is it not equivalent to any
/// `FormattablePartitionKind`:
///
/// ```
/// use ps2hdd::partition_kind::{PartitionKind, FormattablePartitionKind};
/// use std::convert::TryInto;
///
/// let kind = PartitionKind::EXT2;
/// let formattable_kind: Result<FormattablePartitionKind, String> = kind.try_into();
///
/// assert_eq!(formattable_kind, Err("Not a formattable partition kind".to_string()));
/// ```
///
/// [`FormattablePartitionKind`]: enum.FormattablePartitionKind.html
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PartitionKind {
    /// A "Master Boot Record" partition
    MBR = 0x0001,
    /// An EXT2-formatted swap partition, for use with PS2 Linux
    EXT2Swap = 0x0082,
    /// A general-use EXT2-formatted partition, for use with PS2 Linux
    EXT2 = 0x0083,
    /// A PFS partition, formatted for PlayStation®2-specific use
    PFS = 0x0100,
    CFS = 0x0101,
    /// An HDLoader partition, which technically means it follows ISO9660 rules
    HDL = 0x1337,
}

/// The subset of APA partition types in [`PartitionKind`] for which drivers
/// are available
///
/// Can be turned into a [`PartitionKind`] by use of the `Into` trait:
///
/// ```
/// use ps2hdd::partition_kind::{PartitionKind, FormattablePartitionKind};
/// use std::convert::Into;
///
/// let formattable_kind = FormattablePartitionKind::PFS;
/// let kind: PartitionKind = formattable_kind.into();
///
/// assert_eq!(kind, PartitionKind::PFS);
/// ```
///
/// [`PartitionKind`]: enum.PartitionKind.html
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FormattablePartitionKind {
    /// A "Master Boot Record" partition.
    ///
    /// Note that when formatting, this type is treated the same as
    /// [`PFS`](#variant.PFS).
    MBR = PartitionKind::MBR as isize,
    /// A PFS partition, formatted for PlayStation®2-specific use
    PFS = PartitionKind::PFS as isize,
    /// An HDLoader partition, which technically means it follows ISO9660 rules
    HDL = PartitionKind::HDL as isize,
}

impl PartitionKind {
    /// Returns a string representing the internal APA name for the filesystem.
    ///
    /// For use when communicating directly with functions of ps2hdd-sys.
    pub fn as_apa_fs_type(&self) -> &str {
        match self {
            Self::MBR => "MBR",
            Self::EXT2Swap => "EXT2SWAP",
            Self::EXT2 => "EXT2",
            Self::PFS => "PFS",
            Self::CFS => "CFS",
            Self::HDL => "HDL",
        }
    }
}

impl TryFrom<u32> for PartitionKind {
    type Error = String;

    fn try_from(number: u32) -> std::result::Result<Self, Self::Error> {
        match number {
            n if Self::MBR as u32 == n => Ok(Self::MBR),
            n if Self::EXT2Swap as u32 == n => Ok(Self::EXT2Swap),
            n if Self::EXT2 as u32 == n => Ok(Self::EXT2),
            n if Self::PFS as u32 == n => Ok(Self::PFS),
            n if Self::CFS as u32 == n => Ok(Self::CFS),
            n if Self::HDL as u32 == n => Ok(Self::HDL),
            _ => Err("Not a valid partition kind value".to_string()),
        }
    }
}

impl TryFrom<PartitionKind> for FormattablePartitionKind {
    type Error = String;

    fn try_from(kind: PartitionKind) -> std::result::Result<Self, Self::Error> {
        match kind {
            PartitionKind::MBR => Ok(Self::MBR),
            PartitionKind::PFS => Ok(Self::PFS),
            PartitionKind::HDL => Ok(Self::HDL),
            _ => Err("Not a formattable partition kind".to_string()),
        }
    }
}

impl From<FormattablePartitionKind> for PartitionKind {
    fn from(kind: FormattablePartitionKind) -> Self {
        match kind {
            FormattablePartitionKind::MBR => Self::MBR,
            FormattablePartitionKind::PFS => Self::PFS,
            FormattablePartitionKind::HDL => Self::HDL,
        }
    }
}
