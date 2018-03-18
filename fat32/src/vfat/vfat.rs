use std::io;
use std::path::{Path, Component};
use std::mem::size_of;
use std::cmp::min;

use util::SliceExt;
use mbr::{MasterBootRecord, PartitionEntry};
use vfat::{Shared, Cluster, File, Dir, Entry, FatEntry, Error, Status};
use vfat::{BiosParameterBlock, CachedDevice, Partition};
use traits::{FileSystem, BlockDevice};

#[derive(Debug)]
pub struct VFat {
    device: CachedDevice,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    sectors_per_fat: u32,
    fat_start_sector: u64,
    data_start_sector: u64,
    root_dir_cluster: Cluster,
}

impl VFat {
    pub fn from<T>(mut device: T) -> Result<Shared<VFat>, Error>
        where T: BlockDevice + 'static
    {
        let mbr = MasterBootRecord::from(&mut device).map_err(|e| Error::Mbr(e))?;

        // Find the first fat32 partition
        let fat32_part = mbr
            .find_partition_with_type(0xB)
            .or_else(|| mbr.find_partition_with_type(0xC))
            .ok_or(Error::NotFound)?;
        let ebpb_info = BiosParameterBlock::from(&mut device, fat32_part.relative_sector as u64)?;
        let fat_start_sector = (fat32_part.relative_sector as u64) + ebpb_info.reserved_sectors as u64;
        let sector_per_fat = ebpb_info.get_sector_per_fat() as u32;
        let data_start_sector = fat_start_sector + (ebpb_info.fat_num as u64) * (sector_per_fat as u64);
        //println!("{:?}", fat32_part);
        //println!("{:?}", ebpb_info);

        Ok(Shared::new(VFat {
            device: CachedDevice::new(device, Partition {
                start: fat32_part.relative_sector as u64,
                sector_size: ebpb_info.bytes_per_sector as u64
            }),
            bytes_per_sector: ebpb_info.bytes_per_sector,
            sectors_per_cluster: ebpb_info.sectors_per_cluster,
            fat_start_sector,
            sectors_per_fat: sector_per_fat,
            data_start_sector,
            root_dir_cluster: Cluster::from(ebpb_info.root_cluster)
        }))
    }

    fn cluster_to_sector(&self, cluster: Cluster) -> u64 {
        self.data_start_sector + (cluster.get() as u64 - 2) * self.sectors_per_cluster as u64
    }

    // A method to read from an offset of a cluster into a buffer.
    // TODO: Reimplement this method
    // This method is not working
    fn read_cluster(
        &mut self,
        cluster: Cluster,
        offset: usize,
        buf: &mut [u8]
    ) -> io::Result<usize> {
        let mut cur_cluster = cluster;
        let mut current_offset = offset;
        let mut current_pos = 0;
        let mut read_len = 0;
        while current_pos < buf.len() {
            let sector = self.cluster_to_sector(cur_cluster);
            {
                let data = self.device.get(sector)?;
                let len = data.len() - current_offset;
                if current_pos + len >= buf.len() {
                    break;
                }
                buf[current_pos..(current_pos + len)].clone_from_slice(&data[current_pos..]);
                current_offset = 0;
                current_pos += len;
                read_len += len;
            }
            match self.fat_entry(cluster)?.status() {
                Status::Data(cluster) => cur_cluster = cluster,
                _ => break
            }
        }
        return Ok(buf.len());
    }

    // A method to read all of the clusters chained from a starting cluster
    //    into a vector.
    pub fn read_chain(
        &mut self,
        start: Cluster,
        buf: &mut Vec<u8>
    ) -> io::Result<usize> {
        buf.clear();
        let cluster_bytes = (self.bytes_per_sector as usize) * (self.sectors_per_cluster as usize);
        let mut cur_cluster = start;
        loop {
            let buf_start = buf.len();
            buf.resize(buf_start + cluster_bytes, 0);
            let sector = self.cluster_to_sector(cur_cluster);
            self.device.read_sector(sector, &mut buf[buf_start..])?;
            let status = self.fat_entry(cur_cluster)?.status();
            match status {
                Status::Data(cluster) => cur_cluster = cluster,
                Status::Eoc(_) => break,
                _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid FAT32 Cluster chain"))
            }
        }
        return Ok(buf.len());
    }

    // A method to return a reference to a `FatEntry` for a cluster where the
    // reference points directly into a cached sector.
    fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        // Calculate which sector the FAT entry of the cluster is in
        let mut fat_offset = 4 * cluster.get() as usize;
        let sector_offset = fat_offset / (self.bytes_per_sector as usize);
        //println!("{:?}", self);
        //println!("sector offset {}", sector_offset);
        fat_offset = fat_offset % (self.bytes_per_sector as usize);
        if sector_offset >= self.sectors_per_fat as usize {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Out of boundary of FAT"));
        }
        let data = self.device.get(self.fat_start_sector + sector_offset as u64)?;
        //println!("{:?}", &data[..]);
        //println!("{:?}", &data[fat_offset..(fat_offset + 4)]);
        return Ok(unsafe {
            &*(data[fat_offset..(fat_offset + 4)].as_ptr() as *const FatEntry)
        })
    }
}

impl<'a> FileSystem for &'a Shared<VFat> {
    type File = File;
    type Dir = Dir;
    type Entry = Entry;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        //println!("path {:?}", path.as_ref().to_str());
        let mut cur_dir = Entry::Dir(Dir::from_root_cluster(self.clone(), self.borrow().root_dir_cluster));
        let mut first = true;
        for p in path.as_ref().components() {
            if let Component::RootDir = p {
                first = false;
                continue;
            }

            if first {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Can only start from root"));
            }

            if let Component::Normal(name) = p {
                match cur_dir {
                    Entry::Dir(dir) => cur_dir = dir.find(name)?,
                    Entry::File(_) => return Err(io::Error::new(io::ErrorKind::NotFound, "Not a folder"))
                }
            } else {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Can only start from root"));
            }
        }
        return Ok(cur_dir);
    }

    fn create_file<P: AsRef<Path>>(self, _path: P) -> io::Result<Self::File> {
        unimplemented!("read only file system")
    }

    fn create_dir<P>(self, _path: P, _parents: bool) -> io::Result<Self::Dir>
        where P: AsRef<Path>
    {
        unimplemented!("read only file system")
    }

    fn rename<P, Q>(self, _from: P, _to: Q) -> io::Result<()>
        where P: AsRef<Path>, Q: AsRef<Path>
    {
        unimplemented!("read only file system")
    }

    fn remove<P: AsRef<Path>>(self, _path: P, _children: bool) -> io::Result<()> {
        unimplemented!("read only file system")
    }
}
