//! Account-private files. Directory capabilities retain the checked OS object.
//! ref: cap-std cap-std/src/fs/dir.rs; Rust library/std/src/sys/fs/unix.rs.
#[cfg(windows)]
use std::path::PathBuf;
use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod win;

/// A validated private directory. Relative operations cannot select another directory.
pub struct PrivateDirectory {
    handle: File,
    #[cfg(windows)]
    path: PathBuf,
    // Windows retains non-delete-shared ancestor handles until the capability is dropped.
    #[cfg(windows)]
    _ancestors: Vec<File>,
}
fn leaf(path: &Path) -> io::Result<&std::ffi::OsStr> {
    let mut parts = path.components();
    match (parts.next(), parts.next()) {
        (Some(std::path::Component::Normal(name)), None) => Ok(name),
        _ => Err(io::ErrorKind::InvalidInput.into()),
    }
}
impl PrivateDirectory {
    pub fn open(path: &Path) -> io::Result<Self> {
        #[cfg(unix)]
        {
            unix::directory(path, false).map(|handle| Self { handle })
        }
        #[cfg(windows)]
        {
            let (handle, ancestors) = win::open_directory(path)?;
            Ok(Self {
                handle,
                path: path.into(),
                _ancestors: ancestors,
            })
        }
    }
    pub fn create(path: &Path) -> io::Result<Self> {
        #[cfg(unix)]
        {
            unix::directory(path, true).map(|handle| Self { handle })
        }
        #[cfg(windows)]
        {
            win::directory(path)?;
            Self::open(path)
        }
    }
    pub fn open_file(&self, name: &Path) -> io::Result<File> {
        let name = leaf(name)?;
        #[cfg(unix)]
        let file = unix::open_file(&self.handle, name, false)?;
        #[cfg(windows)]
        let file = win::open_file(&self.path.join(name))?;
        validate_file_handle(&file)?;
        Ok(file)
    }
    pub fn create_file(&self, name: &Path) -> io::Result<File> {
        let name = leaf(name)?;
        #[cfg(unix)]
        let file = unix::open_file(&self.handle, name, true)?;
        #[cfg(windows)]
        let file = win::create_new(&self.path.join(name))?;
        validate_file_handle(&file)?;
        Ok(file)
    }
    pub fn read(&self, name: &Path, limit: usize) -> io::Result<Vec<u8>> {
        let file = self.open_file(name)?;
        if file.metadata()?.len() > limit as u64 {
            return Err(io::Error::other("private file limit"));
        }
        let bound = u64::try_from(limit)
            .map_err(io::Error::other)?
            .checked_add(1)
            .ok_or(io::ErrorKind::InvalidInput)?;
        let mut bytes = Vec::new();
        file.take(bound).read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(io::Error::other("private file limit"));
        }
        Ok(bytes)
    }
    pub fn replace(&self, staged: &Path, target: &Path) -> io::Result<()> {
        let staged = Path::new(leaf(staged)?);
        let target = Path::new(leaf(target)?);
        self.sync_file(staged)?;
        match self.open_file(target) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        #[cfg(unix)]
        unix::replace(&self.handle, staged, target)?;
        #[cfg(windows)]
        win::replace(&self.path.join(staged), &self.path.join(target))?;
        self.sync()
    }
    /// Publish without replacing an existing destination. The caller owns staging cleanup.
    pub fn publish_new(&self, staged: &Path, target: &Path) -> io::Result<()> {
        let staged = Path::new(leaf(staged)?);
        let target = Path::new(leaf(target)?);
        self.sync_file(staged)?;
        #[cfg(unix)]
        unix::publish_new(&self.handle, staged, target)?;
        #[cfg(windows)]
        std::fs::hard_link(self.path.join(staged), self.path.join(target))?;
        self.sync()
    }
    fn sync_file(&self, name: &Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            self.open_file(name)?.sync_all()
        }
        #[cfg(windows)]
        {
            let file = win::open_writable(&self.path.join(leaf(name)?))?;
            validate_file_handle(&file)?;
            file.sync_all()
        }
    }
    pub fn sync(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            self.handle.sync_all()
        }
        #[cfg(windows)]
        {
            Ok(())
        } // File publication uses the existing write-through Windows operation.
    }
}
fn validate_file_handle(file: &File) -> io::Result<()> {
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("private regular file required"));
    }
    #[cfg(unix)]
    unix::private_handle(file)?;
    #[cfg(windows)]
    win::file(file)?;
    Ok(())
}
fn parent(path: &Path) -> io::Result<(PrivateDirectory, &Path)> {
    let directory = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let name = Path::new(path.file_name().ok_or(io::ErrorKind::InvalidInput)?);
    Ok((PrivateDirectory::open(directory)?, name))
}
/// Validate private SQLite file metadata without opening or closing its data descriptor.
/// POSIX close releases this process's byte-range locks even on another descriptor;
/// SQLite owns those descriptors and locks. The private parent remains validated.
pub fn validate_sqlite_file(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    unix::sqlite_file(path)?;
    #[cfg(windows)]
    validate(path)?;
    Ok(())
}

/// Validate a directory or regular file, never a symbolic link or special file.
pub fn validate(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let file = unix::open_path(path)?;
        unix::private_handle(&file)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() && !metadata.is_file() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
    }
    #[cfg(windows)]
    {
        if std::fs::symlink_metadata(path)?.is_dir() {
            PrivateDirectory::open(path)?;
        } else {
            let (dir, name) = parent(path)?;
            dir.open_file(name)?;
        }
    }
    Ok(())
}
pub fn directory(path: &Path) -> io::Result<()> {
    PrivateDirectory::create(path).map(|_| ())
}
pub fn read(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let (dir, name) = parent(path)?;
    dir.read(name, limit)
}
pub fn open_existing(path: &Path) -> io::Result<File> {
    let (dir, name) = parent(path)?;
    dir.open_file(name)
}
pub fn create_new(path: &Path) -> io::Result<File> {
    let (dir, name) = parent(path)?;
    dir.create_file(name)
}
pub fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (dir, name) = parent(path)?;
    let mut file = dir.create_file(name)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    dir.sync()
}
fn same_parent<'a>(
    staged: &'a Path,
    target: &'a Path,
) -> io::Result<(PrivateDirectory, &'a Path, &'a Path)> {
    if staged.parent() != target.parent() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let (dir, target_name) = parent(target)?;
    Ok((
        dir,
        Path::new(staged.file_name().ok_or(io::ErrorKind::InvalidInput)?),
        target_name,
    ))
}
pub fn replace(staged: &Path, target: &Path) -> io::Result<()> {
    let (dir, staged, target) = same_parent(staged, target)?;
    dir.replace(staged, target)
}
pub fn publish_new(staged: &Path, target: &Path) -> io::Result<()> {
    let (dir, staged, target) = same_parent(staged, target)?;
    dir.publish_new(staged, target)
}
pub fn sync_parent(path: &Path) -> io::Result<()> {
    let (dir, _) = parent(path)?;
    dir.sync()
}

/// Require a unique pathname for consumers whose journal naming cannot tolerate hard-link aliases.
/// No such restriction applies to the transient two-link no-replace publication operation.
pub fn validate_single_link(path: &Path) -> io::Result<()> {
    let file = open_existing(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if file.metadata()?.nlink() != 1 {
            return Err(io::Error::other("private file aliases"));
        }
    }
    #[cfg(windows)]
    win::single_link(&file)?;
    Ok(())
}
