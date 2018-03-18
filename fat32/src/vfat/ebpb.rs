use std::fmt;

use traits::BlockDevice;
use util::*;
use vfat::Error;

#[repr(C, packed)]
pub struct BiosParameterBlock {
    bootstrap: [u8; 3], // Should be EB XX 90 (JMP SHORT XX 90)
    _oem_id: [u8; 8], // OEM Identifier
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fat_num: u8, // Number of File Allocation Tables
    _max_directory_entries: u16, // Should always be 0 for FAT32
    logical_sectors_2: u16, // Total logical sectors (in 2 bytes, if 0, use logical_sectors_4)
    _fat_id: u8, // media descriptor type
    sector_per_fat_2: u16, // if 0, use sector_per_fat_4
    _sector_per_track: u16,
    _heads: u16,
    hidden_sectors: u32, // Number of hidden sectors
    logical_sectors_4: u32,
    sector_per_fat_4: u32,
    _flags: u16,
    _fat_ver: u16, // The high byte is the major version and the low byte is the minor version.
    root_cluster: u32, // The cluster number of the root directory. Often this field is set to 2.
    _fsinfo_sector: u16, // The sector number of the FSInfo structure.
    _backup_boot_sector: u16, // The sector number of the backup boot sector.
    _reserved: [u8; 12], // Reserved. When the volume is formated these bytes should be zero.
    _drive_number: u8, // 0x00 for a floppy disk and 0x80 for hard disks.
    _reserved_nt: u8, // Flags in Windows NT. Reserved otherwise.
    _signature: u8, // Signature (should be 0x28 or 0x29).
    _volume_id: u32, // Volume serial number for tracking. Ignored.
    volume_label_string: [u8; 11], // Volume label string padded with spaces
    _system_identifier_string: [u8; 8], // Always "FAT32  "
    _boot_code: [u8; 420],
    bootable_signature: u16 // 0xAA55 if bootable
}

impl BiosParameterBlock {
    /// Reads the FAT32 extended BIOS parameter block from sector `sector` of
    /// device `device`.
    ///
    /// # Errors
    ///
    /// If the EBPB signature is invalid, returns an error of `BadSignature`.
    pub fn from<T: BlockDevice>(
        mut device: T,
        sector: u64
    ) -> Result<BiosParameterBlock, Error> {
        let bpb: BiosParameterBlock = unsafe {
            device.read_sector_as::<BiosParameterBlock>(sector).map_err(|e| Error::Io(e))?
        };

        if bpb.bootable_signature != 0xAA55 {
            return Err(Error::BadSignature);
        } else {
            return Ok(bpb);
        }
    }
}

impl fmt::Debug for BiosParameterBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{{");
        writeln!(f, "    bootstrap: {:?},", self.bootstrap);
        writeln!(f, "    bootable_signature: {},", self.bootable_signature);
        writeln!(f, "    fat_num: {}", self.fat_num);
        writeln!(f, "}}")
    }
}
