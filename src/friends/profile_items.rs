//! The profile items a user equips on their Steam profile, and the properties of one item.
//!
//! Steam's own pointer between the two enums is the line above them: "Purpose: See
//! GetProfileItemPropertyString and GetProfileItemPropertyUint" (`isteamfriends.h:150`). That pair is
//! why this module has a decision to make at all: the property enum's twelve entries are annotated
//! one by one with the reader each of them belongs to -- "string" for the image, name, title,
//! description and movie entries, "uint32" for the three id entries (`:162-173`) -- and nothing in
//! the C interface stops a caller from asking the wrong one. The face itself is what Steam calls
//! "Steam Community items equipped by a user on their profile" (`:427`).

use super::*;
use std::ffi::CStr;
use std::os::raw::c_char;

/// One of the five slots a user can equip something into -- "Steam Community items equipped by a
/// user on their profile" (`isteamfriends.h:427`).
///
/// The widths are Steam's: the list runs `k_ECommunityProfileItemType_AnimatedAvatar` = 0 through
/// `k_ECommunityProfileItemType_MiniProfileBackground` = 4 (`isteamfriends.h:154-158`).
/// [`Self::Unrecognized`] cannot be handed to Steam at all -- the generated layer declares no
/// out-of-range name for this enum, so there is no legal value to build -- and the three readers here
/// answer `InvalidParameter` instead of guessing which number the caller meant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ProfileItemType {
    AnimatedAvatar,
    AvatarFrame,
    ProfileModifier,
    ProfileBackground,
    MiniProfileBackground,
    Unrecognized(i32),
}

/// One property of an equipped item, as the header names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ProfileItemProperty {
    ImageSmall,
    ImageLarge,
    InternalName,
    Title,
    Description,
    AppId,
    TypeId,
    Class,
    MovieWebM,
    MovieMp4,
    MovieWebMSmall,
    MovieMp4Small,
    Unrecognized(i32),
}

/// Which of the two readers a property belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ProfileItemPropertyKind {
    String,
    Uint,
}

/// The header's own annotation, read as a table rather than as prose.
///
/// Every entry in `isteamfriends.h:162-173` carries the column word "string" or "uint32" after its
/// comma -- the two readers being, in Steam's own words, "See GetProfileItemPropertyString and
/// GetProfileItemPropertyUint" (`:150`) -- so the pairing is Steam's claim about its own storage, not
/// a rule this crate would be inventing. [`ProfileItemProperty::Unrecognized`] has no kind: a number the table has no name for
/// is not evidence that either reader would answer it.
fn property_kind(prop: ProfileItemProperty) -> Option<ProfileItemPropertyKind> {
    use ProfileItemProperty::*;
    use ProfileItemPropertyKind::*;
    match prop {
        ImageSmall | ImageLarge | InternalName | Title | Description | MovieWebM | MovieMp4
        | MovieWebMSmall | MovieMp4Small => Some(String),
        AppId | TypeId | Class => Some(Uint),
        Unrecognized(_) => None,
    }
}

/// The generated layer's name for this slot, or nothing when the caller is holding a number Steam
/// never named.
fn item_type_to_sys(item: ProfileItemType) -> Option<sys::ECommunityProfileItemType> {
    use ProfileItemType::*;
    Some(match item {
        AnimatedAvatar => {
            sys::ECommunityProfileItemType::k_ECommunityProfileItemType_AnimatedAvatar
        }
        AvatarFrame => sys::ECommunityProfileItemType::k_ECommunityProfileItemType_AvatarFrame,
        ProfileModifier => {
            sys::ECommunityProfileItemType::k_ECommunityProfileItemType_ProfileModifier
        }
        ProfileBackground => {
            sys::ECommunityProfileItemType::k_ECommunityProfileItemType_ProfileBackground
        }
        MiniProfileBackground => {
            sys::ECommunityProfileItemType::k_ECommunityProfileItemType_MiniProfileBackground
        }
        Unrecognized(_) => return None,
    })
}

