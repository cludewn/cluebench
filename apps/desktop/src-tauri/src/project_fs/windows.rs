// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{FsError, Identity, Snapshot, read_bounded};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::mem::{offset_of, size_of};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Component, Path, Prefix};
use std::ptr::{null, null_mut};
use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_CREATE, FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
    FILE_SYNCHRONOUS_IO_NONALERT, FileFsDeviceInformation, NtCreateFile,
    NtQueryVolumeInformationFile,
};
use windows_sys::Wdk::System::SystemServices::{FILE_FS_DEVICE_INFORMATION, FILE_REMOTE_DEVICE};
use windows_sys::Win32::Foundation as win;
use windows_sys::Win32::Foundation::{
    GetLastError, HANDLE, INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE, STATUS_OBJECT_NAME_NOT_FOUND,
    STATUS_OBJECT_PATH_NOT_FOUND, UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

pub(super) struct Parent {
    directory: File,
}

impl Parent {
    pub(super) fn at(path: &Path) -> Result<(Self, OsString), FsError> {
        let mut parts = path.components();
        let drive = match parts.next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::Disk(drive) | Prefix::VerbatimDisk(drive)
                    if drive.is_ascii_alphabetic() =>
                {
                    drive
                }
                _ => return Err(FsError::InvalidTarget),
            },
            _ => return Err(FsError::InvalidTarget),
        };
        if parts.next() != Some(Component::RootDir) {
            return Err(FsError::InvalidTarget);
        }
        let names: Vec<&OsStr> = parts
            .map(|part| match part {
                Component::Normal(name) => validate_name(name).map(|()| name),
                _ => Err(FsError::InvalidTarget),
            })
            .collect::<Result<_, _>>()?;
        let (name, parents) = names.split_last().ok_or(FsError::InvalidTarget)?;
        let root = [u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0];
        // Open the root once; subsequent traversal and all mutations are
        // relative to directory handles, never to a mutable absolute path.
        let handle = unsafe {
            CreateFileW(
                root.as_ptr(),
                FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(FsError::InvalidTarget);
        }
        // SAFETY: CreateFileW returned an owned non-invalid handle.
        let mut directory = unsafe { File::from_raw_handle(handle) };
        check_attributes(&directory, true)?;
        require_local_ntfs(&directory)?;
        for part in parents {
            directory =
                relative_open(&directory, part, true, false)?.ok_or(FsError::InvalidTarget)?;
            check_attributes(&directory, true)?;
            require_local_ntfs(&directory)?;
        }
        Ok((Self { directory }, (*name).to_os_string()))
    }

    pub(super) fn read(&self, name: &OsStr) -> Result<Option<Snapshot>, FsError> {
        relative_open(&self.directory, name, false, false)?
            .map(read_bounded)
            .transpose()
    }

    pub(super) fn create(&self, name: &OsStr) -> Result<File, FsError> {
        relative_open(&self.directory, name, false, true)?.ok_or(FsError::Io)
    }

    pub(super) fn install(
        &self,
        temp: &File,
        _temp_name: &OsStr,
        target: &OsStr,
        replace: bool,
    ) -> Result<(), FsError> {
        validate_name(target)?;
        let target: Vec<u16> = target.encode_wide().collect();
        let offset = offset_of!(FILE_RENAME_INFO, FileName);
        let bytes = (offset + target.len() * size_of::<u16>()).max(size_of::<FILE_RENAME_INFO>());
        // usize storage supplies sufficient alignment for FILE_RENAME_INFO.
        let mut buffer = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        // FILE_RENAME_REPLACE_IF_EXISTS and POSIX_SEMANTICS are documented
        // FileRenameInfoEx flags. Unsupported APIs fail without a fallback.
        let flags = if replace { 0x1 | 0x2 } else { 0 };
        // SAFETY: the aligned allocation covers the fixed header and all UTF-16
        // filename bytes, and both handles stay live throughout the call.
        let ok = unsafe {
            (*info).Anonymous.Flags = flags;
            (*info).RootDirectory = self.directory.as_raw_handle();
            (*info).FileNameLength = (target.len() * size_of::<u16>()) as u32;
            std::ptr::copy_nonoverlapping(
                target.as_ptr(),
                buffer.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
                target.len(),
            );
            SetFileInformationByHandle(
                temp.as_raw_handle(),
                FileRenameInfoEx,
                info.cast(),
                bytes as u32,
            )
        };
        if ok != 0 {
            return Ok(());
        }
        // SAFETY: immediate error query contains no sensitive path information.
        Err(classify_rename_error(unsafe { GetLastError() }))
    }

    pub(super) fn cleanup(&self, temp: &File, _name: &OsStr) -> Result<(), FsError> {
        let info = FILE_DISPOSITION_INFO { DeleteFile: true };
        // SAFETY: deletes only the object represented by our own temp handle.
        let ok = unsafe {
            SetFileInformationByHandle(
                temp.as_raw_handle(),
                FileDispositionInfo,
                (&info as *const FILE_DISPOSITION_INFO).cast(),
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        };
        if ok == 0 { Err(FsError::Io) } else { Ok(()) }
    }

    pub(super) fn sync(&self) -> Result<(), FsError> {
        // No supported directory flush contract is claimed for FileRenameInfoEx.
        // The new file was synced before the visible commit, but the directory
        // rename's power-loss durability remains explicitly uncertain.
        Err(FsError::Io)
    }
}

fn classify_rename_error(error: u32) -> FsError {
    // Only explicit collision, access, sharing, unsupported-operation, and
    // invalid-target errors establish a failed install. Unexpected storage,
    // removal, cancellation, or timeout results do not establish no commit.
    match error {
        win::ERROR_ALREADY_EXISTS | win::ERROR_FILE_EXISTS => FsError::ExternalChange,
        win::ERROR_ACCESS_DENIED
        | win::ERROR_SHARING_VIOLATION
        | win::ERROR_LOCK_VIOLATION
        | win::ERROR_DELETE_PENDING
        | win::ERROR_PRIVILEGE_NOT_HELD
        | win::ERROR_WRITE_PROTECT => FsError::Io,
        win::ERROR_INVALID_FUNCTION
        | win::ERROR_CALL_NOT_IMPLEMENTED
        | win::ERROR_NOT_SUPPORTED
        | win::ERROR_INVALID_PARAMETER
        | win::ERROR_NOT_SAME_DEVICE => FsError::UnsupportedFilesystem,
        win::ERROR_INVALID_NAME
        | win::ERROR_BAD_PATHNAME
        | win::ERROR_FILENAME_EXCED_RANGE
        | win::ERROR_PATH_NOT_FOUND
        | win::ERROR_FILE_NOT_FOUND
        | win::ERROR_DIR_NOT_EMPTY
        | win::ERROR_DIRECTORY
        | win::ERROR_CANNOT_MAKE => FsError::InvalidTarget,
        _ => FsError::OutcomeUnknown,
    }
}

fn validate_name(name: &OsStr) -> Result<(), FsError> {
    let units: Vec<u16> = name.encode_wide().collect();
    if units.is_empty()
        || units.len() > 255
        || units
            .iter()
            .any(|c| *c < 32 || b":\\/*?\"<>|".iter().any(|bad| *c == u16::from(*bad)))
        || matches!(units.last(), Some(32 | 46))
    {
        return Err(FsError::InvalidTarget);
    }
    let base = name
        .to_string_lossy()
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&base.as_str())
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && base.as_bytes()[3].is_ascii_digit())
    {
        return Err(FsError::InvalidTarget);
    }
    Ok(())
}

