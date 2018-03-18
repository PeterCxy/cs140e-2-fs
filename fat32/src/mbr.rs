use std::{fmt, io};

use traits::BlockDevice;
use util::*;

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct CHS {
    // CHS should be ignored by our implementation
    _head: u8,
    _sector_cylinder: u16
}

#[repr(C, packed)]
#[derive(Debug, Clone)]
pub struct PartitionEntry {
    bootable: u8, // 0x00: no, 0x80: yes
    _starting_chs: CHS, // unused, see relative_sector
    partition_type: u8, // FAT32: 0xB or 0xC
    _ending_chs: CHS, // unused, see relative_sector
    relative_sector: u32, // offset from the start of disk to the starting sector
    len: u32 // Total sectors in the partition
}

/// The master boot record (MBR).
#[repr(C, packed)]
#[derive(Clone)]
pub struct MasterBootRecord {
    _bootstrap: [u8; 436], // Bootstrap code, we don't need them here
    _disk_id: [u8; 10], // Disk ID, we don't need them here
    partitions: [PartitionEntry; 4],
    signature: u16 // Should be 0xAA55
}

#[derive(Debug)]
pub enum Error {
    /// There was an I/O error while reading the MBR.
    Io(io::Error),
    /// Partiion `.0` (0-indexed) contains an invalid or unknown boot indicator.
    UnknownBootIndicator(u8),
    /// The MBR magic signature was invalid.
    BadSignature,
}

impl MasterBootRecord {
    /// Reads and returns the master boot record (MBR) from `device`.
    ///
    /// # Errors
    ///
    /// Returns `BadSignature` if the MBR contains an invalid magic signature.
    /// Returns `UnknownBootIndicator(n)` if partition `n` contains an invalid
    /// boot indicator. Returns `Io(err)` if the I/O error `err` occured while
    /// reading the MBR.
    pub fn from<T: BlockDevice>(mut device: T) -> Result<MasterBootRecord, Error> {
        let record: MasterBootRecord = unsafe {
            device.read_sector_as::<MasterBootRecord>(0).map_err(|e| Error::Io(e))?
        };

        // Invalid signature
        if record.signature != 0xAA55 {
            return Err(Error::BadSignature);
        }

        // Ensure every partition is valid
        for (index, partition) in record.partitions.iter().enumerate() {
            if partition.bootable != 0x00 && partition.bootable != 0x80 {
                return Err(Error::UnknownBootIndicator(index as u8));
            }
        }
        return Ok(record);
    }
}

impl fmt::Debug for MasterBootRecord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{:#?}", self.partitions)
    }
}
