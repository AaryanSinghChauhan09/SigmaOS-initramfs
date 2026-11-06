//! cpio `newc` (CRC-free ASCII) archive writer — the initramfs image format
//! used by Linux bootloaders (cpio -H newc), mirroring mkinitcpio/archiso output.
//!
//! Format per entry:
//!   header: "070701" + 13 × 8-hex-digit fields + "\0"
//!   name:   NUL-terminated, padded to 4-byte boundary (with header)
//!   data:   filesize bytes, padded to 4-byte boundary
//! Trailer entry "TRAILER!!!" marks end of archive.

use std::fmt;
use std::io;

/// File type + permission bits stored in `mode` (stat(2) `st_mode` layout).
pub mod mode {
    pub const S_IFREG: u32 = 0o100000;
    pub const S_IFDIR: u32 = 0o040000;
    pub const S_IFLNK: u32 = 0o120000;
    pub const S_IRUSR: u32 = 0o400;
    pub const S_IWUSR: u32 = 0o200;
    pub const S_IXUSR: u32 = 0o100;
    /// Regular file, 0644 — the common case for initramfs payloads.
    pub const REG_0644: u32 = S_IFREG | 0o644;
    /// Directory, 0755.
    pub const DIR_0755: u32 = S_IFDIR | 0o755;
}

/// One file to place in the archive.
#[derive(Debug, Clone)]
pub struct CpioEntry {
    /// Archive path — must be relative, no leading `/`, no `..`.
    pub path: String,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    /// File contents; empty for directories and symlinks.
    pub data: Vec<u8>,
}

impl CpioEntry {
    pub fn file(path: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            mode: mode::REG_0644,
            uid: 0,
            gid: 0,
            data: data.into(),
        }
    }
    pub fn dir(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            mode: mode::DIR_0755,
            uid: 0,
            gid: 0,
            data: Vec::new(),
        }
    }
    pub fn symlink(path: impl Into<String>, target: &str) -> Self {
        Self {
            path: path.into(),
            mode: mode::S_IFLNK | 0o777,
            uid: 0,
            gid: 0,
            data: target.as_bytes().to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpioError {
    /// Path must be relative and contain no `..` components.
    BadPath(String),
    /// Duplicate archive path.
    DuplicatePath(String),
    /// Write failed.
    Io(String),
}

impl fmt::Display for CpioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpioError::BadPath(p) => write!(f, "bad cpio path: {p}"),
            CpioError::DuplicatePath(p) => write!(f, "duplicate cpio path: {p}"),
            CpioError::Io(m) => write!(f, "cpio write error: {m}"),
        }
    }
}
impl std::error::Error for CpioError {}
impl From<io::Error> for CpioError {
    fn from(e: io::Error) -> Self {
        CpioError::Io(e.to_string())
    }
}

/// Ordered initramfs image builder — entries serialize in insertion order,
/// matching `find | cpio` pipeline semantics.
#[derive(Default)]
pub struct InitramfsBuilder {
    entries: Vec<CpioEntry>,
}

impl InitramfsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry; fails fast on bad/duplicate paths.
    pub fn add(&mut self, entry: CpioEntry) -> Result<&mut Self, CpioError> {
        validate_path(&entry.path)?;
        if self.entries.iter().any(|e| e.path == entry.path) {
            return Err(CpioError::DuplicatePath(entry.path));
        }
        self.entries.push(entry);
        Ok(self)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Serialize the archive (newc format) including TRAILER!!! terminator.
    pub fn build(&self) -> Result<Vec<u8>, CpioError> {
        let mut out = Vec::new();
        let mut ino: u32 = 1;
        for e in &self.entries {
            write_entry(&mut out, ino, e)?;
            ino += 1;
        }
        // Trailer: namesize covers "TRAILER!!!\0" = 11 bytes.
        write_header(&mut out, ino, mode::REG_0644, 0, 0, 1, 0, 0, 0, 0, 0, 11)?;
        out.extend_from_slice(b"TRAILER!!!\0");
        pad4(&mut out);
        Ok(out)
    }
}

