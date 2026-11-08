//! UGC that has finished downloading and has not been read yet: how many there are, which handle
//! belongs to which position, how far a download has got, and the chunked read itself.
//!
//! Steam introduces the iteration pair as "Functions to iterate through UGC that has finished
//! downloading but has not yet been read via UGCRead()" (`isteamremotestorage.h:260-262`), and the
//! read as "After download, gets the content of the file." (`:252`). The handle has a marker of its
//! own, and it is not zero: "A handle to a piece of user generated content" is `typedef uint64
//! UGCHandle_t` (`:40-41`), and the same page declares `const UGCHandle_t k_UGCHandleInvalid =
//! 0xffffffffffffffffull;` (`:45`) -- all ones, which the generated layer carries as
//! `k_UGCHandleInvalid` (`steamworks-sys/src/windows_bindings.rs:6587`). Treating `0` as the
//! sentinel, the shape the neighbouring `k_PublishedFileIdInvalid` uses (`:44`), would throw away
//! real handles and keep a fake one.
//!
//! The decisions that belong to this wrapper are the widths arriving as signed `int32` and the one
//! division the header itself warns about.

use super::*;
use std::os::raw::c_int;

/// One downloaded piece of content, as Steam numbers it.
///
/// The width is Steam's: "A handle to a piece of user generated content" is `typedef uint64
/// UGCHandle_t` (`isteamremotestorage.h:40-41`). The invalid value is declared next to it as
/// `0xffffffffffffffffull` (`:45`), so this newtype has no "is valid" helper -- the boundary
/// methods translate the marker instead, and a handle built directly from `u64::MAX` is answered by
/// the same table as one obtained from Steam.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct UgcHandle(pub u64);

/// A handle, or nothing when Steam said its own marker.
pub(super) fn handle_or_none(raw: u64) -> Option<UgcHandle> {
    if raw == sys::k_UGCHandleInvalid {
        None
    } else {
        Some(UgcHandle(raw))
    }
}

/// How far a download has got, in bytes, as Steam reported it.
///
/// Both fields are `int32` at the call (`isteamremotestorage.h:247`) of what Steam calls "Gets the
/// amount of data downloaded so far for a piece of content" (`:245`), which is why they arrive here
/// already clamped: see [`RemoteStorage::ugc_download_progress`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UgcDownloadProgress {
    pub downloaded: u32,
    pub expected: u32,
}

impl UgcDownloadProgress {
    /// The share of the transfer that has arrived, or `None` when Steam has not said how much to
    /// expect.
    ///
    /// This is the division Steam warns about: "so be careful to check for that before dividing to
    /// get a percentage" (`isteamremotestorage.h:245-246`). The warning is the reason the answer is
    /// `Option` rather than a `f32` -- a zero expectation is a normal state of a download that has
    /// not started, and turning it into `0.0` would be indistinguishable from a transfer that has
    /// begun and delivered nothing, while turning it into a panic is what the caller would get if
    /// they had divided by the raw field themselves.
    pub fn fraction(&self) -> Option<f32> {
        progress_fraction(self.downloaded, self.expected)
    }
}

/// The share of a transfer, kept apart from the FFI call that fills it in.
fn progress_fraction(downloaded: u32, expected: u32) -> Option<f32> {
    if expected == 0 {
        return None;
    }
    Some(downloaded as f32 / expected as f32)
}

/// How many cached entries Steam counts, from its signed `int32` (`isteamremotestorage.h:261`) --
/// the count for what the header calls "Functions to iterate through UGC that has finished
/// downloading" (`:260`).
///
/// A negative is not a length: the wrap would become the largest number a `usize` holds and then
/// drive a walk over a list that does not exist. Zero is what an empty cache already looks like.
fn cached_count(raw: c_int) -> usize {
    usize::try_from(raw).unwrap_or(0)
}

/// Which cached entry the caller named, refused when Steam's `iCachedContent` cannot hold it
/// (`isteamremotestorage.h:262`), the second half of those "Functions to iterate through UGC that
/// has finished downloading" (`:260`).
fn cached_index(raw: usize) -> Option<c_int> {
    c_int::try_from(raw).ok()
}

/// One of the two byte counters, from Steam's signed `int32` (`isteamremotestorage.h:247`), the
/// pair belonging to "Gets the amount of data downloaded so far for a piece of content" (`:245`).
///
/// The header's only statement about out-of-range values is that the expected figure "can be 0"; it
/// offers no negative, so a negative is read as the absence of a number rather than as a huge one.
fn signed_bytes(raw: c_int) -> u32 {
    u32::try_from(raw).unwrap_or(0)
}

