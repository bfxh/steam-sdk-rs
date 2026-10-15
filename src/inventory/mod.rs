use super::*;

mod actions;
mod callbacks;
mod definitions;
mod items;
mod prices;
mod result;

// `actions` and `definitions` hold only extra `impl Inventory` blocks -- they export no items
// of their own, so they are declared rather than globbed out.
pub use callbacks::*;
pub use items::*;
pub use prices::*;
pub use result::*;

/// Access to the steam inventory interface
pub struct Inventory {
    pub(crate) inv: *mut sys::ISteamInventory,
    pub(crate) _inner: Arc<Inner>,
}

impl Inventory {
    /// Requests all of the user's inventory items.
    ///
    /// Observe [`InventoryResultReady`] or poll [`InventoryResult::is_ready`]
    /// before reading items from the returned result.
    pub fn get_all_items(&self) -> Result<InventoryResult, SteamError> {
        unsafe { self.start_result(|inv, out| sys::SteamAPI_ISteamInventory_GetAllItems(inv, out)) }
    }

    /// Requests a specific set of inventory items by instance id.
    pub fn get_items_by_id(&self, ids: &[ItemInstanceId]) -> Result<InventoryResult, SteamError> {
        let raw: Vec<u64> = ids.iter().map(|id| id.raw()).collect();
        unsafe {
            self.start_result(|inv, out| {
                sys::SteamAPI_ISteamInventory_GetItemsByID(inv, out, raw.as_ptr(), raw.len() as u32)
            })
        }
    }

    /// Grants the user any promotional items they are eligible for.
    pub fn grant_promo_items(&self) -> Result<InventoryResult, SteamError> {
        unsafe {
            self.start_result(|inv, out| sys::SteamAPI_ISteamInventory_GrantPromoItems(inv, out))
        }
    }

    /// Grants the user a single promotional item.
    pub fn add_promo_item(&self, def: ItemDef) -> Result<InventoryResult, SteamError> {
        let raw = def.raw();
        unsafe {
            self.start_result(|inv, out| sys::SteamAPI_ISteamInventory_AddPromoItem(inv, out, raw))
        }
    }

    /// Grants the user several promotional items.
    pub fn add_promo_items(&self, defs: &[ItemDef]) -> Result<InventoryResult, SteamError> {
        let raw: Vec<i32> = defs.iter().map(|d| d.raw()).collect();
        unsafe {
            self.start_result(|inv, out| {
                sys::SteamAPI_ISteamInventory_AddPromoItems(
                    inv,
                    out,
                    raw.as_ptr(),
                    raw.len() as u32,
                )
            })
        }
    }

    /// Begins a batch of item property updates.
    ///
    /// Changes are not applied until [`InventoryUpdateHandle::submit`] is called.
    pub fn start_update_properties(&self) -> InventoryUpdateHandle {
        unsafe {
            InventoryUpdateHandle {
                inv: self.inv,
                handle: sys::SteamAPI_ISteamInventory_StartUpdateProperties(self.inv),
            }
        }
    }
}
