//! What an item *is*: the definition id, the instance id, the per-item detail record and
//! the flag tests that read it. The tests for all three live here because every one of them
//! is a statement about a flag bit or an id round-trip.

use super::*;
use std::os::raw::c_char;

/// Definition id of an item
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ItemDef(pub(crate) i32);

impl ItemDef {
    /// Creates an `ItemDef` from a raw 32 bit value.
    ///
    /// May be useful for deserializing item definitions from
    /// a network or save format.
    pub fn from_raw(id: i32) -> ItemDef {
        ItemDef(id)
    }

    /// Returns the raw 32 bit value of the item definition.
    pub fn raw(&self) -> i32 {
        self.0
    }
}

impl From<i32> for ItemDef {
    fn from(id: i32) -> Self {
        ItemDef(id)
    }
}

/// Unique instance id of an item owned by a user
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ItemInstanceId(pub(crate) u64);

impl ItemInstanceId {
    /// Creates an `ItemInstanceId` from a raw 64 bit value.
    ///
    /// May be useful for deserializing item instance ids from
    /// a network or save format.
    pub fn from_raw(id: u64) -> ItemInstanceId {
        ItemInstanceId(id)
    }

    /// Returns the raw 64 bit value of the item instance id.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

/// A single item entry returned by [`InventoryResult::items`]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ItemDetail {
    /// Unique instance id of the item
    pub item_id: ItemInstanceId,
    /// The definition of the item
    pub definition: ItemDef,
    /// How many of this item the user owns
    pub quantity: u16,
    /// Raw item flags bitfield
    pub flags: u16,
}

/// Tests `item_flags` against a Steam `ESteamItemFlags` constant.
///
/// Both sides widen to `u32` because bindgen emits the flag's inner type as
/// `c_int` on Windows but `c_uint` on Linux/macOS; writing either one
/// explicitly fails to compile on the other half of the platforms.
fn has_flag(item_flags: u16, flag: sys::ESteamItemFlags) -> bool {
    u32::from(item_flags) & (flag.0 as u32) != 0
}

impl ItemDetail {
    /// Whether the item has been removed (e.g. traded away).
    ///
    /// Removed items are still present in results for historical reasons
    /// and should not be shown to the user.
    pub fn is_removed(&self) -> bool {
        has_flag(self.flags, sys::ESteamItemFlags::k_ESteamItemRemoved)
    }

    /// Whether the item has been consumed.
    ///
    /// Consumed items are still present in results for historical reasons
    /// and should not be shown to the user.
    pub fn is_consumed(&self) -> bool {
        has_flag(self.flags, sys::ESteamItemFlags::k_ESteamItemConsumed)
    }

    /// Whether the item can be traded with other users.
    pub fn is_tradable(&self) -> bool {
        !has_flag(self.flags, sys::ESteamItemFlags::k_ESteamItemNoTrade)
    }
}

/// Shared two-pass property read: Steam reports the needed size when given
/// a null buffer, then fills the caller-owned buffer on the second call.
// `pub(super)`: the split left two readers of this buffer loop in sibling files.
pub(super) unsafe fn read_property_buffer(
    mut call: impl FnMut(*mut c_char, *mut sys::uint32) -> bool,
) -> Result<Option<String>, SteamError> {
    let mut size: sys::uint32 = 0;
    if !call(std::ptr::null_mut(), &mut size) {
        return Err(SteamError::InvalidParameter);
    }
    if size == 0 {
        return Ok(None);
    }
    let mut buf = vec![0 as c_char; size as usize];
    let mut written = size;
    if !call(buf.as_mut_ptr(), &mut written) {
        return Err(SteamError::IOFailure);
    }
    let value = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Narrows a Steam flag to the item's 16-bit field, failing loudly instead
    /// of truncating if Steam ever assigns a value above `u16::MAX`.
    fn flag(f: sys::ESteamItemFlags) -> u16 {
        u16::try_from(f.0).expect("Steam item flag exceeds u16; widen ItemDetail::flags")
    }

    fn detail_with_flags(flags: u16) -> ItemDetail {
        ItemDetail {
            item_id: ItemInstanceId(0),
            definition: ItemDef(0),
            quantity: 1,
            flags,
        }
    }

    #[test]
    fn plain_item_is_tradable_and_not_removed() {
        let d = detail_with_flags(0);
        assert!(d.is_tradable());
        assert!(!d.is_removed());
        assert!(!d.is_consumed());
    }

    #[test]
    fn no_trade_flag_makes_item_untradable() {
        let d = detail_with_flags(flag(sys::ESteamItemFlags::k_ESteamItemNoTrade));
        assert!(!d.is_tradable());
        // NoTrade must not leak into the removed/consumed predicates.
        assert!(!d.is_removed());
        assert!(!d.is_consumed());
    }

    #[test]
    fn removed_and_consumed_flags_are_distinct() {
        let removed = flag(sys::ESteamItemFlags::k_ESteamItemRemoved);
        let consumed = flag(sys::ESteamItemFlags::k_ESteamItemConsumed);
        assert_ne!(
            removed, consumed,
            "removed/consumed share a value, predicates cannot separate them"
        );

        let r = detail_with_flags(removed);
        assert!(r.is_removed());
        assert!(!r.is_consumed());

        let c = detail_with_flags(consumed);
        assert!(c.is_consumed());
        assert!(!c.is_removed());
    }

    #[test]
    fn flags_combine_and_each_predicate_stays_scoped() {
        let combined = flag(sys::ESteamItemFlags::k_ESteamItemNoTrade)
            | flag(sys::ESteamItemFlags::k_ESteamItemRemoved);
        let d = detail_with_flags(combined);
        assert!(!d.is_tradable());
        assert!(d.is_removed());
        assert!(!d.is_consumed());
    }

    #[test]
    fn item_def_round_trips_through_raw() {
        for id in [0i32, 1, -1, i32::MIN, i32::MAX] {
            assert_eq!(ItemDef::from_raw(id).raw(), id);
            assert_eq!(ItemDef::from(id), ItemDef::from_raw(id));
        }
    }

    #[test]
    fn item_instance_id_round_trips_through_raw() {
        for id in [0u64, 1, u64::MAX / 2, u64::MAX] {
            assert_eq!(ItemInstanceId::from_raw(id).raw(), id);
        }
    }
}