/// How many bytes the read delivered, or nothing when Steam's `int32` says a number that cannot be
/// a count (`isteamremotestorage.h:258`), the call Steam introduces as "After download, gets the
/// content of the file" (`:252`).
///
/// `0` is kept as `Some(0)`: reading nothing is a real answer, and folding it together with a
/// negative would erase the difference between a delivered nothing and a number this crate cannot
/// interpret.
fn bytes_read(raw: c_int) -> Option<usize> {
    usize::try_from(raw).ok()
}

/// What `UGCRead` should do with the file handle afterwards.
///
/// The three values are Steam's, in its order, and the comments on them are what makes the choice
/// meaningful rather than arbitrary -- see each variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum UgcReadAction {
    /// "Keeps the file handle open unless the last byte is read" (`isteamremotestorage.h:146`),
    /// which Steam describes as the behaviour from before this parameter existed (`:148`).
    ContinueReadingUntilFinished,
    /// "Keeps the file handle open. Use this when using UGCRead to seek to different parts of the
    /// file" (`:151-152`).
    ContinueReading,
    /// "Frees the file handle" when the content has been read to the end (`:155`);
    /// "To read the file from Steam again you will need to call UGCDownload again" (`:156`).
    Close,
}

fn action_to_sys(action: UgcReadAction) -> sys::EUGCReadAction {
    use sys::EUGCReadAction::*;
    match action {
        UgcReadAction::ContinueReadingUntilFinished => k_EUGCRead_ContinueReadingUntilFinished,
        UgcReadAction::ContinueReading => k_EUGCRead_ContinueReading,
        UgcReadAction::Close => k_EUGCRead_Close,
    }
}

impl RemoteStorage {
    /// How many downloaded-but-unread pieces of content Steam holds.
    pub fn cached_ugc_count(&self) -> usize {
        let raw = unsafe { sys::SteamAPI_ISteamRemoteStorage_GetCachedUGCCount(self.rs) };
        cached_count(raw)
    }

    /// The handle at this position in Steam's cache list.
    ///
    /// Walk it with [`RemoteStorage::cached_ugc_count`]; an index this wrapper cannot express as
    /// Steam's `iCachedContent` is refused (`isteamremotestorage.h:262`), the pair the header calls
    /// "Functions to iterate through UGC that has finished downloading" (`:260`). `None` is Steam's
    /// own answer, its marker for a handle that names nothing (`:45`), not this crate's guess.
    pub fn cached_ugc_handle(&self, index: usize) -> Option<UgcHandle> {
        let index = cached_index(index)?;
        let raw = unsafe { sys::SteamAPI_ISteamRemoteStorage_GetCachedUGCHandle(self.rs, index) };
        handle_or_none(raw)
    }

    /// How far this piece of content has downloaded.
    ///
    /// `Err(InvalidParameter)` is Steam declining to answer
    /// (`isteamremotestorage.h:247`) for what it calls "Gets the amount of data downloaded so far
    /// for a piece of content" (`:245`), the same shape every other out-parameter getter in this
    /// crate uses. Once it does answer, the expected
    /// figure can still be zero -- the header says so for a transfer that has not started -- which
    /// is why [`UgcDownloadProgress::fraction`] is the way to turn this into a proportion.
    pub fn ugc_download_progress(&self, handle: UgcHandle) -> SteamResult<UgcDownloadProgress> {
        let mut downloaded: c_int = 0;
        let mut expected: c_int = 0;
        let answered = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_GetUGCDownloadProgress(
                self.rs,
                handle.0,
                &mut downloaded,
                &mut expected,
            )
        };
        if !answered {
            return Err(SteamError::InvalidParameter);
        }
        Ok(UgcDownloadProgress {
            downloaded: signed_bytes(downloaded),
            expected: signed_bytes(expected),
        })
    }

    /// Read a chunk of downloaded content into `into`, from `offset`, and say how many bytes arrived.
    ///
    /// Three of Steam's lines decide the shape of this call. The size is Steam's `int32`
    /// (`isteamremotestorage.h:258`), so a buffer wider than that is refused through the same check
    /// the cloud file writers use rather than clipped. "Once the last byte is read, the file is
    /// implicitly closed and further calls to UGCRead will fail" unless it is downloaded again
    /// (`:255-256`), which is what [`UgcReadAction`] is for. And "For especially large files
    /// (anything over 100MB) it is a requirement that the file is read in chunks" (`:257`) -- which
    /// is why the caller supplies the buffer and this wrapper never allocates one from a number
    /// Steam returned.
    pub fn ugc_read(
        &self,
        handle: UgcHandle,
        into: &mut [u8],
        offset: u32,
        action: UgcReadAction,
    ) -> SteamResult<Option<usize>> {
        let wanted = cloud_chunk_len(into.len())?;
        let read = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UGCRead(
                self.rs,
                handle.0,
                into.as_mut_ptr().cast(),
                wanted,
                offset,
                action_to_sys(action),
            )
        };
        Ok(bytes_read(read))
    }
}

