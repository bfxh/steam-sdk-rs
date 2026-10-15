use super::*;
#[cfg(test)]
use serial_test::serial;

/// Access to the steam remote storage interface
pub struct RemoteStorage {
    pub(crate) rs: *mut sys::ISteamRemoteStorage,
    pub(crate) util: *mut sys::ISteamUtils,
    pub(crate) inner: Arc<Inner>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishedFileVisibility {
    Public,
    FriendsOnly,
    Private,
    Unlisted,

    /// A visibility this binding does not name.
    ///
    /// `ERemoteStoragePublishedFileVisibility` is `#[non_exhaustive]` and the vendored headers
    /// stop at `...Unlisted = 3`, so a Steam newer than this SDK can answer 4 or above. The
    /// fallback arm used to reach `unreachable!()`, which panicked inside the caller's process
    /// while reading an item back.
    ///
    /// Read-only. Sending it back refuses with [`SteamError::InvalidParameter`] rather than
    /// inventing a code: see the `TryFrom` below.
    Unknown,
}

impl From<sys::ERemoteStoragePublishedFileVisibility> for PublishedFileVisibility {
    fn from(visibility: sys::ERemoteStoragePublishedFileVisibility) -> Self {
        match visibility {
            sys::ERemoteStoragePublishedFileVisibility::k_ERemoteStoragePublishedFileVisibilityPublic => PublishedFileVisibility::Public,
            sys::ERemoteStoragePublishedFileVisibility::k_ERemoteStoragePublishedFileVisibilityFriendsOnly => PublishedFileVisibility::FriendsOnly,
            sys::ERemoteStoragePublishedFileVisibility::k_ERemoteStoragePublishedFileVisibilityPrivate => PublishedFileVisibility::Private,
            sys::ERemoteStoragePublishedFileVisibility::k_ERemoteStoragePublishedFileVisibilityUnlisted => PublishedFileVisibility::Unlisted,
            // The binding is `#[non_exhaustive]`, so this arm is the answer to "newer than us",
            // not an assertion that it cannot happen.
            _ => PublishedFileVisibility::Unknown,
        }
    }
}

/// Writes a visibility to Steam.
///
/// The infallible `Into` this replaced could not answer `Unknown` honestly. Unlike
/// [`crate::FileType`], whose fix defers by sending Steam's own `k_EWorkshopFileTypeMax`,
/// `ERemoteStoragePublishedFileVisibility` declares no out-of-range variant at all -- measured
/// against the vendored bindings, its values are exactly 0..3 -- so there is no sentinel to hand
/// over and no legal Rust way to build one. `Unknown` therefore refuses here, at the boundary,
/// instead of reaching Steam as a visibility the caller never asked for.
impl TryFrom<PublishedFileVisibility> for sys::ERemoteStoragePublishedFileVisibility {
    type Error = SteamError;

    fn try_from(visibility: PublishedFileVisibility) -> Result<Self, Self::Error> {
        Ok(match visibility {
            PublishedFileVisibility::Public => {
                sys::ERemoteStoragePublishedFileVisibility::k_ERemoteStoragePublishedFileVisibilityPublic
            }
            PublishedFileVisibility::FriendsOnly => {
                sys::ERemoteStoragePublishedFileVisibility::k_ERemoteStoragePublishedFileVisibilityFriendsOnly
            }
            PublishedFileVisibility::Private => {
                sys::ERemoteStoragePublishedFileVisibility::k_ERemoteStoragePublishedFileVisibilityPrivate
            }
            PublishedFileVisibility::Unlisted => {
                sys::ERemoteStoragePublishedFileVisibility::k_ERemoteStoragePublishedFileVisibilityUnlisted
            }
            PublishedFileVisibility::Unknown => return Err(SteamError::InvalidParameter),
        })
    }
}

impl Clone for RemoteStorage {
    fn clone(&self) -> Self {
        RemoteStorage {
            inner: self.inner.clone(),
            rs: self.rs,
            util: self.util,
        }
    }
}

impl RemoteStorage {
    /// Toggles whether the steam cloud is enabled for the application
    pub fn set_cloud_enabled_for_app(&self, enabled: bool) {
        unsafe {
            sys::SteamAPI_ISteamRemoteStorage_SetCloudEnabledForApp(self.rs, enabled);
        }
    }