/// The generated layer's name for this property, refused under the same rule as the slot.
fn property_to_sys(prop: ProfileItemProperty) -> Option<sys::ECommunityProfileItemProperty> {
    use sys::ECommunityProfileItemProperty as sysp;
    use ProfileItemProperty::*;
    Some(match prop {
        ImageSmall => sysp::k_ECommunityProfileItemProperty_ImageSmall,
        ImageLarge => sysp::k_ECommunityProfileItemProperty_ImageLarge,
        InternalName => sysp::k_ECommunityProfileItemProperty_InternalName,
        Title => sysp::k_ECommunityProfileItemProperty_Title,
        Description => sysp::k_ECommunityProfileItemProperty_Description,
        AppId => sysp::k_ECommunityProfileItemProperty_AppID,
        TypeId => sysp::k_ECommunityProfileItemProperty_TypeID,
        Class => sysp::k_ECommunityProfileItemProperty_Class,
        MovieWebM => sysp::k_ECommunityProfileItemProperty_MovieWebM,
        MovieMp4 => sysp::k_ECommunityProfileItemProperty_MovieMP4,
        MovieWebMSmall => sysp::k_ECommunityProfileItemProperty_MovieWebMSmall,
        MovieMp4Small => sysp::k_ECommunityProfileItemProperty_MovieMP4Small,
        Unrecognized(_) => return None,
    })
}

