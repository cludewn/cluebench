// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{FsError, Identity, Snapshot};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::path::Path;

pub(super) struct Parent;
impl Parent {
    pub(super) fn at(_: &Path) -> Result<(Self, OsString), FsError> {
        Err(FsError::UnsupportedFilesystem)
    }
    pub(super) fn read(&self, _: &OsStr) -> Result<Option<Snapshot>, FsError> {
        Err(FsError::UnsupportedFilesystem)
    }
    pub(super) fn create(&self, _: &OsStr) -> Result<File, FsError> {
        Err(FsError::UnsupportedFilesystem)
    }
    pub(super) fn install(&self, _: &File, _: &OsStr, _: &OsStr, _: bool) -> Result<(), FsError> {
        Err(FsError::UnsupportedFilesystem)
    }
    pub(super) fn cleanup(&self, _: &File, _: &OsStr) -> Result<(), FsError> {
        Err(FsError::UnsupportedFilesystem)
    }
    pub(super) fn sync(&self) -> Result<(), FsError> {
        Err(FsError::UnsupportedFilesystem)
    }
}
pub(super) fn identity(_: &File) -> Result<Identity, FsError> {
    Err(FsError::UnsupportedFilesystem)
}