    /// Returns whether the steam cloud is enabled for the application
    ///
    /// # Note
    ///
    /// This is independent from the account wide setting
    pub fn is_cloud_enabled_for_app(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_IsCloudEnabledForApp(self.rs) }
    }

    /// Returns whether the steam cloud is enabled for the account
    ///
    /// # Note
    ///
    /// This is independent from the application setting
    pub fn is_cloud_enabled_for_account(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_IsCloudEnabledForAccount(self.rs) }
    }

    /// Returns information about all files in the cloud storage
    pub fn files(&self) -> Vec<SteamFileInfo> {
        unsafe {
            let count = sys::SteamAPI_ISteamRemoteStorage_GetFileCount(self.rs);
            if count == -1 {
                return Vec::new();
            }
            let mut files = Vec::with_capacity(count as usize);
            for idx in 0..count {
                let mut size = 0;
                let name = CStr::from_ptr(sys::SteamAPI_ISteamRemoteStorage_GetFileNameAndSize(
                    self.rs, idx, &mut size,
                ));
                files.push(SteamFileInfo {
                    name: name.to_string_lossy().into_owned(),
                    size: size as u64,
                })
            }

            files
        }
    }

    /// Returns a handle to a steam cloud file
    ///
    /// The file does not have to exist.
    pub fn file(&self, name: &str) -> SteamFile {
        SteamFile {
            rs: self.rs,
            util: self.util,
            _inner: self.inner.clone(),
            name: CString::new(name).unwrap(),
        }
    }

    /// Writes a whole file in one synchronous call.
    ///
    /// Where [`SteamFile::write`] streams through Steam, this is the one-shot
    /// form Steam documents for small files. The FFI length parameter is an
    /// int32, so an input that cannot be expressed in it is rejected rather
    /// than silently truncated.
    pub fn write_file(&self, name: &str, data: &[u8]) -> Result<(), SteamError> {
        let file = cloud_file_name(name)?;
        let len = cloud_chunk_len(data.len())?;
        // Steam takes a payload pointer even for a zero length write. Handing
        // it null is not something we can show is safe, so an empty write gets
        // an aligned, never-dereferenced address instead.
        let data_ptr: *const c_void = if data.is_empty() {
            std::ptr::NonNull::<u8>::dangling().as_ptr() as *const c_void
        } else {
            data.as_ptr() as *const c_void
        };
        let ok = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_FileWrite(self.rs, file.as_ptr(), data_ptr, len)
        };
        if ok {
            Ok(())
        } else {
            Err(SteamError::IOFailure)
        }
    }

    /// Reads up to `max_bytes` of a cloud file in one synchronous call, and
    /// returns what was actually read.
    ///
    /// Steam reports a missing file, a buffer that was too small and a failed
    /// read all as a negative count, so that is surfaced as
    /// [`SteamError::FileNotFound`] rather than as an empty file.
    pub fn read_file(&self, name: &str, max_bytes: usize) -> Result<Vec<u8>, SteamError> {
        let file = cloud_file_name(name)?;
        let to_read = cloud_chunk_len(max_bytes)?;
        let mut buf = vec![0u8; max_bytes];
        let read = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_FileRead(
                self.rs,
                file.as_ptr(),
                buf.as_mut_ptr() as *mut c_void,
                to_read,
            )
        };
        if read < 0 {
            return Err(SteamError::FileNotFound);
        }
        buf.truncate(read as usize);
        Ok(buf)
    }

    /// Cloud storage quota as `(total_bytes, available_bytes)`.
    ///
    /// `None` when Steam cannot answer, which is what it does while cloud saves
    /// are off for this app -- so `None` is not "zero bytes left".
    pub fn quota(&self) -> Option<(u64, u64)> {
        let mut total = 0u64;
        let mut available = 0u64;
        let ok = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_GetQuota(self.rs, &mut total, &mut available)
        };
        ok.then_some((total, available))
    }

    /// Cloud files that changed locally since the previous call.
    ///
    /// Steam hands back its own buffer for each name, so every name is copied
    /// out before the next query.
    pub fn local_file_changes(&self) -> Vec<LocalFileChange> {
        let count = unsafe { sys::SteamAPI_ISteamRemoteStorage_GetLocalFileChangeCount(self.rs) };
        if count <= 0 {
            return Vec::new();
        }
        let mut changes = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut change =
                sys::ERemoteStorageLocalFileChange::k_ERemoteStorageLocalFileChange_Invalid;
            let mut path_kind =
                sys::ERemoteStorageFilePathType::k_ERemoteStorageFilePathType_Invalid;
            let raw = unsafe {
                sys::SteamAPI_ISteamRemoteStorage_GetLocalFileChange(
                    self.rs,
                    index,
                    &mut change,
                    &mut path_kind,
                )
            };
            let path = if raw.is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(raw).to_string_lossy().into_owned() }
            };
            changes.push(LocalFileChange {
                path,
                change: change.into(),
                path_kind: path_kind.into(),
            });
        }
        changes
    }

    /// Starts a group of file writes that Steam syncs once when the returned
    /// guard is dropped.
    ///
    /// Fails when Steam refuses the batch, so a caller cannot mistake a refused
    /// batch for a fast one.
    pub fn begin_write_batch(&self) -> Result<FileWriteBatch, SteamError> {
        if unsafe { sys::SteamAPI_ISteamRemoteStorage_BeginFileWriteBatch(self.rs) } {
            Ok(FileWriteBatch { rs: self.rs })
        } else {
            Err(SteamError::IOFailure)
        }
    }
}

