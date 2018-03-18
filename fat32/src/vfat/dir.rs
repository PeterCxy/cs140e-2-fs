use std::ffi::OsStr;
use std::str::from_utf8_unchecked;
use std::char::{decode_utf16, REPLACEMENT_CHARACTER};
use std::borrow::Cow;
use std::io;

use traits;
use util::VecExt;
use vfat::{VFat, Shared, File, Cluster, Entry};
use vfat::{Metadata, Attributes, Timestamp, Time, Date};

#[derive(Debug)]
pub struct Dir {
    drive: Shared<VFat>,
    cluster: Cluster,
    pub name: String,
    pub metadata: Metadata
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct VFatRegularDirEntry {
    name: [u8; 8],
    extension: [u8; 3],
    attribute: Attributes,
    _reserved: u8,
    _creation_time_tenth: u8, // Creation time in tenths of a second
    create_time: Time,
    create_date: Date,
    last_access_date: Date,
    first_cluster_high: u16, // High 16 bits of the first cluster
    last_modification_time: Time,
    last_modification_date: Date,
    first_cluster_low: u16, // Low 16 bits of the first cluster
    size: u32
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct VFatLfnDirEntry {
    seq_number: u8,
    name: [u16; 5], // can be terminated with 0x00 or 0xFF
    attribute: Attributes, // always 0x0F
    dir_type: u8, // always 0x00
    checksum: u8, // checksum of DOS file name
    name2: [u16; 6], // should be appended to the first, can be terminated with 0x00 or 0xFF
    _reserved: u16, // always 0x0000
    name3: [u16; 2], // should be appended to the second, same termination rule
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct VFatUnknownDirEntry {
    _unknown_1: [u8; 11],
    attribute: Attributes,
    _unknown_2: [u8; 20]
}

pub union VFatDirEntry {
    unknown: VFatUnknownDirEntry,
    regular: VFatRegularDirEntry,
    long_filename: VFatLfnDirEntry,
}

#[derive(Debug)]
pub enum VFatDirEntrySafe {
    Regular(VFatRegularDirEntry),
    Lfn(VFatLfnDirEntry),
    Deleted,
    End
}

unsafe fn parse_dir_entry(ent: &VFatDirEntry) -> VFatDirEntrySafe {
    if ent.unknown.attribute.equal_to(Attributes::LFN) {
        VFatDirEntrySafe::Regular(ent.regular.clone())
    } else if ent.unknown._unknown_1[0] == 0xE5 {
        VFatDirEntrySafe::Deleted
    } else if ent.unknown._unknown_1[0] == 0x00 {
        VFatDirEntrySafe::End
    } else {
        VFatDirEntrySafe::Lfn(ent.long_filename.clone())
    }
}

fn decode_file_name_utf8_ascii(name: &[u8]) -> String {
    unsafe {
        from_utf8_unchecked(name).to_string()
    }
}

fn decode_file_name_utf16(name: &[u16]) -> String {
    decode_utf16(name.iter().cloned())
        .map(|r| r.unwrap_or(REPLACEMENT_CHARACTER))
        .collect()
}

impl Dir {
    pub fn from_root_cluster(drive: Shared<VFat>, cluster: Cluster) -> Dir {
        Dir {
            drive,
            cluster,
            name: "".to_string(),
            metadata: Metadata {
                is_read_only: false,
                is_hidden: false,
                created: Timestamp::empty(),
                last_accessed: Timestamp::empty(),
                last_modified: Timestamp::empty()
            }
        }
    }
    /// Finds the entry named `name` in `self` and returns it. Comparison is
    /// case-insensitive.
    ///
    /// # Errors
    ///
    /// If no entry with name `name` exists in `self`, an error of `NotFound` is
    /// returned.
    ///
    /// If `name` contains invalid UTF-8 characters, an error of `InvalidInput`
    /// is returned.
    pub fn find<P: AsRef<OsStr>>(&self, name: P) -> io::Result<Entry> {
        use traits::{Dir, Entry};
        let name = name.as_ref().to_str().ok_or(io::Error::new(io::ErrorKind::InvalidInput, "Invalid file name"))?;
        for dir in self.entries()? {
            if dir.name().eq_ignore_ascii_case(name) {
                return Ok(dir);
            }
        }
        return Err(io::Error::new(io::ErrorKind::NotFound, "File not found"));
    }
}

pub struct DirIter {
    drive: Shared<VFat>,
    buf: Vec<u8>,
    pos: usize
}

impl DirIter {
    fn parse_regular_dir(&mut self, dir: VFatRegularDirEntry, prefix: &str) -> Entry {
        let mut name = format!("{}{}", prefix, decode_file_name_utf8_ascii(&dir.name));
        if dir.extension[0] != 0 {
            name = format!("{}.{}", name, decode_file_name_utf8_ascii(&dir.extension));
        }
        let cluster = Cluster::from(((dir.first_cluster_high as u32) << 16) + dir.first_cluster_low as u32);
        let metadata = Metadata {
            is_read_only: dir.attribute.has_flag(Attributes::READ_ONLY),
            is_hidden: dir.attribute.has_flag(Attributes::HIDDEN),
            created: Timestamp {
                date: dir.create_date,
                time: dir.create_time
            },
            last_accessed: Timestamp {
                date: dir.last_access_date,
                time: Time::empty()
            },
            last_modified: Timestamp {
                date: dir.last_modification_date,
                time: dir.last_modification_time
            }
        };
        if dir.attribute.has_flag(Attributes::DIRECTORY) {
            // Is a directory!
            Entry::Dir(Dir {
                drive: self.drive.clone(),
                cluster,
                name,
                metadata
            })
        } else {
            // Is a file!
            Entry::File(File {
                drive: self.drive.clone(),
                cluster,
                name,
                metadata,
                size: dir.size
            })
        }
    }
}

impl Iterator for DirIter {
    type Item = Entry;

    fn next(&mut self) -> Option<Entry> {
        if (self.pos + 32) >= self.buf.len() {
            // We must have exhausted the cluster chain
            return None;
        }

        let ent = unsafe {
            parse_dir_entry(&*(self.buf[(self.pos)..(self.pos + 32)].as_ptr() as *const VFatDirEntry))
        };
        self.pos += 32;

        match ent {
            VFatDirEntrySafe::Regular(regular) => Some(self.parse_regular_dir(regular, "")),
            VFatDirEntrySafe::Lfn(lfn) => unimplemented!(), // TODO: Implement LFN!!!
            VFatDirEntrySafe::End => None,
            VFatDirEntrySafe::Deleted => self.next()
        }
    }
}

impl traits::Dir for Dir {
    type Entry = Entry;
    type Iter = DirIter;

    fn entries(&self) -> io::Result<DirIter> {
        let mut buf: Vec<u8> = Vec::new();
        self.drive.borrow_mut().read_chain(self.cluster, &mut buf)?;
        Ok(DirIter {
            drive: self.drive.clone(),
            buf,
            pos: 0
        })
    }
}