//! The three cloud calls whose answer arrives later, through a call result instead of a return
//! value: the two UGC downloads and the asynchronous write.
//!
//! Steam introduces the download as "Downloads a UGC file.  A priority value of 0 will download the
//! file immediately, otherwise it will wait to download the file until all downloads with a lower
//! priority value are completed.  Downloads with equal priority will occur simultaneously."
//! (`isteamremotestorage.h:239-241`), which is why `priority` is passed through untouched: it is
//! Steam's queue position, not a timeout, and this wrapper has no business re-ordering it. The two
//! declarations are `virtual SteamAPICall_t UGCDownload( UGCHandle_t hContent, uint32 unPriority ) = 0;`
//! (`:243`) and `virtual SteamAPICall_t UGCDownloadToLocation( UGCHandle_t hContent, const char
//! *pchLocation, uint32 unPriority ) = 0;` (`:312`), each under
//! `STEAM_CALL_RESULT( RemoteStorageDownloadUGCResult_t )` (`:242`, `:311`).
//!
//! These are the producers the cached-UGC face in `cached_ugc.rs` was missing: that module can
//! enumerate and read content that is already down, but nothing in this crate could put content into
//! that state.

use super::*;
use std::os::raw::c_char;

/// One piece of content Steam says it has.
///
/// "The result of a call to UGCDownload()" (`isteamremotestorage.h:452`) is the six fields of
/// `struct RemoteStorageDownloadUGCResult_t` (`:454-463`), and each one is kept because Steam named
/// it: "The handle to the file that was attempted to be downloaded." (`:458`), "ID of the app that
/// created this file." (`:459`), "The size of the file that was downloaded, in bytes." (`:460`),
/// "The name of the file that was downloaded." (`:461`) and "Steam ID of the user who created this
/// content." (`:462`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UgcDownloaded {
    pub handle: UgcHandle,
    pub app_id: AppId,
    /// Steam's "The size of the file that was downloaded, in bytes." (`isteamremotestorage.h:460`),
    /// clamped by the same rule as the out-parameter read: see `details_size`.
    pub size: u64,
    /// The fixed 260-entry name field, read as Steam's C string: see `fixed_name`.
    pub name: Option<String>,
    /// Steam's "Steam ID of the user who created this content." (`isteamremotestorage.h:462`),
    /// passed through as written.
    pub owner: SteamId,
}

/// A name Steam wrote into a fixed-size buffer, read without walking past that buffer.
///
/// The field is `char m_pchFileName[k_cchFilenameMax];` (`isteamremotestorage.h:461`) and the
/// capacity is its own constant: "const uint32 k_cchFilenameMax = 260;" (`:57`). A buffer that is
/// full to 260 entries needs no reason to hold a terminator, and `CStr::from_ptr` over it would read
/// past the end of the array looking for one -- so the scan is bounded by the slice it was given, and
/// an unterminated buffer is read as the 260 bytes that are actually there.
///
/// An all-zero buffer and a non-UTF-8 one both read as `None`, the collapse this crate already makes
/// for names (`src/remote_storage/workshop_update.rs` and `src/friends/history.rs`): neither shape is
/// a name a caller could show or compare.
pub(super) fn fixed_name(raw: &[c_char]) -> Option<String> {
    let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    let bytes: Vec<u8> = raw[..end].iter().map(|byte| *byte as u8).collect();
    let text = std::str::from_utf8(&bytes).ok()?;
    if text.is_empty() {
        return None;
    }
    Some(text.to_owned())
}

/// The whole answer, once Steam said it was a result and not an error.
///
/// The handle still gets its marker translated: Steam declares "const UGCHandle_t
/// k_UGCHandleInvalid = 0xffffffffffffffffull;" (`isteamremotestorage.h:45`), and this crate refuses
/// to build that value anywhere else (see `cached_ugc.rs`). An OK whose handle is that marker is two
/// statements from Steam at once, so this wrapper takes the pessimistic one and reports failure
/// rather than handing out a handle every other path here would reject.
fn downloaded(
    handle: u64,
    app_id: sys::AppId_t,
    size: ::std::os::raw::c_int,
    name: &[c_char],
    owner: u64,
) -> Result<UgcDownloaded, SteamError> {
    let handle = handle_or_none(handle).ok_or(SteamError::IOFailure)?;
    Ok(UgcDownloaded {
        handle,
        app_id: AppId(app_id),
        size: details_size(size),
        name: fixed_name(name),
        owner: SteamId::from_raw(owner),
    })
}