/// A name is passed to Steam as a C string, so an embedded NUL is a bad
/// argument rather than a panic.
fn cloud_file_name(name: &str) -> Result<CString, SteamError> {
    CString::new(name).map_err(|_| SteamError::InvalidParameter)
}

/// Steam sizes these calls with an int32; a length it cannot express is a bad
/// argument, not something to clip.
fn cloud_chunk_len(len: usize) -> Result<i32, SteamError> {
    i32::try_from(len).map_err(|_| SteamError::InvalidParameter)
}

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

/// Ends the Steam write batch on drop.
pub struct FileWriteBatch {
    rs: *mut sys::ISteamRemoteStorage,
}

impl Drop for FileWriteBatch {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamRemoteStorage_EndFileWriteBatch(self.rs);
        }
    }
}

bitflags! {
    /// Platform flags used with [`SteamFile::set_sync_platforms`] to restrict
    /// syncing to specific operating systems.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[repr(C)]
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct RemoteStoragePlatforms: u32 {
        const WINDOWS = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformWindows.0 as _;
        const MACOS = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformOSX.0 as _;
        const PS3 = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformPS3.0 as _;
        const LINUX = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformLinux.0 as _;
        const SWITCH = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformSwitch.0 as _;
        const ANDROID = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformAndroid.0 as _;
        const IOS = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformIOS.0 as _;
    }
}

impl From<RemoteStoragePlatforms> for sys::ERemoteStoragePlatform {
    fn from(platforms: RemoteStoragePlatforms) -> Self {
        sys::ERemoteStoragePlatform(platforms.bits() as _)
    }
}

/// A handle for a possible steam cloud file
pub struct SteamFile {
    pub(crate) rs: *mut sys::ISteamRemoteStorage,
    pub(crate) util: *mut sys::ISteamUtils,
    pub(crate) _inner: Arc<Inner>,
    name: CString,
}

