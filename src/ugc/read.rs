//! Low-level reading and mapping helpers shared by the query-side modules.
//!
//! Kept separate because these are the functions where a wrong bound becomes an
//! out-of-bounds read rather than a bad value, and each one is tested on its own.

use super::*;

/// How much this wrapper asks Steam for a key-value tag's value.
///
/// Steam publishes no such limit (`k_cchTagListMax` covers a whole tag list, not
/// one value), so this keeps the 256 the file has always used for KV tags.
pub(super) const UGC_KEY_VALUE_TAG_MAX: u32 = 256;

/// How much this wrapper asks Steam for one end of a game-version range. Same
/// reason as [`UGC_KEY_VALUE_TAG_MAX`]: no published per-branch limit to size by.
pub(super) const UGC_GAME_BRANCH_MAX: u32 = 256;

/// Turns a required-tag group into the C strings Steam expects.
///
/// One unusable tag fails the whole group: a partial group would ask Steam for
/// something the caller never wrote.
pub(super) fn tag_group_strings<S: AsRef<str>>(groups: &[S]) -> Result<Vec<CString>, SteamError> {
    groups
        .iter()
        .map(|tag| CString::new(tag.as_ref()).map_err(|_| SteamError::InvalidParameter))
        .collect()
}

/// Reads a Steam out-string without trusting its terminator: the answer is cut at
/// the first nul inside the buffer this side owns, so an answer that fills the
/// whole buffer cannot make the read run past it.
pub(crate) fn steam_out_string(buffer: &[c_char]) -> String {
    let end = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    let bytes: Vec<u8> = buffer[..end].iter().map(|byte| *byte as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The paging cursor of a completed query. Steam leaves the field empty for a
/// page-number query and for the last cursor page, and both mean "no next page".
pub(super) fn next_cursor_from(raw: &[c_char]) -> Option<String> {
    let cursor = steam_out_string(raw);
    (!cursor.is_empty()).then_some(cursor)
}

/// One of the extra previews on a workshop item, beyond its main picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdditionalPreview {
    /// A URL, or a YouTube video id for [`PreviewKind::YouTubeVideo`] -- Steam
    /// uses the same field for both.
    pub url_or_video_id: String,
    pub original_filename: String,
    pub kind: PreviewKind,
}

/// One game-version (Steam branch) range an item declares it works with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameVersionRange {
    pub min_branch: String,
    pub max_branch: String,
}

/// What kind of preview Steam recorded for an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    Image,
    YouTubeVideo,
    Sketchfab,
    EnvironmentMapHorizontalCross,
    EnvironmentMapLatLong,
    Clip,
    /// A preview type this binding cannot name, including Steam's `ReservedMax`
    /// sentinel and any type added after these bindings were generated.
    Unknown,
}

/// Maps Steam's `EItemPreviewType` answer.
///
/// The out-parameter is read as a plain 32-bit integer rather than as
/// `sys::EItemPreviewType`: the generated enum is `#[repr(i32)]` on Windows and
/// `#[repr(u32)]` on the other platforms, and both are `#[non_exhaustive]`.
pub(super) fn preview_kind_from(raw: i32) -> PreviewKind {
    use sys::EItemPreviewType as P;
    match raw {
        x if x == P::k_EItemPreviewType_Image as i32 => PreviewKind::Image,
        x if x == P::k_EItemPreviewType_YouTubeVideo as i32 => PreviewKind::YouTubeVideo,
        x if x == P::k_EItemPreviewType_Sketchfab as i32 => PreviewKind::Sketchfab,
        x if x == P::k_EItemPreviewType_EnvironmentMap_HorizontalCross as i32 => {
            PreviewKind::EnvironmentMapHorizontalCross
        }
        x if x == P::k_EItemPreviewType_EnvironmentMap_LatLong as i32 => {
            PreviewKind::EnvironmentMapLatLong
        }
        x if x == P::k_EItemPreviewType_Clip as i32 => PreviewKind::Clip,
        _ => PreviewKind::Unknown,
    }
}

/// How many content-descriptor preferences this wrapper has room for in one read.
///
/// Five descriptors exist in these bindings; the extra room is so a growing list
/// does not silently overflow the array, and Steam's reported count is only ever
/// trusted up to this length.
pub(super) const UGC_CONTENT_DESCRIPTORS_MAX: usize = 8;

// Compile-time rather than a test: shrinking the array below Steam's own code list
// must stop the build, not turn a test red.
const _: () = assert!(UGC_CONTENT_DESCRIPTORS_MAX > 5);

/// A preview kind going to Steam, which takes the type by value.
///
/// [`PreviewKind::Unknown`] is refused rather than sent -- it is this wrapper's
/// word for "a code I cannot name", so there is no integer that means it.
pub(super) fn preview_kind_to_sys(kind: PreviewKind) -> Result<sys::EItemPreviewType, SteamError> {
    use sys::EItemPreviewType as P;
    Ok(match kind {
        PreviewKind::Image => P::k_EItemPreviewType_Image,
        PreviewKind::YouTubeVideo => P::k_EItemPreviewType_YouTubeVideo,
        PreviewKind::Sketchfab => P::k_EItemPreviewType_Sketchfab,
        PreviewKind::EnvironmentMapHorizontalCross => {
            P::k_EItemPreviewType_EnvironmentMap_HorizontalCross
        }
        PreviewKind::EnvironmentMapLatLong => P::k_EItemPreviewType_EnvironmentMap_LatLong,
        PreviewKind::Clip => P::k_EItemPreviewType_Clip,
        PreviewKind::Unknown => return Err(SteamError::InvalidParameter),
    })
}

/// A preview file as Steam receives it: resolved on this machine first, so a
/// missing path is an answer rather than an `unwrap()` in the caller's loop.
pub(super) fn preview_file_path(path: &Path) -> Result<CString, SteamError> {
    let resolved = path.canonicalize().map_err(|_| SteamError::FileNotFound)?;
    CString::new(resolved.to_string_lossy().as_bytes()).map_err(|_| SteamError::InvalidParameter)
}

/// How many entries to take from a Steam array answer: the count Steam reports is
/// an answer about its own write, not a promise about the caller's buffer.
pub(super) fn clamped_entries(reported: usize, capacity: usize) -> usize {
    reported.min(capacity)
}
