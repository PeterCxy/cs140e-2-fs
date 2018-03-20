use std::io;
use std::path::{Path, Component};
use std::mem::size_of;
use std::cmp::min;

use util::SliceExt;
use mbr::{MasterBootRecord, PartitionEntry};
use vfat::{Shared, Cluster, ClusterIter, File, Dir, Entry, FatEntry, Error, Status};
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
            .find_partition_with_type(0xC)
            .or_else(|| mbr.find_partition_with_type(0xC))
            .ok_or(Error::NotFound)?;
        let ebpb_info = BiosParameterBlock::from(&mut device, fat32_part.relative_sector as u64)?;
        let fat_start_sector = (fat32_part.relative_sector as u64) + ebpb_info.reserved_sectors as u64;
        let sector_per_fat = ebpb_info.get_sector_per_fat() as u32;
        let data_start_sector = fat_start_sector + (ebpb_info.fat_num as u64) * (sector_per_fat as u64);

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

    // Find the starting sector of a given cluster
    #[inline(always)]
    fn cluster_to_sector(&self, cluster: Cluster) -> u64 {
        self.data_start_sector + (cluster.get() as u64 - 2) * self.sectors_per_cluster as u64
    }

    // Calculate length of a cluster
    #[inline(always)]
    fn bytes_per_cluster(&self) -> usize {
        (self.bytes_per_sector as usize) * (self.sectors_per_cluster as usize)
    }

    // Read a full cluster into a buffer (or fill the buffer)
    // including all the sectors inside the cluster
    fn _read_cluster(&mut self, cluster: Cluster, buf: &mut [u8]) -> io::Result<usize> {
        let start_sector = self.cluster_to_sector(cluster);
        let mut bytes_read = 0;

        // Read all the sectors and join them
        // as the data of the cluster
        for i in 0..self.sectors_per_cluster {
            let cur_start = (i as u64 * self.device.sector_size()) as usize;

            // Read a full sector into the buffer
            // or fill the buffer if remaining buffer length is not enough
            let cur_end = min(buf.len(), cur_start + self.device.sector_size() as usize);
            bytes_read += self.device.read_sector(start_sector + i as u64, &mut buf[cur_start..cur_end])?;

            // buffer full, exit
            if cur_end == buf.len() {
                break;
            }
        }
        Ok(bytes_read)
    }

    // A method to return a reference to a `FatEntry` for a cluster where the
    // reference points directly into a cached sector.
    pub fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        // Calculate which sector the FAT entry of the cluster is in
        let mut fat_offset = 4 * cluster.get() as usize;
        let sector_offset = fat_offset / (self.bytes_per_sector as usize);
        fat_offset = fat_offset % (self.bytes_per_sector as usize);
        if sector_offset >= self.sectors_per_fat as usize {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Out of boundary of FAT"));
        }
        let data = self.device.get(self.fat_start_sector + sector_offset as u64)?;
        return Ok(unsafe {
            &*(data[fat_offset..(fat_offset + 4)].as_ptr() as *const FatEntry)
        })
    }
}

pub trait VFatExt {
    // A method to read all of the clusters chained from a starting cluster
    //    into a vector.
    fn read_chain(
        &self,
        start: Cluster,
        buf: &mut Vec<u8>
    ) -> io::Result<usize>;

    // A method to read from an offset of a cluster into a buffer.
    // read in the cluster chain until `buffer` is full.
    // if the `offset` is out of cluster boundary, this method
    // will automatically skip to the target cluster
    fn read_cluster(
        &self,
        cluster: Cluster,
        offset: usize,
        buf: &mut [u8]
    ) -> io::Result<usize>;
}

impl VFatExt for Shared<VFat> {
    fn read_chain(
        &self,
        start: Cluster,
        buf: &mut Vec<u8>
    ) -> io::Result<usize> {
        buf.clear();
        let cluster_bytes = self.borrow().bytes_per_cluster();
        for cluster in start.iter(self.clone()) {
            let cur_cluster = cluster?;
            let buf_start = buf.len();

            // Append data of the current cluster into the chain
            buf.resize(buf_start + cluster_bytes, 0);
            self.borrow_mut()._read_cluster(cur_cluster, &mut buf[buf_start..])?;
        }
        return Ok(buf.len());
    }

    fn read_cluster(
        &self,
        cluster: Cluster,
        offset: usize,
        buf: &mut [u8]
    ) -> io::Result<usize> {
        let cluster_bytes = self.borrow().bytes_per_cluster();

        // How many clusters in a chain should be skipped for `offset`
        // because we allow `offset`ing  over cluster boundary
        let skip_clusters = offset / cluster_bytes;

        // How much offset is still remaining after `skip_clusters`
        // when `offset` is not a multiple of `cluster_bytes`
        let mut cur_offset = offset % cluster_bytes;

        // The current position in `buf`
        // Subsequent reads should start from this position
        let mut cur_buf_pos = 0;
        for c in cluster.iter(self.clone()).skip(skip_clusters) {
            if cur_buf_pos >= buf.len() {
                break;
            }

            let cur_cluster = c?;

            // Temporary buffer to hold the full data from the cluster
            let mut cluster_buf = vec![0u8; cluster_bytes];
            self.borrow_mut()._read_cluster(cur_cluster, &mut cluster_buf)?;

            // Calculate how much data should be read for this cluster
            // i.e. read the full cluster (minus `cur_offset`)
            //    or fill the buffer if buffer is not enough
            let cur_buf_end = min(buf.len(), cur_buf_pos + cluster_bytes - cur_offset);
            let cur_read_len = cur_buf_end - cur_buf_pos;
            let mut cluster_slice = &cluster_buf[cur_offset..(cur_offset + cur_read_len)];
            let mut buf_slice = &mut buf[cur_buf_pos..cur_buf_end];
            io::copy(&mut cluster_slice, &mut buf_slice)?;

            // Shift the position to prepare for the next read
            cur_buf_pos += cur_read_len;
            cur_offset = 0;
        }
        Ok(cur_buf_pos)
    }
}

impl<'a> FileSystem for &'a Shared<VFat> {
    type File = File;
    type Dir = Dir;
    type Entry = Entry;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
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