fn validate_path(path: &str) -> Result<(), CpioError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.split('/').any(|c| c == "..")
        || path.contains('\0')
    {
        return Err(CpioError::BadPath(path.to_string()));
    }
    Ok(())
}

fn write_header(
    out: &mut Vec<u8>,
    ino: u32,
    mode_: u32,
    uid: u32,
    gid: u32,
    nlink: u32,
    filesize: u32,
    devmajor: u32,
    devminor: u32,
    rdevmajor: u32,
    rdevminor: u32,
    namesize: u32,
) -> Result<(), CpioError> {
    out.extend_from_slice(b"070701");
    for field in [
        ino, mode_, uid, gid, nlink, 0, filesize, devmajor, devminor, rdevmajor, rdevminor,
        namesize, 0,
    ] {
        out.extend_from_slice(format!("{field:08x}").as_bytes());
    }
    out.push(0);
    Ok(())
}

fn write_entry(out: &mut Vec<u8>, ino: u32, e: &CpioEntry) -> Result<(), CpioError> {
    let namesize = e.path.len() as u32 + 1; // + NUL
    write_header(
        out,
        ino,
        e.mode,
        e.uid,
        e.gid,
        1,
        e.data.len() as u32,
        0,
        0,
        0,
        0,
        namesize,
    )?;
    out.extend_from_slice(e.path.as_bytes());
    out.push(0);
    pad4(out);
    out.extend_from_slice(&e.data);
    pad4(out);
    Ok(())
}

/// Pad `out` with NULs to a 4-byte boundary (cpio newc alignment).
fn pad4(out: &mut Vec<u8>) {
    while out.len() % 4 != 0 {
        out.push(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_starts_with_newc_magic() {
        let mut b = InitramfsBuilder::new();
        b.add(CpioEntry::file("init", b"#!/bin/sh\n")).unwrap();
        let img = b.build().unwrap();
        assert_eq!(&img[..6], b"070701");
    }

    #[test]
    fn trailer_terminates_archive() {
        let mut b = InitramfsBuilder::new();
        b.add(CpioEntry::dir("etc")).unwrap();
        let img = b.build().unwrap();
        assert!(img.windows(10).any(|w| w == b"TRAILER!!!"));
    }

    #[test]
    fn entry_order_preserved() {
        let mut b = InitramfsBuilder::new();
        b.add(CpioEntry::file("a", b"1")).unwrap();
        b.add(CpioEntry::file("b", b"2")).unwrap();
        let img = b.build().unwrap();
        let a = img.windows(1).position(|_| false); // silence unused
        let _ = a;
        let pos_a = find_sub(&img, b"a\0");
        let pos_b = find_sub(&img, b"b\0");
        assert!(pos_a < pos_b);
    }

    fn find_sub(hay: &[u8], needle: &[u8]) -> usize {
        hay.windows(needle.len()).position(|w| w == needle).unwrap()
    }

    #[test]
    fn bad_paths_rejected() {
        let mut b = InitramfsBuilder::new();
        assert!(matches!(
            b.add(CpioEntry::file("/abs", b"")),
            Err(CpioError::BadPath(_))
        ));
        assert!(matches!(
            b.add(CpioEntry::file("../escape", b"")),
            Err(CpioError::BadPath(_))
        ));
    }

    #[test]
    fn duplicate_paths_rejected() {
        let mut b = InitramfsBuilder::new();
        b.add(CpioEntry::file("x", b"1")).unwrap();
        assert!(matches!(
            b.add(CpioEntry::file("x", b"2")),
            Err(CpioError::DuplicatePath(_))
        ));
    }

    #[test]
    fn symlink_data_is_target() {
        let mut b = InitramfsBuilder::new();
        b.add(CpioEntry::symlink("bin/sh", "/bin/bash")).unwrap();
        let img = b.build().unwrap();
        assert!(img.windows(9).any(|w| w == b"/bin/bash"));
    }
}
