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
    FILE_RENAME_INFORMATION, FILE_RENAME_POSIX_SEMANTICS, FILE_RENAME_REPLACE_IF_EXISTS,
    FILE_SYNCHRONOUS_IO_NONALERT, FileFsDeviceInformation, FileRenameInformationEx, NtCreateFile,
    NtQueryVolumeInformationFile, NtSetInformationFile,
};
use windows_sys::Wdk::System::SystemServices::{FILE_FS_DEVICE_INFORMATION, FILE_REMOTE_DEVICE};
use windows_sys::Win32::Foundation as win;
use windows_sys::Win32::Foundation::{
    HANDLE, INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE, STATUS_OBJECT_NAME_NOT_FOUND,
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
        let (status, completion) = rename_relative(temp, &self.directory, target, replace)?;
        rename_result(status, completion)
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
        // No supported directory flush contract is claimed for the NT rename.
        // The new file was synced before the visible commit, but the directory
        // rename's power-loss durability remains explicitly uncertain.
        Err(FsError::Io)
    }
}

fn rename_buffer(
    parent: HANDLE,
    target: &OsStr,
    replace: bool,
) -> Result<(Vec<usize>, u32), FsError> {
    validate_name(target)?;
    let target: Vec<u16> = target.encode_wide().collect();
    let offset = offset_of!(FILE_RENAME_INFORMATION, FileName);
    let bytes =
        (offset + target.len() * size_of::<u16>()).max(size_of::<FILE_RENAME_INFORMATION>());
    // usize storage supplies the native structure's pointer alignment. Only
    // the byte length below is passed; no trailing NUL belongs to FileName.
    let mut buffer = vec![0usize; bytes.div_ceil(size_of::<usize>())];
    let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
    // SAFETY: the aligned allocation covers the header and all UTF-16 bytes.
    unsafe {
        (*info).Anonymous.Flags = if replace {
            FILE_RENAME_REPLACE_IF_EXISTS | FILE_RENAME_POSIX_SEMANTICS
        } else {
            0
        };
        (*info).RootDirectory = parent;
        (*info).FileNameLength = (target.len() * size_of::<u16>()) as u32;
        std::ptr::copy_nonoverlapping(
            target.as_ptr(),
            buffer.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
            target.len(),
        );
    }
    Ok((buffer, bytes as u32))
}

fn rename_relative(
    temp: &File,
    parent: &File,
    target: &OsStr,
    replace: bool,
) -> Result<(win::NTSTATUS, win::NTSTATUS), FsError> {
    let (buffer, bytes) = rename_buffer(parent.as_raw_handle(), target, replace)?;
    let mut completion = IO_STATUS_BLOCK::default();
    // Call the native user-mode API directly: the Win32 rename wrapper can
    // reject a non-null RootDirectory with a relative name.
    // SAFETY: our temp handles use FILE_SYNCHRONOUS_IO_NONALERT, so the call
    // completes before returning. Both handles, IOSB, and the aligned buffer
    // remain live throughout it. Class 65 performs normal access checks.
    let status = unsafe {
        NtSetInformationFile(
            temp.as_raw_handle(),
            &mut completion,
            buffer.as_ptr().cast(),
            bytes,
            FileRenameInformationEx,
        )
    };
    // SAFETY: Status is the NTSTATUS member of the native IO_STATUS_BLOCK.
    Ok((status, unsafe { completion.Anonymous.Status }))
}

fn rename_result(status: win::NTSTATUS, completion: win::NTSTATUS) -> Result<(), FsError> {
    if status == win::STATUS_SUCCESS && completion == win::STATUS_SUCCESS {
        Ok(())
    } else if status < 0 {
        Err(classify_rename_status(status))
    } else {
        // Pending, warning, informational, and inconsistent completion results
        // do not establish a completed commit, even when NT_SUCCESS is true.
        Err(FsError::OutcomeUnknown)
    }
}