/// How many bytes Steam may be told to write, from a `usize` the caller handed over.
///
/// "virtual SteamAPICall_t FileWriteAsync( const char *pchFile, const void *pvData, uint32 cubData ) = 0;"
/// (`isteamremotestorage.h:202`) sizes this one as `uint32`, not the `int32` the synchronous
/// `FileWrite` uses (`:197`) -- two different ceilings, which is why the two paths do not share a
/// guard: `super::cloud_chunk_len` refuses what an `int32` cannot carry, this one refuses what a
/// `uint32` cannot. Either way a length Steam cannot receive is a bad argument, not something to
/// clip: clipping would report a short write as success.
fn async_write_len(len: usize) -> Result<u32, SteamError> {
    u32::try_from(len).map_err(|_| SteamError::InvalidParameter)
}

impl RemoteStorage {
    /// Download one piece of content, answering through `cb` when Steam has it.
    ///
    /// `priority` is Steam's own queue position -- see the module note about
    /// "Downloads with equal priority will occur simultaneously" (`isteamremotestorage.h:239-241`).
    pub fn ugc_download<F>(&self, handle: UgcHandle, priority: u32, cb: F)
    where
        F: FnOnce(Result<UgcDownloaded, SteamError>) + 'static + Send,
    {
        let api_call =
            unsafe { sys::SteamAPI_ISteamRemoteStorage_UGCDownload(self.rs, handle.0, priority) };
        self.answer_download(api_call, cb)
    }

    /// Download one piece of content to a chosen location, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t UGCDownloadToLocation( UGCHandle_t hContent, const char *pchLocation,
    /// uint32 unPriority ) = 0;" (`isteamremotestorage.h:312`). A location with an embedded NUL is
    /// refused before Steam is asked and `cb` gets the error: a truncated path would download to
    /// somewhere the caller did not name, which is the failure this crate already refuses on the
    /// synchronous file paths.
    pub fn ugc_download_to_location<F>(
        &self,
        handle: UgcHandle,
        location: &str,
        priority: u32,
        cb: F,
    ) where
        F: FnOnce(Result<UgcDownloaded, SteamError>) + 'static + Send,
    {
        let location = match cloud_file_name(location) {
            Ok(location) => location,
            Err(error) => return cb(Err(error)),
        };
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UGCDownloadToLocation(
                self.rs,
                handle.0,
                location.as_ptr(),
                priority,
            )
        };
        self.answer_download(api_call, cb)
    }

    /// Write a cloud file without blocking, answering through `cb` when Steam has taken it.
    ///
    /// The bytes are copied and the copy is moved into the call-result closure. Steam says only
    /// "STEAM_CALL_RESULT( RemoteStorageFileWriteAsyncComplete_t )" (`isteamremotestorage.h:201`)
    /// above "virtual SteamAPICall_t FileWriteAsync( const char *pchFile, const void *pvData, uint32
    /// cubData ) = 0;" (`:202`), and never states when `pvData` is no longer read -- so a caller's
    /// buffer that went out of scope at the return of this call could still be read. Owning the
    /// copy until the answer arrives is the choice that cannot dangle, and it is also why an oversized
    /// request is refused rather than clipped -- see `async_write_len`.
    pub fn write_file_async<F>(&self, name: &str, data: &[u8], cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        let named = match cloud_file_name(name) {
            Ok(named) => named,
            Err(error) => return cb(Err(error)),
        };
        let wanted = match async_write_len(data.len()) {
            Ok(wanted) => wanted,
            Err(error) => return cb(Err(error)),
        };
        let buffer = Arc::new(data.to_vec());
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_FileWriteAsync(
                self.rs,
                named.as_ptr(),
                buffer.as_ptr().cast(),
                wanted,
            )
        };
        unsafe {
            register_call_result::<sys::RemoteStorageFileWriteAsyncComplete_t, _>(
                &self.inner,
                api_call,
                // The closure owns `buffer`, so the bytes live at least as long as Steam's read of
                // the pointer handed over above; nothing else keeps them alive.
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult)
                    });
                    // The copy drops here, not at the call: this closure is what keeps the bytes
                    // alive while Steam reads the pointer handed over above.
                    drop(buffer);
                },
            );
        }
    }

    /// The one shape both download entries answer with, kept out of the two calls that use it.
    fn answer_download<F>(&self, api_call: sys::SteamAPICall_t, cb: F)
    where
        F: FnOnce(Result<UgcDownloaded, SteamError>) + 'static + Send,
    {
        unsafe {
            register_call_result::<sys::RemoteStorageDownloadUGCResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    let answer = to_steam_result(v.m_eResult).and_then(|()| {
                        downloaded(
                            v.m_hFile,
                            v.m_nAppID,
                            v.m_nSizeInBytes,
                            &v.m_pchFileName,
                            v.m_ulSteamIDOwner,
                        )
                    });
                    cb(answer)
                },
            );
        }
    }
}