impl SteamFile {
    /// Deletes the file locally and remotely.
    ///
    /// Returns whether a file was actually deleted
    pub fn delete(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_FileDelete(self.rs, self.name.as_ptr()) }
    }
    /// Deletes the file remotely whilst keeping it locally.
    ///
    /// Returns whether a file was actually forgotten
    pub fn forget(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_FileForget(self.rs, self.name.as_ptr()) }
    }

    /// Returns whether a file exists
    pub fn exists(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_FileExists(self.rs, self.name.as_ptr()) }
    }

    /// Returns whether a file is persisted in the steam cloud
    pub fn is_persisted(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_FilePersisted(self.rs, self.name.as_ptr()) }
    }

    /// Returns the timestamp of the file
    pub fn timestamp(&self) -> i64 {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_GetFileTimestamp(self.rs, self.name.as_ptr()) }
    }

    /// Set which platforms the file should be available on
    pub fn set_sync_platforms(&self, platforms: RemoteStoragePlatforms) {
        unsafe {
            sys::SteamAPI_ISteamRemoteStorage_SetSyncPlatforms(
                self.rs,
                self.name.as_ptr(),
                platforms.into(),
            );
        }
    }

    /// Returns the platforms the file is available on
    pub fn get_sync_platforms(&self) -> RemoteStoragePlatforms {
        let bits = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_GetSyncPlatforms(self.rs, self.name.as_ptr())
        };
        RemoteStoragePlatforms::from_bits_truncate(bits.0 as _)
    }

    pub fn write(self) -> SteamFileWriter {
        unsafe {
            let handle =
                sys::SteamAPI_ISteamRemoteStorage_FileWriteStreamOpen(self.rs, self.name.as_ptr());
            SteamFileWriter { file: self, handle }
        }
    }

    pub fn read(self) -> SteamFileReader {
        unsafe {
            SteamFileReader {
                offset: 0,
                size: sys::SteamAPI_ISteamRemoteStorage_GetFileSize(self.rs, self.name.as_ptr())
                    as usize,
                file: self,
            }
        }
    }

    pub fn share(&self, cb: impl FnOnce(Result<u64, SteamError>) + 'static + Send) {
        let api_call =
            unsafe { sys::SteamAPI_ISteamRemoteStorage_FileShare(self.rs, self.name.as_ptr()) };
        unsafe {
            register_call_result::<sys::RemoteStorageFileShareResult_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        cb(Err(SteamError::IOFailure));
                        return;
                    }

                    cb(to_steam_result(v.m_eResult).map(|()| v.m_hFile))
                },
            )
        }
    }
}

/// A write handle for a steam cloud file
pub struct SteamFileWriter {
    file: SteamFile,
    handle: sys::UGCFileWriteStreamHandle_t,
}

impl std::io::Write for SteamFileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        unsafe {
            if sys::SteamAPI_ISteamRemoteStorage_FileWriteStreamWriteChunk(
                self.file.rs,
                self.handle,
                buf.as_ptr().cast(),
                buf.len() as _,
            ) {
                Ok(buf.len())
            } else {
                Err(std::io::ErrorKind::Other.into())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for SteamFileWriter {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamRemoteStorage_FileWriteStreamClose(self.file.rs, self.handle);
        }
    }
}

/// A read handle for a steam cloud file
pub struct SteamFileReader {
    file: SteamFile,
    offset: usize,
    size: usize,
}

impl std::io::Read for SteamFileReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::cmp::min;
        if buf.is_empty() || self.size - self.offset == 0 {
            return Ok(0);
        }
        let len = min(buf.len(), self.size - self.offset);
        unsafe {
            let api_call = sys::SteamAPI_ISteamRemoteStorage_FileReadAsync(
                self.file.rs,
                self.file.name.as_ptr(),
                self.offset as _,
                len as _,
            );

            let mut failed = false;
            while !sys::SteamAPI_ISteamUtils_IsAPICallCompleted(
                self.file.util,
                api_call,
                &mut failed,
            ) {
                std::thread::yield_now();
            }
            if failed {
                return Err(std::io::ErrorKind::Other.into());
            }
            let mut callback: sys::RemoteStorageFileReadAsyncComplete_t = std::mem::zeroed();
            sys::SteamAPI_ISteamUtils_GetAPICallResult(
                self.file.util,
                api_call,
                (&mut callback) as *mut _ as *mut _,
                std::mem::size_of::<sys::RemoteStorageFileReadAsyncComplete_t>() as _,
                1332,
                &mut failed,
            );

            if to_steam_result(callback.m_eResult).is_err() {
                return Err(std::io::ErrorKind::Other.into());
            }
            let size = callback.m_cubRead as usize;
            sys::SteamAPI_ISteamRemoteStorage_FileReadAsyncComplete(
                self.file.rs,
                callback.m_hFileReadAsync,
                buf.as_mut_ptr().cast(),
                callback.m_cubRead,
            );

            self.offset += size;
            Ok(size)
        }
    }
}

