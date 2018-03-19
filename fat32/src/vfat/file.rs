use std::cmp::{min, max};
use std::io::{self, SeekFrom};

use traits;
use vfat::{VFat, VFatExt, Shared, Cluster, Metadata};

#[derive(Debug)]
pub struct File {
    pub drive: Shared<VFat>,
    pub cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
    pub size: u64,
    pub offset: u64
}

impl File {
    fn set_offset(&mut self, pos: u64) -> io::Result<u64> {
        if pos > self.size {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "Cannot seek beyond file end"))
        } else {
            self.offset = pos;
            Ok(self.offset)
        }
    }
}

impl traits::File for File {
    fn sync(&mut self) -> io::Result<()> {
        unimplemented!();
    }

    fn size(&self) -> u64 {
        self.size as u64
    }
}

impl io::Seek for File {
    /// Seek to offset `pos` in the file.
    ///
    /// A seek to the end of the file is allowed. A seek _beyond_ the end of the
    /// file returns an `InvalidInput` error.
    ///
    /// If the seek operation completes successfully, this method returns the
    /// new position from the start of the stream. That position can be used
    /// later with SeekFrom::Start.
    ///
    /// # Errors
    ///
    /// Seeking before the start of a file or beyond the end of the file results
    /// in an `InvalidInput` error.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let cur_offset = self.offset;
        let cur_size = self.size;
        match pos {
            SeekFrom::Start(p) => self.set_offset(p),
            SeekFrom::Current(p) => self.set_offset(((cur_offset as i64) + p) as u64),
            SeekFrom::End(p) => self.set_offset(((cur_size as i64) - 1 + p) as u64)
        }
    }
}

impl io::Read for File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use std::io::Seek;
        let max_len = min((self.size - self.offset) as usize, buf.len());
        let read_bytes = self.drive.read_cluster(self.cluster, self.offset as usize, &mut buf[..max_len])?;
        self.seek(SeekFrom::Current(read_bytes as i64))?;
        Ok(read_bytes)
    }
}

impl io::Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unimplemented!();
    }

    fn flush(&mut self) -> io::Result<()> {
        unimplemented!();
    }
}

