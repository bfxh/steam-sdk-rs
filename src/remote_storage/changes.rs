//! What changed locally since the last sync: the two vocabularies Steam answers with and the
//! one record that bundles them.

use super::*;

/// How one cloud file changed locally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalFileChangeKind {
    Invalid,
    Updated,
    Deleted,
}

impl From<sys::ERemoteStorageLocalFileChange> for LocalFileChangeKind {
    fn from(raw: sys::ERemoteStorageLocalFileChange) -> Self {
        match raw {
            sys::ERemoteStorageLocalFileChange::k_ERemoteStorageLocalFileChange_FileUpdated => {
                LocalFileChangeKind::Updated
            }
            sys::ERemoteStorageLocalFileChange::k_ERemoteStorageLocalFileChange_FileDeleted => {
                LocalFileChangeKind::Deleted
            }
            // The binding is `#[non_exhaustive]`, so a value this crate was not
            // built against can arrive; it maps to Invalid instead of panicking.
            _ => LocalFileChangeKind::Invalid,
        }
    }
}

/// Whether Steam reported an OS path or an API file name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalFilePathKind {
    Invalid,
    Absolute,
    ApiFilename,
}

impl From<sys::ERemoteStorageFilePathType> for LocalFilePathKind {
    fn from(raw: sys::ERemoteStorageFilePathType) -> Self {
        match raw {
            sys::ERemoteStorageFilePathType::k_ERemoteStorageFilePathType_Absolute => {
                LocalFilePathKind::Absolute
            }
            sys::ERemoteStorageFilePathType::k_ERemoteStorageFilePathType_APIFilename => {
                LocalFilePathKind::ApiFilename
            }
            _ => LocalFilePathKind::Invalid,
        }
    }
}

/// One entry of [`RemoteStorage::local_file_changes`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalFileChange {
    pub path: String,
    pub change: LocalFileChangeKind,
    pub path_kind: LocalFilePathKind,
}