impl std::io::Seek for SteamFileReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            std::io::SeekFrom::Current(o) => {
                if self.offset as isize + o as isize >= self.size as isize {
                    return Err(std::io::ErrorKind::InvalidInput.into());
                }
                self.offset = (self.offset as isize + o as isize) as usize;
            }
            std::io::SeekFrom::End(o) => {
                if o as isize >= self.size as isize {
                    return Err(std::io::ErrorKind::InvalidInput.into());
                }
                self.offset = (self.size as isize - 1 - o as isize) as usize;
            }
            std::io::SeekFrom::Start(o) => {
                if o as usize >= self.size {
                    return Err(std::io::ErrorKind::InvalidInput.into());
                }
                self.offset = o as usize;
            }
        }
        Ok(self.offset as u64)
    }
}

/// Name and size information about a file in the steam cloud
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamFileInfo {
    /// The file name
    pub name: String,
    /// The size of the file in bytes
    pub size: u64,
}

/// Offline guards for the one-shot cloud file calls. These need no Steam
/// client: they pin the argument checks that run before any FFI call.
#[cfg(test)]
mod offline_tests {
    use super::*;

    #[test]
    fn embedded_nul_in_a_cloud_file_name_is_rejected_not_panicked() {
        assert!(cloud_file_name("saves/slot1").is_ok());
        assert!(matches!(
            cloud_file_name("slot1\0second"),
            Err(SteamError::InvalidParameter)
        ));
    }

    #[test]
    fn lengths_steam_cannot_express_are_rejected_not_clipped() {
        assert_eq!(cloud_chunk_len(0).unwrap(), 0);
        assert_eq!(cloud_chunk_len(4096).unwrap(), 4096);
        assert_eq!(cloud_chunk_len(i32::MAX as usize).unwrap(), i32::MAX);
        for too_big in [i32::MAX as usize + 1, usize::MAX] {
            assert!(
                matches!(cloud_chunk_len(too_big), Err(SteamError::InvalidParameter)),
                "{too_big} bytes should not be accepted as an int32 length"
            );
        }
    }

