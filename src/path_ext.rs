use std::os::unix::prelude::OsStrExt;
use std::path::Path;

pub trait PathExt {
    fn is_hidden(&self) -> bool;
}

impl PathExt for Path {
    fn is_hidden(&self) -> bool {
        self.file_name()
            .map_or(false, |name| name.as_bytes().starts_with(&[b'.']))
    }
}
