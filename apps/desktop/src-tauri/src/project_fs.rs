// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Ciphertext-only, handle-relative project file operations.
//!
//! A binding pins its parent directory. Comparisons detect observed external
//! changes; they are not a compare-and-swap guarantee against a hostile writer.

use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[cfg(target_os = "linux")]
#[path = "project_fs/linux.rs"]
mod platform;
#[cfg(windows)]
#[path = "project_fs/windows.rs"]
mod platform;
#[cfg(not(any(target_os = "linux", windows)))]
#[path = "project_fs/unsupported.rs"]
mod platform;

pub(crate) const MAX_FILE_BYTES: usize = 384 * 1024;

/// Deliberately contains no paths, OS diagnostics, or project contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FsError {
    UnsupportedFilesystem,
    InvalidTarget,
    AlreadyExists,
    TooLarge,
    ExternalChange,
    Io,
    OutcomeUnknown,
}

impl FsError {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::UnsupportedFilesystem => "PROJECT_UNSUPPORTED_FILESYSTEM",
            Self::InvalidTarget => "PROJECT_UNSAFE_DESTINATION",
            Self::AlreadyExists => "PROJECT_DESTINATION_EXISTS",
            Self::TooLarge => "PROJECT_BUDGET_EXCEEDED",
            Self::ExternalChange => "PROJECT_EXTERNAL_CHANGE",
            Self::Io => "PROJECT_IO",
            Self::OutcomeUnknown => "PROJECT_OUTCOME_UNKNOWN",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WriteOutcome {
    NotCommitted(FsError),
    Saved,
    CommittedDurabilityUncertain,
    OutcomeUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    volume: u64,
    high: u64,
    low: u64,
}

struct Snapshot {
    identity: Identity,
    ciphertext: Vec<u8>,
}

pub(crate) struct FileBinding {
    parent: platform::Parent,
    name: OsString,
    expected: Option<Snapshot>,
    poisoned: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Write,
    Flush,
    FileSync,
    Check,
    Rename,
    AfterRename,
    DirectorySync,
    Cleanup,
}

trait Faults {
    fn check(&self, _step: Step) -> Result<(), FsError> {
        Ok(())
    }
}
struct NoFaults;
impl Faults for NoFaults {}

impl FileBinding {
    pub(crate) fn new(path: &Path) -> Result<Self, FsError> {
        let (parent, name) = platform::Parent::at(path)?;
        if parent.read(&name)?.is_some() {
            return Err(FsError::AlreadyExists);
        }
        Ok(Self {
            parent,
            name,
            expected: None,
            poisoned: false,
        })
    }

    pub(crate) fn open(path: &Path) -> Result<(Self, Vec<u8>), FsError> {
        let (parent, name) = platform::Parent::at(path)?;
        let snapshot = parent.read(&name)?.ok_or(FsError::InvalidTarget)?;
        let ciphertext = snapshot.ciphertext.clone();
        Ok((
            Self {
                parent,
                name,
                expected: Some(snapshot),
                poisoned: false,
            },
            ciphertext,
        ))
    }

    pub(crate) fn read_unchanged(&self) -> Result<Vec<u8>, FsError> {
        if self.poisoned {
            return Err(FsError::OutcomeUnknown);
        }
        let current = self.parent.read(&self.name)?;
        match (&self.expected, current) {
            (Some(expected), Some(current))
                if expected.identity == current.identity
                    && expected.ciphertext == current.ciphertext =>
            {
                Ok(current.ciphertext)
            }
            _ => Err(FsError::ExternalChange),
        }
    }

    fn check_unchanged(&self) -> Result<(), FsError> {
        if self.poisoned {
            return Err(FsError::OutcomeUnknown);
        }
        match (&self.expected, self.parent.read(&self.name)?) {
            (None, None) => Ok(()),
            (Some(expected), Some(current))
                if expected.identity == current.identity
                    && expected.ciphertext == current.ciphertext =>
            {
                Ok(())
            }
            _ => Err(FsError::ExternalChange),
        }
    }

