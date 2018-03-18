use traits;
use vfat::{File, Dir, Metadata, Cluster};

// TODO: You may need to change this definition.
#[derive(Debug)]
pub enum Entry {
    File(File),
    Dir(Dir)
}

// TODO: Implement any useful helper methods on `Entry`.

impl traits::Entry for Entry {
    type File = File;
    type Dir = Dir;
    type Metadata = Metadata;

    fn name(&self) -> &str {
        match *self {
            Entry::File(ref file) => &file.name,
            Entry::Dir(ref dir) => &dir.name
        }
    }

    fn metadata(&self) -> &Metadata {
        match *self {
            Entry::File(ref file) => &file.metadata,
            Entry::Dir(ref dir) => &dir.metadata
        }
    }

    fn as_file(&self) -> Option<&File> {
        match *self {
            Entry::File(ref file) => Some(file),
            Entry::Dir(_) => None
        }
    }

    fn as_dir(&self) -> Option<&Dir> {
        match *self {
            Entry::File(_) => None,
            Entry::Dir(ref dir) => Some(dir)
        }
    }

    fn into_file(self) -> Option<File> {
        match self {
            Entry::File(file) => Some(file),
            Entry::Dir(_) => None
        }
    }

    fn into_dir(self) -> Option<Dir> {
        match self {
            Entry::File(_) => None,
            Entry::Dir(dir) => Some(dir)
        }
    }
}