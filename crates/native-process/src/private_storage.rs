//! User-owned runtime files. Windows validates the actual open handle's ACL.
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};
#[cfg(windows)]
mod win;

pub fn validate(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other("private path required"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::other("private ownership required"));
        }
    }
    #[cfg(windows)]
    {
        win::validate(path)?;
    }
    Ok(())
}
pub fn directory(path: &Path) -> io::Result<()> {
    if !path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(path)?;
        }
        #[cfg(windows)]
        {
            win::directory(path)?;
        }
    }
    validate(path)?;
    if !path.is_dir() {
        return Err(io::Error::other("private directory required"));
    }
    Ok(())
}
pub fn read(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    validate(
        path.parent()
            .ok_or_else(|| io::Error::other("parent required"))?,
    )?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() > limit as u64 {
        return Err(io::Error::other("private file limit"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err(io::Error::other("private ownership"));
        }
    }
    #[cfg(windows)]
    {
        win::file(&file)?;
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::other("private file limit"));
    }
    Ok(bytes)
}
pub fn create_new(path: &Path) -> io::Result<File> {
    validate(
        path.parent()
            .ok_or_else(|| io::Error::other("parent required"))?,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
    }
    #[cfg(windows)]
    {
        win::create_new(path)
    }
}
pub fn replace(staged: &Path, path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::rename(staged, path)?;
        File::open(
            path.parent()
                .ok_or_else(|| io::Error::other("parent required"))?,
        )?
        .sync_all()
    }
    #[cfg(windows)]
    {
        win::replace(staged, path)
    }
}
pub fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = create_new(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
#[cfg(windows)]
pub use win::{enter_secret, key_path, protect_key, random_key, save_dialog, unprotect_key};
