use std::error::Error;
use std::fs::File;
use std::{fs, io};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Command;

pub type BoxResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EitherOr<X, Y> {
    A(X),
    B(Y),
}

pub(crate) fn read_lines<P>(path: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(path)?;
    Ok(io::BufReader::new(file).lines())
}

pub(crate) fn walk_dir(path: impl AsRef<Path>, max_depth: i32) -> io::Result<Vec<PathBuf>> {
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

pub(crate) fn get_item_list(data_input: EitherOr<String,String>) -> BoxResult<String>{
    match data_input {
        EitherOr::A(file) => {
            Ok(read_lines(file)?.map_while(Result::ok).collect::<Vec<String>>().join("\n"))
        }
        EitherOr::B(list) => {
            Ok(list)
        }
    }
}

pub(crate) fn get_last_git_activity(registry_root: &str, path: &str) -> BoxResult<u64> {
    let cmd_output = Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--format=%ct")
        .arg(path)
        .current_dir(registry_root)
        .output()?;
    if !cmd_output.status.success() {
        eprintln!("{:?}", String::from_utf8_lossy(&cmd_output.stderr));
        return Err("git log failed".into());
    }
    let output = String::from_utf8(cmd_output.stdout)?;
    let output_clean = match output.strip_suffix('\n') {
        Some(s) => s,
        None => output.as_str()
    };
    Ok(output_clean.parse::<u64>()?)
}