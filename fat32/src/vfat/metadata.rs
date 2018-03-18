use std::fmt;

use traits;

/// A date as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Date(u16);

impl Date {
    pub fn empty() -> Date {
        Date(0)
    }
}

/// Time as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Time(u16);

impl Time {
    pub fn empty() -> Time {
        Time(0)
    }
}

/// File attributes as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Attributes(u8);

impl Attributes {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN: u8 = 0x02;
    pub const SYSTEM: u8 = 0x04;
    pub const VOLUME_ID: u8 = 0x08; 
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE: u8 = 0x20;
    pub const LFN: u8 = 0x0F;

    pub fn has_flag(&self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn equal_to(&self, flag: u8) -> bool {
        self.0 == flag
    }
}

/// A structure containing a date and time.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    pub date: Date,
    pub time: Time
}

impl Timestamp {
    pub fn empty() -> Timestamp {
        Timestamp {
            date: Date::empty(),
            time: Time::empty()
        }
    }
}

/// Metadata for a directory entry.
#[derive(Default, Debug, Clone)]
pub struct Metadata {
    pub is_read_only: bool,
    pub is_hidden: bool,
    pub created: Timestamp,
    pub last_accessed: Timestamp,
    pub last_modified: Timestamp
}

impl traits::Timestamp for Timestamp {
    fn year(&self) -> usize {
        (self.date.0 >> 9) as usize + 1980
    }

    fn month(&self) -> u8 {
        ((self.date.0 & 0b0000000111100000) >> 5) as u8
    }

    fn day(&self) -> u8 {
        (self.date.0 & 0b0000000000011111) as u8
    }

    fn hour(&self) -> u8 {
        (self.time.0 >> 11) as u8
    }

    fn minute(&self) -> u8 {
        ((self.time.0 & 0b0000011111100000) >> 5) as u8
    }

    fn second(&self) -> u8 {
        ((self.time.0 & 0b0000000000011111) << 1) as u8
    }
}

impl traits::Metadata for Metadata {
    type Timestamp = Timestamp;

    fn read_only(&self) -> bool {
        self.is_read_only
    }

    fn hidden(&self) -> bool {
        self.is_hidden
    }

    fn created(&self) -> Timestamp {
        self.created
    }

    fn accessed(&self) -> Timestamp {
        self.last_accessed
    }

    fn modified(&self) -> Timestamp {
        self.last_modified
    }
}

// FIXME: Implement `fmt::Display` (to your liking) for `Metadata`.