fn classify_rename_status(status: win::NTSTATUS) -> FsError {
    // Only explicit collision, access, sharing, unsupported-operation, and
    // invalid-target errors establish a failed install. Unexpected storage,
    // removal, cancellation, or timeout results do not establish no commit.
    match status {
        win::STATUS_OBJECT_NAME_COLLISION => FsError::ExternalChange,
        win::STATUS_ACCESS_DENIED
        | win::STATUS_SHARING_VIOLATION
        | win::STATUS_FILE_LOCK_CONFLICT
        | win::STATUS_LOCK_NOT_GRANTED
        | win::STATUS_DELETE_PENDING
        | win::STATUS_PRIVILEGE_NOT_HELD
        | win::STATUS_MEDIA_WRITE_PROTECTED
        | win::STATUS_CANNOT_DELETE => FsError::Io,
        win::STATUS_NOT_IMPLEMENTED
        | win::STATUS_INVALID_DEVICE_REQUEST
        | win::STATUS_INVALID_INFO_CLASS
        | win::STATUS_NOT_SUPPORTED
        | win::STATUS_INVALID_PARAMETER
        | win::STATUS_NOT_SAME_DEVICE => FsError::UnsupportedFilesystem,
        win::STATUS_OBJECT_NAME_INVALID
        | win::STATUS_OBJECT_PATH_INVALID
        | win::STATUS_OBJECT_PATH_SYNTAX_BAD
        | win::STATUS_NAME_TOO_LONG
        | win::STATUS_OBJECT_PATH_NOT_FOUND
        | win::STATUS_OBJECT_NAME_NOT_FOUND
        | win::STATUS_DIRECTORY_NOT_EMPTY
        | win::STATUS_NOT_A_DIRECTORY
        | win::STATUS_FILE_IS_A_DIRECTORY => FsError::InvalidTarget,
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

    #[cfg(target_env = "msvc")]
    #[test]
    fn project_test_process_activates_common_controls_v6() {
        use windows_sys::Win32::Foundation::FreeLibrary;
        use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
        use windows_sys::core::{s, w};

        // Check the actual process activation context without opening a dialog.
        // A missing test manifest selects v5, which lacks TaskDialogIndirect.
        // SAFETY: both names are static, terminated strings. The loaded handle
        // stays live until the symbol lookup finishes; the symbol is not called.
        let module = unsafe { LoadLibraryW(w!("comctl32.dll")) };
        assert!(!module.is_null(), "Common Controls must load");
        let available = unsafe { GetProcAddress(module, s!("TaskDialogIndirect")) }.is_some();
        let released = unsafe { FreeLibrary(module) };
        assert!(
            available,
            "The test process must activate Common Controls v6"
        );
        assert_ne!(released, 0, "The test must release its module reference");
    }

    #[test]
    fn native_rename_buffer_uses_the_documented_layout_and_utf16_byte_length() {
        let pointer_bytes = size_of::<usize>();
        assert_eq!(
            offset_of!(FILE_RENAME_INFORMATION, RootDirectory),
            pointer_bytes
        );
        assert_eq!(
            offset_of!(FILE_RENAME_INFORMATION, FileNameLength),
            pointer_bytes * 2
        );
        assert_eq!(
            offset_of!(FILE_RENAME_INFORMATION, FileName),
            pointer_bytes * 2 + 4
        );
        assert_eq!(
            std::mem::align_of::<FILE_RENAME_INFORMATION>(),
            pointer_bytes
        );
        let parent = std::ptr::without_provenance_mut(0x40);
        let name = OsStr::new("synthetic-🧩.age");
        let encoded: Vec<u16> = name.encode_wide().collect();
        for replace in [false, true] {
            let (buffer, bytes) = rename_buffer(parent, name, replace).unwrap();
            assert_eq!(
                buffer.as_ptr() as usize % std::mem::align_of::<FILE_RENAME_INFORMATION>(),
                0
            );
            assert_eq!(
                bytes as usize,
                (offset_of!(FILE_RENAME_INFORMATION, FileName) + encoded.len() * 2)
                    .max(size_of::<FILE_RENAME_INFORMATION>())
            );
            // SAFETY: rename_buffer supplies an aligned allocation covering
            // the native header and its complete variable-length name.
            unsafe {
                let info = &*buffer.as_ptr().cast::<FILE_RENAME_INFORMATION>();
                assert_eq!(info.RootDirectory, parent);
                assert_eq!(info.FileNameLength as usize, encoded.len() * 2);
                assert_eq!(
                    info.Anonymous.Flags,
                    if replace {
                        FILE_RENAME_REPLACE_IF_EXISTS | FILE_RENAME_POSIX_SEMANTICS
                    } else {
                        0
                    }
                );
                let actual = std::slice::from_raw_parts(
                    buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset_of!(FILE_RENAME_INFORMATION, FileName))
                        .cast::<u16>(),
                    encoded.len(),
                );
                assert_eq!(actual, encoded);
            }
        }
        for invalid in [
            "",
            "target.age:stream",
            r"subdir\target.age",
            "../target.age",
        ] {
            assert_eq!(
                rename_buffer(parent, OsStr::new(invalid), false).err(),
                Some(FsError::InvalidTarget)
            );
        }
    }

    #[test]
    fn native_relative_rename_creates_refuses_collision_and_replaces_without_losing_old_handles() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("synthetic.age");
        let (parent, name) = Parent::at(&target).unwrap();
        let mut first = parent.create(OsStr::new("first.tmp")).unwrap();
        first.write_all(b"old synthetic ciphertext").unwrap();
        first.sync_all().unwrap();
        let created = rename_relative(&first, &parent.directory, &name, false).unwrap();
        assert_eq!(
            created,
            (win::STATUS_SUCCESS, win::STATUS_SUCCESS),
            "Native new rename NTSTATUS / IOSB"
        );
        assert_eq!(rename_result(created.0, created.1), Ok(()));
        assert_eq!(std::fs::read(&target).unwrap(), b"old synthetic ciphertext");
        // Keep the old object open with delete sharing; replacement must leave
        // this handle on the old object and install the new object at the name.
        drop(first);
        let mut old = std::fs::File::open(&target).unwrap();
        let mut next = parent.create(OsStr::new("next.tmp")).unwrap();
        next.write_all(b"new synthetic ciphertext").unwrap();
        next.sync_all().unwrap();
        let collision = rename_relative(&next, &parent.directory, &name, false).unwrap();
        assert_eq!(
            collision.0,
            win::STATUS_OBJECT_NAME_COLLISION,
            "Native no-replace rename NTSTATUS"
        );
        assert_eq!(
            rename_result(collision.0, collision.1),
            Err(FsError::ExternalChange)
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"old synthetic ciphertext");
        assert_eq!(
            std::fs::read(dir.path().join("next.tmp")).unwrap(),
            b"new synthetic ciphertext"
        );
        let replaced = rename_relative(&next, &parent.directory, &name, true).unwrap();
        assert_eq!(
            replaced,
            (win::STATUS_SUCCESS, win::STATUS_SUCCESS),
            "Native replacement NTSTATUS / IOSB"
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"new synthetic ciphertext");
        let mut retained = Vec::new();
        std::io::Read::read_to_end(&mut old, &mut retained).unwrap();
        assert_eq!(retained, b"old synthetic ciphertext");
        assert!(!dir.path().join("next.tmp").exists());
    }

    #[test]
    fn native_rename_failure_preserves_an_existing_directory_and_source() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("existing");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(
            target.join("synthetic.age"),
            b"existing synthetic ciphertext",
        )
        .unwrap();
        let (parent, name) = Parent::at(&target).unwrap();
        let mut source = parent.create(OsStr::new("source.tmp")).unwrap();
        source
            .write_all(b"replacement synthetic ciphertext")
            .unwrap();
        source.sync_all().unwrap();
        let failed = rename_relative(&source, &parent.directory, &name, true).unwrap();
        assert!(
            failed.0 < 0,
            "Replacing a directory with a file must fail, NTSTATUS={:#010x}",
            failed.0
        );
        assert!(
            matches!(
                rename_result(failed.0, failed.1),
                Err(FsError::InvalidTarget | FsError::ExternalChange | FsError::Io)
            ),
            "Native directory replacement NTSTATUS / IOSB = {:#010x} / {:#010x}",
            failed.0,
            failed.1
        );
        assert_eq!(
            std::fs::read(target.join("synthetic.age")).unwrap(),
            b"existing synthetic ciphertext"
        );
        assert_eq!(
            std::fs::read(dir.path().join("source.tmp")).unwrap(),
            b"replacement synthetic ciphertext"
        );
    }

    #[test]
    fn rename_status_classifier_only_marks_known_precommit_results() {
        assert_eq!(
            rename_result(win::STATUS_SUCCESS, win::STATUS_SUCCESS),
            Ok(())
        );
        assert_eq!(
            classify_rename_status(win::STATUS_OBJECT_NAME_COLLISION),
            FsError::ExternalChange
        );
        for status in [
            win::STATUS_ACCESS_DENIED,
            win::STATUS_SHARING_VIOLATION,
            win::STATUS_PRIVILEGE_NOT_HELD,
            win::STATUS_CANNOT_DELETE,
        ] {
            assert_eq!(classify_rename_status(status), FsError::Io);
        }
        for status in [
            win::STATUS_NOT_SUPPORTED,
            win::STATUS_INVALID_PARAMETER,
            win::STATUS_INVALID_INFO_CLASS,
        ] {
            assert_eq!(
                classify_rename_status(status),
                FsError::UnsupportedFilesystem
            );
        }
        assert_eq!(
            classify_rename_status(win::STATUS_OBJECT_NAME_INVALID),
            FsError::InvalidTarget
        );
        for status in [
            win::STATUS_IO_DEVICE_ERROR,
            win::STATUS_DEVICE_NOT_CONNECTED,
            win::STATUS_IO_TIMEOUT,
            win::STATUS_CANCELLED,
            win::STATUS_FILE_INVALID,
            win::STATUS_DISK_FULL,
            win::STATUS_UNSUCCESSFUL,
            win::STATUS_PENDING,
            win::STATUS_TIMEOUT,
            win::STATUS_OBJECT_NAME_EXISTS,
            win::STATUS_BUFFER_OVERFLOW,
            -1,
        ] {
            assert_eq!(classify_rename_status(status), FsError::OutcomeUnknown);
            assert_eq!(rename_result(status, status), Err(FsError::OutcomeUnknown));
        }
        for completion in [
            win::STATUS_PENDING,
            win::STATUS_ACCESS_DENIED,
            win::STATUS_IO_DEVICE_ERROR,
        ] {
            assert_eq!(
                rename_result(win::STATUS_SUCCESS, completion),
                Err(FsError::OutcomeUnknown)
            );
        }
    }
}