    pub(crate) fn write(&mut self, ciphertext: &[u8]) -> WriteOutcome {
        self.write_with(ciphertext, &NoFaults)
    }

    fn write_with(&mut self, ciphertext: &[u8], faults: &impl Faults) -> WriteOutcome {
        if self.poisoned {
            return WriteOutcome::OutcomeUnknown;
        }
        if ciphertext.len() > MAX_FILE_BYTES {
            return WriteOutcome::NotCommitted(FsError::TooLarge);
        }
        if let Err(error) = self.check_unchanged() {
            return WriteOutcome::NotCommitted(error);
        }
        let temp_name = OsString::from(format!(".cluebench-{}.tmp", uuid::Uuid::new_v4()));
        let mut temp = match self.parent.create(&temp_name) {
            Ok(temp) => temp,
            Err(error) => return WriteOutcome::NotCommitted(error),
        };
        let before_commit = (|| {
            faults.check(Step::Write)?;
            temp.write_all(ciphertext).map_err(|_| FsError::Io)?;
            faults.check(Step::Flush)?;
            temp.flush().map_err(|_| FsError::Io)?;
            faults.check(Step::FileSync)?;
            temp.sync_all().map_err(|_| FsError::Io)?;
            let identity = platform::identity(&temp)?;
            faults.check(Step::Check)?;
            self.check_unchanged()?;
            faults.check(Step::Rename)?;
            Ok(identity)
        })();
        let identity = match before_commit {
            Ok(identity) => identity,
            Err(error) => {
                // Cleanup may fail. Only our still-identical temp is eligible;
                // a leftover is ciphertext, never a backup or plaintext draft.
                if faults.check(Step::Cleanup).is_ok() {
                    let _ = self.parent.cleanup(&temp, &temp_name);
                }
                return WriteOutcome::NotCommitted(error);
            }
        };
        if let Err(error) =
            self.parent
                .install(&temp, &temp_name, &self.name, self.expected.is_some())
        {
            // On supported local filesystems an ordinary failed rename has no
            // commit. EIO/ambiguous native results are conservatively unknown.
            if error == FsError::OutcomeUnknown {
                self.poisoned = true;
                return WriteOutcome::OutcomeUnknown;
            }
            if faults.check(Step::Cleanup).is_ok() {
                let _ = self.parent.cleanup(&temp, &temp_name);
            }
            return WriteOutcome::NotCommitted(error);
        }
        // The rename is the commit point. Do not roll back after this point.
        self.expected = Some(Snapshot {
            identity,
            ciphertext: ciphertext.to_vec(),
        });
        if faults.check(Step::AfterRename).is_err() {
            self.poisoned = true;
            return WriteOutcome::OutcomeUnknown;
        }
        if faults.check(Step::DirectorySync).is_err() || self.parent.sync().is_err() {
            return WriteOutcome::CommittedDurabilityUncertain;
        }
        WriteOutcome::Saved
    }
}

fn read_bounded(mut file: File) -> Result<Snapshot, FsError> {
    let identity = platform::identity(&file)?;
    let before = file.metadata().map_err(|_| FsError::Io)?;
    if before.len() > MAX_FILE_BYTES as u64 {
        return Err(FsError::TooLarge);
    }
    let mut ciphertext = Vec::with_capacity((before.len() as usize).min(MAX_FILE_BYTES));
    Read::by_ref(&mut file)
        .take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut ciphertext)
        .map_err(|_| FsError::Io)?;
    if ciphertext.len() > MAX_FILE_BYTES {
        return Err(FsError::TooLarge);
    }
    let after = file.metadata().map_err(|_| FsError::Io)?;
    if identity != platform::identity(&file)?
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(FsError::ExternalChange);
    }
    Ok(Snapshot {
        identity,
        ciphertext,
    })
}

#[cfg(test)]
#[path = "project_fs/tests.rs"]
mod tests;
