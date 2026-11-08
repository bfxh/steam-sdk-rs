//! The visibility vocabulary this interface carries, with both directions of the conversion
//! onto Steam's enum: readable from whatever Steam answers, and refused with
//! `InvalidParameter` on the way in when the value has no name.

use super::*;

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