fn relative_open(
    parent: &File,
    name: &OsStr,
    directory: bool,
    create: bool,
) -> Result<Option<File>, FsError> {
    validate_name(name)?;
    let mut name: Vec<u16> = name.encode_wide().collect();
    let name = UNICODE_STRING {
        Length: (name.len() * size_of::<u16>()) as u16,
        MaximumLength: (name.len() * size_of::<u16>()) as u16,
        Buffer: name.as_mut_ptr(),
    };
    let attrs = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &name,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: null(),
        SecurityQualityOfService: null(),
    };
    let mut handle: HANDLE = null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    let access = FILE_READ_ATTRIBUTES
        | SYNCHRONIZE
        | if directory {
            FILE_LIST_DIRECTORY | FILE_TRAVERSE
        } else if create {
            FILE_READ_DATA | FILE_WRITE_DATA | DELETE
        } else {
            FILE_READ_DATA
        };
    let options = FILE_OPEN_REPARSE_POINT
        | FILE_SYNCHRONOUS_IO_NONALERT
        | if directory {
            FILE_DIRECTORY_FILE
        } else {
            FILE_NON_DIRECTORY_FILE
        };
    // SAFETY: names are a single checked component. All pointers refer to live
    // buffers; synchronous NtCreateFile returns an owned handle on success.
    let share = if create {
        FILE_SHARE_READ
    } else {
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
    };
    let result = unsafe {
        NtCreateFile(
            &mut handle,
            access,
            &attrs,
            &mut status,
            null(),
            FILE_ATTRIBUTE_NORMAL,
            share,
            if create { FILE_CREATE } else { FILE_OPEN },
            options,
            null(),
            0,
        )
    };
    if result == STATUS_OBJECT_NAME_NOT_FOUND || result == STATUS_OBJECT_PATH_NOT_FOUND {
        return Ok(None);
    }
    if result < 0 {
        return Err(FsError::Io);
    }
    // SAFETY: successful NtCreateFile transfers this handle to our File.
    let file = unsafe { File::from_raw_handle(handle) };
    check_attributes(&file, directory)?;
    Ok(Some(file))
}

