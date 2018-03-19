use vfat::*;
use std::io;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone, Hash)]
pub struct Cluster(u32);

impl Cluster {
    // Iterate over cluster chain, using `self` as starting point
    // needs a `drive` to fetch FAT information from
    pub fn iter(&self, drive: Shared<VFat>) -> ClusterIter {
        ClusterIter::from(drive, *self)
    }
}

impl From<u32> for Cluster {
    fn from(raw_num: u32) -> Cluster {
        Cluster(raw_num & !(0xF << 28))
    }
}

impl Cluster {
    pub fn get(&self) -> u32 {
        self.0
    }
}

/*
 * Iterator over a cluster chain
 * each item is a `io::Result`
 * with success value for the next cluster
 * and error value for the errors that may
 * occur when reading from the FAT
 */
pub struct ClusterIter {
    drive: Shared<VFat>,
    current: Cluster,
    eoc_occurred: bool
}

impl ClusterIter {
    fn from(drive: Shared<VFat>, start: Cluster) -> ClusterIter {
        ClusterIter {
            drive,
            current: start,
            eoc_occurred: false
        }
    }
}

impl Iterator for ClusterIter {
    type Item = io::Result<Cluster>;

    fn next(&mut self) -> Option<io::Result<Cluster>> {
        if self.eoc_occurred {
            return None;
        }

        match self.drive.borrow_mut().fat_entry(self.current) {
            Err(e) => Some(Err(e)),
            Ok(entry) => match entry.status() {
                Status::Data(next) => {
                    let current = self.current;
                    self.current = next;
                    Some(Ok(current))
                },
                Status::Eoc(_) => {
                    self.eoc_occurred = true;
                    Some(Ok(self.current))
                },
                _ => Some(Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid FAT chain")))
            }
        }
    }
}