    #[test]
    fn local_change_kinds_map_onto_the_variants_steam_defines() {
        use sys::ERemoteStorageLocalFileChange as C;
        let mapped: Vec<_> = [
            C::k_ERemoteStorageLocalFileChange_Invalid,
            C::k_ERemoteStorageLocalFileChange_FileUpdated,
            C::k_ERemoteStorageLocalFileChange_FileDeleted,
        ]
        .iter()
        .map(|k| LocalFileChangeKind::from(*k))
        .collect();
        assert_eq!(
            mapped,
            vec![
                LocalFileChangeKind::Invalid,
                LocalFileChangeKind::Updated,
                LocalFileChangeKind::Deleted
            ]
        );
        // Every Steam variant has to land on a distinct safe variant, or two
        // different states would read back as one.
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(mapped[i], mapped[j]);
            }
        }
    }

    #[test]
    fn local_path_kinds_map_onto_the_variants_steam_defines() {
        use sys::ERemoteStorageFilePathType as P;
        let mapped: Vec<_> = [
            P::k_ERemoteStorageFilePathType_Invalid,
            P::k_ERemoteStorageFilePathType_Absolute,
            P::k_ERemoteStorageFilePathType_APIFilename,
        ]
        .iter()
        .map(|p| LocalFilePathKind::from(*p))
        .collect();
        assert_eq!(
            mapped,
            vec![
                LocalFilePathKind::Invalid,
                LocalFilePathKind::Absolute,
                LocalFilePathKind::ApiFilename
            ]
        );
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(mapped[i], mapped[j]);
            }
        }
    }

    #[test]
    fn the_four_visibility_codes_keep_their_distinct_steam_numbers() {
        use sys::ERemoteStoragePublishedFileVisibility as V;
        let codes = [
            (
                PublishedFileVisibility::Public,
                V::k_ERemoteStoragePublishedFileVisibilityPublic,
            ),
            (
                PublishedFileVisibility::FriendsOnly,
                V::k_ERemoteStoragePublishedFileVisibilityFriendsOnly,
            ),
            (
                PublishedFileVisibility::Private,
                V::k_ERemoteStoragePublishedFileVisibilityPrivate,
            ),
            (
                PublishedFileVisibility::Unlisted,
                V::k_ERemoteStoragePublishedFileVisibilityUnlisted,
            ),
        ];
        for (ours, theirs) in codes {
            assert_eq!(V::try_from(ours).unwrap(), theirs);
        }
        // Distinctness is the point, not the names: two visibilities sharing a code would let a
        // caller set one and have Steam store the other.
        for i in 0..codes.len() {
            for j in (i + 1)..codes.len() {
                assert_ne!(
                    V::try_from(codes[i].0).unwrap(),
                    V::try_from(codes[j].0).unwrap()
                );
            }
        }
    }

    #[test]
    fn a_visibility_steam_was_never_told_about_has_no_code_to_send() {
        // There is no out-of-range variant here to defer to (the bindings declare exactly 0..3),
        // so the only alternative to refusing is picking a real visibility for the caller. A
        // private item written back as Public is a publish, and no test would show it.
        assert!(matches!(
            sys::ERemoteStoragePublishedFileVisibility::try_from(PublishedFileVisibility::Unknown),
            Err(SteamError::InvalidParameter)
        ));
    }
}

/// Round-trips the one-shot file APIs. Needs a client, so it is ignored by
/// default; run it with `cargo test -- --ignored`.
#[test]
#[serial]
#[ignore = "requires a running Steam client"]
fn test_one_shot_cloud_io() {
    let client = Client::init().unwrap();
    let rs = client.remote_storage();

    let payload = b"steamworks-rs one-shot write".to_vec();
    rs.write_file("qoder-one-shot.bin", &payload).unwrap();
    // A second, empty write must not be sent as a null payload.
    rs.write_file("qoder-empty.bin", &[]).unwrap();

    assert_eq!(rs.read_file("qoder-one-shot.bin", 4096).unwrap(), payload);
    assert_eq!(rs.read_file("qoder-empty.bin", 4096).unwrap(), Vec::new());
    // Asking for fewer bytes than the file holds truncates, it does not fail.
    assert_eq!(rs.read_file("qoder-one-shot.bin", 4).unwrap().len(), 4);
    assert!(matches!(
        rs.read_file("qoder-does-not-exist.bin", 16),
        Err(SteamError::FileNotFound)
    ));

    let _batch = rs.begin_write_batch().unwrap();
    rs.write_file("qoder-in-batch.bin", b"batched").unwrap();
    drop(_batch);

    for change in rs.local_file_changes() {
        println!("{change:?}");
    }
    if let Some((total, available)) = rs.quota() {
        println!("quota: {available} of {total} bytes free");
    }
    for name in [
        "qoder-one-shot.bin",
        "qoder-empty.bin",
        "qoder-in-batch.bin",
    ] {
        rs.file(name).delete();
    }
}

#[test]
#[serial]
#[ignore = "requires a running Steam client"]
fn test_cloud() {
    use std::io::{Read, Write};
    let client = Client::init().unwrap();

    let rs = client.remote_storage();
    println!("Listing files:");
    for f in rs.files() {
        println!("{:?}", f);
    }

    {
        let test = rs.file("test.txt");
        let mut w = test.write();
        write!(w, "Testing").unwrap();
    }

    println!("Listing files:");
    for f in rs.files() {
        println!("{:?}", f);
    }

    let mut output = String::new();
    let test = rs.file("test.txt");
    test.read().read_to_string(&mut output).unwrap();
    println!("Got: {:?}", output);

    assert_eq!(output, "Testing");
}
