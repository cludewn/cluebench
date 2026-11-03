// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{FsError, Identity, Snapshot, read_bounded};
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, RenameFlags, fstat, fstatfs, open, openat, renameat_with,
    unlinkat,
};
use rustix::io::Errno;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

pub(super) struct Parent {
    directory: File,
}

impl Parent {
    pub(super) fn at(path: &Path) -> Result<(Self, OsString), FsError> {
        if !path.is_absolute() {
            return Err(FsError::InvalidTarget);
        }
        let name = path
            .file_name()
            .ok_or(FsError::InvalidTarget)?
            .to_os_string();
        let parent = path.parent().ok_or(FsError::InvalidTarget)?;
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mut directory = File::from(open("/", flags, Mode::empty()).map_err(|_| FsError::Io)?);
        for component in parent.components() {
            match component {
                Component::RootDir => (),
                Component::Normal(name) => {
                    directory = File::from(
                        openat(&directory, name, flags, Mode::empty())
                            .map_err(|_| FsError::InvalidTarget)?,
                    )
                }
                _ => return Err(FsError::InvalidTarget),
            }
        }
        require_ext4(&directory)?;
        require_protected_parent(&directory)?;
        Ok((Self { directory }, name))
    }

    pub(super) fn read(&self, name: &OsStr) -> Result<Option<Snapshot>, FsError> {
        require_protected_parent(&self.directory)?;
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
        match openat(&self.directory, name, flags, Mode::empty()) {
            Ok(fd) => {
                let file = File::from(fd);
                // A mounted file must not smuggle an unsupported filesystem
                // beneath an otherwise supported pinned parent directory.
                if identity(&file)?.volume
                    != fstat(&self.directory).map_err(|_| FsError::Io)?.st_dev
                {
                    return Err(FsError::UnsupportedFilesystem);
                }
                read_bounded(file).map(Some)
            }
            Err(Errno::NOENT) => Ok(None),
            Err(Errno::LOOP | Errno::ISDIR | Errno::NOTDIR) => Err(FsError::InvalidTarget),
            Err(_) => Err(FsError::Io),
        }
    }

    pub(super) fn create(&self, name: &OsStr) -> Result<File, FsError> {
        require_protected_parent(&self.directory)?;
        let flags =
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let file = File::from(
            openat(&self.directory, name, flags, Mode::RUSR | Mode::WUSR)
                .map_err(|_| FsError::Io)?,
        );
        identity(&file)?;
        Ok(file)
    }

    pub(super) fn install(
        &self,
        temp: &File,
        temp_name: &OsStr,
        target: &OsStr,
        replace: bool,
    ) -> Result<(), FsError> {
        require_protected_parent(&self.directory)?;
        if !self.read(temp_name)?.is_some_and(|current| {
            identity(temp).is_ok_and(|identity| identity == current.identity)
        }) {
            return Err(FsError::ExternalChange);
        }
        let flags = if replace {
            RenameFlags::empty()
        } else {
            RenameFlags::NOREPLACE
        };
        renameat_with(&self.directory, temp_name, &self.directory, target, flags).map_err(|error| {
            match error {
                Errno::EXIST => FsError::ExternalChange,
                Errno::IO => FsError::OutcomeUnknown,
                Errno::NOSYS | Errno::OPNOTSUPP | Errno::INVAL => FsError::UnsupportedFilesystem,
                _ => FsError::Io,
            }
        })
    }

    pub(super) fn cleanup(&self, temp: &File, name: &OsStr) -> Result<(), FsError> {
        require_protected_parent(&self.directory)?;
        if let Some(current) = self.read(name)? {
            if current.identity != identity(temp)? {
                return Err(FsError::ExternalChange);
            }
            unlinkat(&self.directory, name, AtFlags::empty()).map_err(|_| FsError::Io)?;
        }
        Ok(())
    }

    pub(super) fn sync(&self) -> Result<(), FsError> {
        self.directory.sync_all().map_err(|_| FsError::Io)
    }
}

fn require_protected_parent(directory: &File) -> Result<(), FsError> {
    let metadata = fstat(directory).map_err(|_| FsError::Io)?;
    let current_uid = rustix::process::geteuid().as_raw();
    // Only this principal and the system administrator are trusted to mutate
    // the pinned directory's owner/mode. The group mode bits include the POSIX
    // ACL effective mask, so a nonsticky named-user write grant fails closed.
    // A trusted-owner sticky directory protects our owned temp entry against
    // removal or substitution by other principals, even when creation is shared.
    let trusted_owner = metadata.st_uid == current_uid || metadata.st_uid == 0;
    let shared_write = metadata.st_mode & (Mode::WGRP | Mode::WOTH).bits() != 0;
    let sticky = metadata.st_mode & Mode::SVTX.bits() != 0;
    if !trusted_owner || (shared_write && !sticky) {
        return Err(FsError::InvalidTarget);
    }
    Ok(())
}

pub(super) fn identity(file: &File) -> Result<Identity, FsError> {
    let metadata = fstat(file).map_err(|_| FsError::Io)?;
    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
        return Err(FsError::InvalidTarget);
    }
    Ok(Identity {
        volume: metadata.st_dev,
        high: 0,
        low: metadata.st_ino,
    })
}

fn require_ext4(directory: &File) -> Result<(), FsError> {
    // ext2/3 share the superblock magic, so require the ext4 driver in this
    // mount namespace as well. Network and unrecognised filesystems fail closed.
    if fstatfs(directory).map_err(|_| FsError::Io)?.f_type as u64 != 0xef53 {
        return Err(FsError::UnsupportedFilesystem);
    }
    let device = fstat(directory).map_err(|_| FsError::Io)?.st_dev;
    let device = format!(
        "{}:{}",
        rustix::fs::major(device),
        rustix::fs::minor(device)
    );
    let mut mounts = Vec::new();
    File::open("/proc/self/mountinfo")
        .map_err(|_| FsError::UnsupportedFilesystem)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut mounts)
        .map_err(|_| FsError::UnsupportedFilesystem)?;
    if mounts.len() > 1024 * 1024 {
        return Err(FsError::UnsupportedFilesystem);
    }
    let mounts = std::str::from_utf8(&mounts).map_err(|_| FsError::UnsupportedFilesystem)?;
    let supported = mounts.lines().any(|line| {
        let mut parts = line.split_ascii_whitespace();
        let dev = parts.nth(2);
        dev == Some(device.as_str())
            && line
                .split_once(" - ")
                .is_some_and(|(_, tail)| tail.split_ascii_whitespace().next() == Some("ext4"))
    });
    if supported {
        Ok(())
    } else {
        Err(FsError::UnsupportedFilesystem)
    }
}