fn check_attributes(file: &File, directory: bool) -> Result<(), FsError> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: writable structure is valid for the live file handle.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(FsError::Io);
    }
    let invalid = FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DEVICE;
    if info.dwFileAttributes & invalid != 0
        || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
    {
        return Err(FsError::InvalidTarget);
    }
    // SAFETY: GetFileType accepts any live file handle.
    if unsafe { GetFileType(file.as_raw_handle()) } != FILE_TYPE_DISK {
        return Err(FsError::InvalidTarget);
    }
    Ok(())
}

pub(super) fn identity(file: &File) -> Result<Identity, FsError> {
    check_attributes(file, false)?;
    let mut info = FILE_ID_INFO::default();
    // SAFETY: structure and its exact byte size match FileIdInfo.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(FsError::Io);
    }
    Ok(Identity {
        volume: info.VolumeSerialNumber,
        high: u64::from_le_bytes(
            info.FileId.Identifier[..8]
                .try_into()
                .map_err(|_| FsError::Io)?,
        ),
        low: u64::from_le_bytes(
            info.FileId.Identifier[8..]
                .try_into()
                .map_err(|_| FsError::Io)?,
        ),
    })
}

fn require_local_ntfs(directory: &File) -> Result<(), FsError> {
    let mut status = IO_STATUS_BLOCK::default();
    let mut device = FILE_FS_DEVICE_INFORMATION::default();
    // SAFETY: the native API writes exactly the documented device-info layout.
    let result = unsafe {
        NtQueryVolumeInformationFile(
            directory.as_raw_handle(),
            &mut status,
            (&mut device as *mut FILE_FS_DEVICE_INFORMATION).cast(),
            size_of::<FILE_FS_DEVICE_INFORMATION>() as u32,
            FileFsDeviceInformation,
        )
    };
    if result < 0
        || device.Characteristics & FILE_REMOTE_DEVICE != 0
        || device.DeviceType != FILE_DEVICE_DISK
    {
        return Err(FsError::UnsupportedFilesystem);
    }
    let mut filesystem = [0u16; 32];
    // SAFETY: optional outputs are null; filesystem output has its stated size.
    let ok = unsafe {
        GetVolumeInformationByHandleW(
            directory.as_raw_handle(),
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        )
    };
    let end = filesystem
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(filesystem.len());
    if ok == 0
        || filesystem[..end]
            != [
                u16::from(b'N'),
                u16::from(b'T'),
                u16::from(b'F'),
                u16::from(b'S'),
            ]
    {
        return Err(FsError::UnsupportedFilesystem);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_failure_classifier_only_marks_known_precommit_results() {
        for error in [win::ERROR_ALREADY_EXISTS, win::ERROR_FILE_EXISTS] {
            assert_eq!(classify_rename_error(error), FsError::ExternalChange);
        }
        for error in [
            win::ERROR_ACCESS_DENIED,
            win::ERROR_SHARING_VIOLATION,
            win::ERROR_PRIVILEGE_NOT_HELD,
        ] {
            assert_eq!(classify_rename_error(error), FsError::Io);
        }
        for error in [win::ERROR_NOT_SUPPORTED, win::ERROR_INVALID_PARAMETER] {
            assert_eq!(classify_rename_error(error), FsError::UnsupportedFilesystem);
        }
        assert_eq!(
            classify_rename_error(win::ERROR_INVALID_NAME),
            FsError::InvalidTarget
        );
        for error in [
            win::ERROR_IO_DEVICE,
            win::ERROR_DEVICE_NOT_CONNECTED,
            win::ERROR_TIMEOUT,
            win::ERROR_OPERATION_ABORTED,
            win::ERROR_FILE_INVALID,
            win::ERROR_GEN_FAILURE,
            win::ERROR_UNEXP_NET_ERR,
            u32::MAX,
        ] {
            assert_eq!(classify_rename_error(error), FsError::OutcomeUnknown);
        }
    }
}
