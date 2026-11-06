//! Directory → initramfs image packer.
//!
//! Walks a root filesystem tree (like `find | cpio -o -H newc`), maps each
//! file to a [`CpioEntry`], and writes the archive — the mkinitcpio/archiso
//! "generate image" step on top of the [`crate::cpio`] writer.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::cpio::{CpioEntry, CpioError, InitramfsBuilder};

/// Errors from packing a directory tree.
#[derive(Debug)]
pub enum PackError {
    Io(io::Error),
    Cpio(CpioError),
    /// Root path is not a directory.
    NotADirectory(PathBuf),
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackError::Io(e) => write!(f, "i/o error while packing: {e}"),
            PackError::Cpio(e) => write!(f, "{e}"),
            PackError::NotADirectory(p) => write!(f, "not a directory: {}", p.display()),
        }
    }
}
impl std::error::Error for PackError {}
impl From<io::Error> for PackError {
    fn from(e: io::Error) -> Self {
        PackError::Io(e)
    }
}
impl From<CpioError> for PackError {
    fn from(e: CpioError) -> Self {
        PackError::Cpio(e)
    }
}

/// POSIX permission bits from `std::fs::Metadata` (unix only).
#[cfg(unix)]
fn mode_bits(md: &fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    // Keep file-type bits + permission bits; mask to st_mode layout.
    (md.mode() & 0o170777) as u32
}

#[cfg(not(unix))]
fn mode_bits(md: &fs::Metadata) -> u32 {
    let _ = md;
    crate::cpio::mode::REG_0644
}

/// Recursively collect entries under `root` into `builder`.
/// Paths inside the archive are relative to `root`.
pub fn collect_tree(root: &Path, builder: &mut InitramfsBuilder) -> Result<usize, PackError> {
    if !root.is_dir() {
        return Err(PackError::NotADirectory(root.to_path_buf()));
    }
    let mut count = 0usize;
    collect_dir(root, root, builder, &mut count)?;
    Ok(count)
}

fn collect_dir(
    root: &Path,
    dir: &Path,
    builder: &mut InitramfsBuilder,
    count: &mut usize,
) -> Result<(), PackError> {
    // Deterministic order: sort directory entries by file name.
    let mut children: Vec<PathBuf> = fs::read_dir(dir)?.map(|e| e.unwrap().path()).collect();
    children.sort();
    for child in children {
        let rel = child.strip_prefix(root).unwrap();
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str.is_empty() {
            continue;
        }
        // symlink_metadata: do not follow links — symlinks are archived as links.
        let md = fs::symlink_metadata(&child)?;
        if md.file_type().is_symlink() {
            let target = fs::read_link(&child)?;
            builder.add(CpioEntry::symlink(
                rel_str,
                &target.to_string_lossy(),
            ))?;
        } else if md.is_dir() {
            builder.add(CpioEntry {
                path: rel_str.clone(),
                mode: mode_bits(&md),
                uid: 0,
                gid: 0,
                data: Vec::new(),
            })?;
            collect_dir(root, &child, builder, count)?;
        } else if md.is_file() {
            builder.add(CpioEntry {
                path: rel_str,
                mode: mode_bits(&md),
                uid: 0,
                gid: 0,
                data: fs::read(&child)?,
            })?;
        }
        *count += 1;
    }
    Ok(())
}

/// Pack `root` directory into an initramfs image written to `out_path`.
/// Returns the number of filesystem entries included.
pub fn pack_dir(root: &Path, out_path: &Path) -> Result<usize, PackError> {
    let mut builder = InitramfsBuilder::new();
    let count = collect_tree(root, &mut builder)?;
    let image = builder.build()?;
    fs::write(out_path, image)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("initramfs-test-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("etc")).unwrap();
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::write(dir.join("init"), "#!/bin/sh\necho hi\n").unwrap();
        fs::write(dir.join("etc/hostname"), "sigmaos\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/bin/busybox", dir.join("bin/sh")).unwrap();
        dir
    }

    #[test]
    fn pack_produces_valid_archive() {
        let root = fixture("a");
        let out = root.join("image.cpio");
        let n = pack_dir(&root, &out).unwrap();
        // init + etc dir + etc/hostname + bin dir + bin/sh symlink (+ maybe image itself excluded since written after scan)
        assert!(n >= 4);
        let img = fs::read(&out).unwrap();
        assert_eq!(&img[..6], b"070701");
        assert!(img.windows(10).any(|w| w == b"TRAILER!!!"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn paths_are_relative() {
        let root = fixture("b");
        let mut b = InitramfsBuilder::new();
        collect_tree(&root, &mut b).unwrap();
        let img = b.build().unwrap();
        // Archive paths must not contain the temp dir prefix.
        let s = String::from_utf8_lossy(&img);
        assert!(!s.contains("initramfs-test"));
        assert!(s.contains("etc/hostname"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn non_directory_fails() {
        let f = std::env::temp_dir().join("not-a-dir-file");
        fs::write(&f, b"x").unwrap();
        assert!(matches!(
            pack_dir(&f, &f.with_extension("out")),
            Err(PackError::NotADirectory(_))
        ));
        let _ = fs::remove_file(&f);
    }
}
