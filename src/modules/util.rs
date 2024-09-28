use std::error::Error;
use std::fs::File;
use std::{fs, io};
use std::io::BufRead;
use std::path::{Path, PathBuf};


pub type BoxResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone)]
pub enum EitherOr<X, Z> {
    A(X),
    B(Z),
}

pub fn read_lines<P>(path: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(path)?;
    Ok(io::BufReader::new(file).lines())
}

pub fn walk_dir(path: impl AsRef<Path>, max_depth: i32) -> io::Result<Vec<PathBuf>> {
    if max_depth == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "max depth reached"));
    }
    let mut buf = vec![];
    let entries = fs::read_dir(path)?;

    for entry in entries {
        let entry = entry?;
        let meta = entry.metadata()?;

        if meta.is_dir() {
            let mut sub_dir = walk_dir(entry.path(), max_depth - 1)?;
            buf.append(&mut sub_dir);
        }

        if meta.is_file() {
            buf.push(entry.path());
        }
    }

    Ok(buf)
}