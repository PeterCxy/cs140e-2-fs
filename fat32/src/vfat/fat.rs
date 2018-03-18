use std::fmt;
use vfat::*;

use self::Status::*;

#[derive(Debug, PartialEq)]
pub enum Status {
    /// The FAT entry corresponds to an unused (free) cluster.
    Free,
    /// The FAT entry/cluster is reserved.
    Reserved,
    /// The FAT entry corresponds to a valid data cluster. The next cluster in
    /// the chain is `Cluster`.
    Data(Cluster),
    /// The FAT entry corresponds to a bad (disk failed) cluster.
    Bad,
    /// The FAT entry corresponds to a valid data cluster. The corresponding
    /// cluster is the last in its chain.
    Eoc(u32)
}

#[repr(C, packed)]
pub struct FatEntry(pub u32);

impl FatEntry {
    /// Returns the `Status` of the FAT entry `self`.
    pub fn status(&self) -> Status {
        let entry_value = self.0 & 0x0FFFFFFF; // The first half of a byte is not used
        if entry_value == 0x00000000 {
            Status::Free
        } else if entry_value == 0x00000001 || (entry_value >= 0x0FFFFFF0 && entry_value <= 0x0FFFFFF6) {
            Status::Reserved
        } else if entry_value >= 0x00000002 && entry_value <= 0x0FFFFFEF {
            Status::Data(Cluster::from(entry_value))
        } else if entry_value == 0x0FFFFFF7 {
            Status::Bad
        } else {
            Status::Eoc(entry_value)
        }
    }
}

impl fmt::Debug for FatEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FatEntry")
            .field("value", &self.0)
            .field("status", &self.status())
            .finish()
    }
}