#[cfg(test)]
mod download_tests {
    //! The three decisions this face adds to Steam's answer: the bounded read of a fixed name
    //! buffer, the marker that must not become a handle, and the `uint32` ceiling of the async
    //! write. Whether a download actually delivers content, and what a location Steam dislikes
    //! returns for, need a live client and are registered as unverified.

    use super::{async_write_len, downloaded, fixed_name, UgcDownloaded, UgcHandle};
    use crate::remote_storage::details_size;
    use crate::{AppId, SteamError, SteamId};
    use std::os::raw::{c_char, c_int};
    use steamworks_sys as sys;

    /// A name is read up to the terminator or up to the end of the buffer, never past either.
    #[test]
    fn a_name_is_read_inside_the_buffer_steam_gave() {
        let short: Vec<c_char> = "map.bsp"
            .bytes()
            .map(|b| b as c_char)
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(fixed_name(&short), Some("map.bsp".to_owned()));

        let blank = vec![0 as c_char; 8];
        assert_eq!(fixed_name(&blank), None);

        // 260 bytes of name and no terminator at all: `CStr::from_ptr` would walk off the end here.
        let full: Vec<c_char> = std::iter::repeat(b'A' as c_char)
            .take(sys::k_cchFilenameMax as usize)
            .collect();
        let got = fixed_name(&full).expect("an unterminated full buffer still holds its 260 bytes");
        assert_eq!(got.len(), 260);

        let broken: Vec<c_char> = vec![0xffu8 as c_char, 0xfeu8 as c_char, 0];
        assert_eq!(fixed_name(&broken), None, "not UTF-8 is not a name");
    }

    /// The all-ones marker is not a handle, even when Steam also said the operation was OK.
    #[test]
    fn a_marker_never_becomes_a_handle() {
        assert_eq!(
            sys::k_UGCHandleInvalid,
            u64::MAX,
            "isteamremotestorage.h:45"
        );
        let name: Vec<c_char> = "art.png".bytes().map(|b| b as c_char).chain([0]).collect();
        assert_eq!(
            downloaded(sys::k_UGCHandleInvalid, 480, 1024, &name, 7),
            Err(SteamError::IOFailure),
            "a handle Steam itself calls invalid cannot be handed out as content"
        );
        let ok =
            downloaded(42, 480, 1024, &name, 7).expect("an ordinary handle is an ordinary answer");
        assert_eq!(
            ok,
            UgcDownloaded {
                handle: UgcHandle(42),
                app_id: AppId(480),
                size: 1024,
                name: Some("art.png".to_owned()),
                owner: SteamId::from_raw(7),
            }
        );
    }

    /// A negative size is clamped by the same rule the out-parameter read uses.
    #[test]
    fn a_downloaded_size_cannot_be_huge_by_being_negative() {
        assert_eq!(details_size(-1 as c_int), 0);
        let name: Vec<c_char> = std::iter::once(0).collect();
        let ok = downloaded(9, 1, -1, &name, 0).expect("the size is not the reason to refuse");
        assert_eq!(ok.size, 0);
        assert_eq!(ok.name, None);
    }

    /// The async write carries a `uint32`, so the ceiling is 4 GiB minus one -- and a length above
    /// it is refused instead of being clipped into a shorter successful write.
    #[test]
    fn a_length_the_uint32_cannot_carry_is_refused() {
        assert_eq!(async_write_len(0), Ok(0));
        assert_eq!(async_write_len(u32::MAX as usize), Ok(u32::MAX));
        assert_eq!(
            async_write_len(u32::MAX as usize + 1),
            Err(SteamError::InvalidParameter)
        );
        assert_eq!(
            async_write_len(usize::MAX),
            Err(SteamError::InvalidParameter),
            "clipping this would report a short write as success"
        );
    }
}
