use std::cmp;
use std::{io, fmt};
use std::collections::HashMap;

use traits::BlockDevice;

#[derive(Debug)]
struct CacheEntry {
    data: Vec<u8>,
    dirty: bool
}

pub struct Partition {
    /// The physical sector where the partition begins.
    pub start: u64,
    /// The size, in bytes, of a logical sector in the partition.
    pub sector_size: u64
}

pub struct CachedDevice {
    device: Box<BlockDevice>,
    cache: HashMap<u64, CacheEntry>,
    partition: Partition
}

impl CachedDevice {
    /// Creates a new `CachedDevice` that transparently caches sectors from
    /// `device` and maps physical sectors to logical sectors inside of
    /// `partition`. All reads and writes from `CacheDevice` are performed on
    /// in-memory caches.
    ///
    /// The `partition` parameter determines the size of a logical sector and
    /// where logical sectors begin. An access to a sector `n` _before_
    /// `partition.start` is made to physical sector `n`. Cached sectors before
    /// `partition.start` are the size of a physical sector. An access to a
    /// sector `n` at or after `partition.start` is made to the _logical_ sector
    /// `n - partition.start`. Cached sectors at or after `partition.start` are
    /// the size of a logical sector, `partition.sector_size`.
    ///
    /// `partition.sector_size` must be an integer multiple of
    /// `device.sector_size()`.
    ///
    /// # Panics
    ///
    /// Panics if the partition's sector size is < the device's sector size.
    pub fn new<T>(device: T, partition: Partition) -> CachedDevice
        where T: BlockDevice + 'static
    {
        assert!(partition.sector_size >= device.sector_size());

        CachedDevice {
            device: Box::new(device),
            cache: HashMap::new(),
            partition: partition
        }
    }

    /// Maps a user's request for a sector `virt` to the physical sector and
    /// number of physical sectors required to access `virt`.
    fn virtual_to_physical(&self, virt: u64) -> (u64, u64) {
        if self.device.sector_size() == self.partition.sector_size {
            (virt, 1)
        } else if virt < self.partition.start {
            (virt, 1)
        } else {
            let factor = self.partition.sector_size / self.device.sector_size();
            let logical_offset = virt - self.partition.start;
            let physical_offset = logical_offset * factor;
            let physical_sector = self.partition.start + physical_offset;
            (physical_sector, factor)
        }
    }

    // Ensure that `sector` is read and inside cache
    fn ensure_cache(&mut self, sector: u64) -> io::Result<()> {
        if !self.cache.contains_key(&sector) {
            let mut buf = vec![0u8; self.partition.sector_size as usize];
            let device_sector_size = self.device.sector_size() as usize;
            let (device_sector, num) = self.virtual_to_physical(sector);
            for i in 0..(num as usize) {
                let start = i * device_sector_size;
                self.device.read_sector(device_sector + i as u64, &mut buf[start..(start + device_sector_size)])?;
            }
            self.cache.insert(sector, CacheEntry {
                data: buf,
                dirty: false
            });
        }
        Ok(())
    }

    /// Returns a mutable reference to the cached sector `sector`. If the sector
    /// is not already cached, the sector is first read from the disk.
    ///
    /// The sector is marked dirty as a result of calling this method as it is
    /// presumed that the sector will be written to. If this is not intended,
    /// use `get()` instead.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an error reading the sector from the disk.
    pub fn get_mut(&mut self, sector: u64) -> io::Result<&mut [u8]> {
        self.ensure_cache(sector)?;
        Ok(&mut self.cache.get_mut(&sector).unwrap().data)
    }

    /// Returns a reference to the cached sector `sector`. If the sector is not
    /// already cached, the sector is first read from the disk.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an error reading the sector from the disk.
    pub fn get(&mut self, sector: u64) -> io::Result<&[u8]> {
        self.ensure_cache(sector)?;
        Ok(&self.cache.get_mut(&sector).unwrap().data)
    }
}

// FIXME: Implement `BlockDevice` for `CacheDevice`. The `read_sector` and
// `write_sector` methods should only read/write from/to cached sectors.
impl BlockDevice for CachedDevice {
    fn sector_size(&self) -> u64 {
        self.partition.sector_size
    }

    fn read_sector(&mut self, n: u64, mut buf: &mut [u8]) -> io::Result<usize> {
        let len = cmp::min(buf.len(), self.partition.sector_size as usize);
        let mut sector = &self.get(n)?[..len];
        io::copy(&mut sector, &mut buf)?;
        Ok(len as usize)
    }

    fn write_sector(&mut self, n: u64, buf: &[u8]) -> io::Result<usize> {
        unimplemented!("BlockDevice::write() unimplemented!");
        /*let len = cmp::min(buf.len() as u64, self.partition.sector_size);

        if self.cache.contains_key(&n) {
            let cache_entry = self.cache.get_mut(&n).unwrap();
            cache_entry.dirty = true;
            //cache_entry.data[..].clone_from_slice(buf);
            cache_entry.data[..len].clone_from_slice(buf);
        } else {
            self.cache.insert(n, CacheEntry {
                data: Vec::from(buf.clone()),
                dirty: true
            });
        }
        Ok(buf.len())*/
    }
}

impl fmt::Debug for CachedDevice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CachedDevice")
            .field("device", &"<block device>")
            .field("cache", &self.cache)
            .finish()
    }
}