#[cfg(test)]
mod cached_ugc_tests {
    //! The four numbers Steam hands over or takes back, and the one division its own comment warns
    //! about. Which content is actually cached, and how far it has got, needs a live client and is
    //! registered as unverified, as is what Steam answers for a position past the end of the cache.

    use super::{
        action_to_sys, bytes_read, cached_count, cached_index, handle_or_none, progress_fraction,
        signed_bytes, UgcDownloadProgress, UgcHandle,
    };
    use std::os::raw::c_int;
    use steamworks_sys as sys;

    /// The clamp that keeps a count from becoming a loop bound of 18446744073709551615.
    #[test]
    fn a_signed_count_never_becomes_a_wrap() {
        assert_eq!(cached_count(0), 0);
        assert_eq!(cached_count(7), 7);
        assert_eq!(cached_count(-1), 0);
        assert_eq!(cached_count(c_int::MIN), 0);
        assert_ne!(cached_count(-1), usize::MAX);
    }

    /// An index Steam cannot receive is refused before anything is asked, because a cast would have
    /// named a different entry than the caller meant.
    #[test]
    fn an_index_that_cannot_be_a_c_int_is_never_asked_about() {
        assert_eq!(cached_index(0), Some(0));
        assert_eq!(cached_index(c_int::MAX as usize), Some(c_int::MAX));
        assert_eq!(cached_index(c_int::MAX as usize + 1), None);
    }

    /// Zero expectation is not a number to divide by, and the header says exactly that; the two
    /// zero-numerators it mentions (`returns false`, `hasn't started yet`) both land on `None`.
    #[test]
    fn a_proportion_is_only_computed_when_steam_said_how_much_to_expect() {
        assert_eq!(progress_fraction(0, 0), None);
        assert_eq!(progress_fraction(512, 0), None);
        assert_eq!(progress_fraction(0, 1024), Some(0.0));
        assert_eq!(progress_fraction(1024, 1024), Some(1.0));
        let mid = UgcDownloadProgress {
            downloaded: 256,
            expected: 1024,
        };
        assert_eq!(mid.fraction(), Some(0.25));
    }

    /// The header's own label for this call is "Gets the amount of data downloaded so far for a piece
    /// of content" (`isteamremotestorage.h:245`), and the byte counters arrive signed from it, so
    /// they are never allowed to wrap into a huge length.
    #[test]
    fn signed_byte_counters_do_not_wrap_into_huge_values() {
        assert_eq!(signed_bytes(0), 0);
        assert_eq!(signed_bytes(4096), 4096);
        assert_eq!(signed_bytes(-1), 0);
        assert_ne!(signed_bytes(-1), u32::MAX);
    }

    /// A read of nothing and a number that cannot be a count stay two separate answers.
    #[test]
    fn a_read_of_zero_stays_distinct_from_a_number_that_is_not_a_count() {
        assert_eq!(bytes_read(0), Some(0));
        assert_eq!(bytes_read(16), Some(16));
        assert_eq!(bytes_read(-1), None);
    }

    /// The three actions map to Steam's own three numbers, which are the order its comments assume.
    #[test]
    fn the_three_actions_map_to_the_numbers_the_header_orders_them_by() {
        use super::UgcReadAction::*;
        assert_eq!(
            action_to_sys(ContinueReadingUntilFinished) as c_int,
            sys::EUGCReadAction::k_EUGCRead_ContinueReadingUntilFinished as c_int
        );
        assert_eq!(
            action_to_sys(ContinueReading) as c_int,
            sys::EUGCReadAction::k_EUGCRead_ContinueReading as c_int
        );
        assert_eq!(
            action_to_sys(Close) as c_int,
            sys::EUGCReadAction::k_EUGCRead_Close as c_int
        );
    }

    /// Steam's marker for a handle that names nothing is all ones, and zero is an ordinary value.
    /// these two the wrong way round is the quiet failure: real handles vanish and a fake one stays.
    #[test]
    fn the_invalid_handle_is_steam_s_marker_and_not_zero() {
        assert_eq!(
            sys::k_UGCHandleInvalid,
            u64::MAX,
            "isteamremotestorage.h:45 declares 0xffffffffffffffffull; a `0` here would be a different claim"
        );
        assert_eq!(handle_or_none(sys::k_UGCHandleInvalid), None);
        assert_eq!(handle_or_none(0), Some(UgcHandle(0)));
        assert_eq!(
            handle_or_none(18446744073709551614),
            Some(UgcHandle(18446744073709551614))
        );
    }
}
