//! Bounded reads and immutable, synchronized checkpoint publication.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub fn read(path: &Path, limit: usize) -> io::Result<String> {
    // Reject a named pipe before open can wait for a writer. Check the opened
    // handle again as well, since paths may change between these operations.
    if !fs::metadata(path)?.is_file() {
        return Err(io::Error::other("checkpoint/input is not a regular file"));
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || u128::from(metadata.len()) > limit as u128 {
        return Err(io::Error::other(
            "checkpoint/input is not a regular file within the byte cap",
        ));
    }
    let mut bytes = Vec::new();
    file.take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::other("checkpoint/input exceeds the byte cap"));
    }
    String::from_utf8(bytes).map_err(io::Error::other)
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}
pub fn reserve(path: &Path) -> io::Result<()> {
    fs::create_dir(path)?;
    // Persist the directory entry before any draw. Existing destinations,
    // including symlinks and directories from interrupted calls, are refused.
    sync_directory(parent(path))?;
    sync_directory(path)
}
fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

pub fn publish(directory: &Path, index: u128, text: &str, limit: usize) -> io::Result<PathBuf> {
    if text.len() > limit {
        return Err(io::Error::other(
            "encoded CLI checkpoint exceeds --max-checkpoint-bytes",
        ));
    }
    let destination = directory.join(format!("checkpoint-{index:020}.json"));
    let temporary = directory.join(format!(".checkpoint-{index:020}.pending"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        // hard_link atomically refuses an existing destination; rename would
        // overwrite it on Unix. The fully written inode is visible all at once.
        fs::hard_link(&temporary, &destination)?;
        sync_directory(directory)?;
        fs::remove_file(&temporary)?;
        sync_directory(directory)?;
        Ok(destination)
    })();
    if result.is_err() {
        // Never remove a published checkpoint: even if directory synchronization
        // failed, an earlier committed checkpoint remains untouched.
        let _ = fs::remove_file(&temporary);
    }
    result
}