/// A string Steam handed over, whole or not at all -- the reader Steam names is
/// "GetProfileItemPropertyString" (`isteamfriends.h:150`, `:432`).
///
/// `const char *` (`isteamfriends.h:432`) says nothing about the encoding, and the answer is used as
/// text by whoever asked, so a lossy conversion would silently rewrite the item's name or
/// description into something the caller cannot compare back. A null pointer and a buffer that is not
/// UTF-8 are both reported as `None` rather than as a plausible-looking string.
fn text_or_nothing(raw: *const c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(raw) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

impl Friends {
    /// Whether this user has equipped something in this slot.
    pub fn has_equipped_profile_item(
        &self,
        user: SteamId,
        item: ProfileItemType,
    ) -> SteamResult<bool> {
        let item = item_type_to_sys(item).ok_or(SteamError::InvalidParameter)?;
        Ok(unsafe {
            sys::SteamAPI_ISteamFriends_BHasEquippedProfileItem(self.friends, user.raw(), item)
        })
    }

    /// A text property of the item equipped in this slot.
    ///
    /// Asking for a "uint32" property here is refused before anything is called: the two readers are
    /// separate functions, and the header annotates each property of what it calls "Steam Community
    /// items equipped by a user on their profile" (`isteamfriends.h:427`) with the reader it belongs
    /// to (`:162-173`), so a mismatch is not a question Steam is being asked at all.
    pub fn profile_item_property_string(
        &self,
        user: SteamId,
        item: ProfileItemType,
        prop: ProfileItemProperty,
    ) -> SteamResult<Option<String>> {
        let item = item_type_to_sys(item).ok_or(SteamError::InvalidParameter)?;
        if property_kind(prop) != Some(ProfileItemPropertyKind::String) {
            return Err(SteamError::InvalidParameter);
        }
        let prop = property_to_sys(prop).ok_or(SteamError::InvalidParameter)?;
        let raw = unsafe {
            sys::SteamAPI_ISteamFriends_GetProfileItemPropertyString(
                self.friends,
                user.raw(),
                item,
                prop,
            )
        };
        Ok(text_or_nothing(raw))
    }

    /// A numeric property of the item equipped in this slot.
    pub fn profile_item_property_uint(
        &self,
        user: SteamId,
        item: ProfileItemType,
        prop: ProfileItemProperty,
    ) -> SteamResult<u32> {
        let item = item_type_to_sys(item).ok_or(SteamError::InvalidParameter)?;
        if property_kind(prop) != Some(ProfileItemPropertyKind::Uint) {
            return Err(SteamError::InvalidParameter);
        }
        let prop = property_to_sys(prop).ok_or(SteamError::InvalidParameter)?;
        Ok(unsafe {
            sys::SteamAPI_ISteamFriends_GetProfileItemPropertyUint(
                self.friends,
                user.raw(),
                item,
                prop,
            )
        })
    }
}

#[cfg(test)]
mod profile_item_tests {
    //! The table and the two refusals. Whether a given user has equipped what, and what the strings
    //! say, needs a live client and is registered as unverified.

    use super::{
        item_type_to_sys, property_kind, property_to_sys, text_or_nothing, ProfileItemProperty,
        ProfileItemPropertyKind, ProfileItemType,
    };
    use std::ffi::CString;
    use steamworks_sys as sys;

    /// The header splits the twelve properties into the two readers; the split is what stops a call
    /// that Steam has no answer for.
    #[test]
    fn every_property_belongs_to_exactly_one_reader() {
        use ProfileItemProperty::*;
        let strings = [
            ImageSmall,
            ImageLarge,
            InternalName,
            Title,
            Description,
            MovieWebM,
            MovieMp4,
            MovieWebMSmall,
            MovieMp4Small,
        ];
        let uints = [AppId, TypeId, Class];
        for prop in strings {
            assert_eq!(
                property_kind(prop),
                Some(ProfileItemPropertyKind::String),
                "{:?} is a string property",
                prop
            );
        }
        for prop in uints {
            assert_eq!(
                property_kind(prop),
                Some(ProfileItemPropertyKind::Uint),
                "{:?} is a uint32 property",
                prop
            );
        }
        assert_eq!(property_kind(ProfileItemProperty::Unrecognized(12)), None);
        assert_eq!(
            strings.len() + uints.len(),
            12,
            "the header lists twelve properties"
        );
    }

    /// A number the generated layer never named cannot be turned into one of Steam's arguments --
    /// the bindings declare no out-of-range value, so there is nothing legal to send.
    #[test]
    fn a_number_steam_never_named_is_not_sent_as_an_argument() {
        assert_eq!(
            item_type_to_sys(ProfileItemType::MiniProfileBackground),
            Some(sys::ECommunityProfileItemType::k_ECommunityProfileItemType_MiniProfileBackground)
        );
        assert_eq!(item_type_to_sys(ProfileItemType::Unrecognized(7)), None);
        assert_eq!(
            property_to_sys(ProfileItemProperty::AppId),
            Some(sys::ECommunityProfileItemProperty::k_ECommunityProfileItemProperty_AppID)
        );
        assert_eq!(property_to_sys(ProfileItemProperty::Unrecognized(99)), None);
    }

    /// A buffer that is not UTF-8 is not a name: `to_string_lossy` would hand back replacement
    /// characters that look like the item's title and compare against nothing.
    #[test]
    fn a_string_is_returned_whole_or_not_at_all() {
        assert_eq!(text_or_nothing(std::ptr::null()), None);
        let ok = CString::new("Nyan Cow").unwrap();
        assert_eq!(text_or_nothing(ok.as_ptr()), Some("Nyan Cow".to_owned()));
        let bytes = [0xff_u8, 0x2f, 0x00];
        assert_eq!(text_or_nothing(bytes.as_ptr().cast()), None);
    }

    /// The premise the whole table rests on: the generated layer's numbers are the header's, so a
    /// silent renumbering would move properties between the two readers.
    #[test]
    fn the_generated_layer_still_numbers_the_properties_as_the_header_does() {
        use sys::ECommunityProfileItemProperty::*;
        assert_eq!(
            k_ECommunityProfileItemProperty_ImageSmall as i32, 0,
            "isteamfriends.h:162 anchors the string block at zero"
        );
        assert_eq!(
            k_ECommunityProfileItemProperty_AppID as i32, 5,
            "isteamfriends.h:167"
        );
        assert_eq!(
            k_ECommunityProfileItemProperty_Class as i32, 7,
            "isteamfriends.h:169"
        );
        assert_eq!(
            k_ECommunityProfileItemProperty_MovieMP4Small as i32, 11,
            "isteamfriends.h:173 is the last entry"
        );
    }
}
