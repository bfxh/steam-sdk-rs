//! Requests that change what the player owns: consuming, exchanging, transferring,
//! trading and dropping items.

use super::*;

impl Inventory {
    /// Consumes a quantity of an item.
    pub fn consume_item(
        &self,
        item: ItemInstanceId,
        quantity: u32,
    ) -> Result<InventoryResult, SteamError> {
        let raw = item.raw();
        unsafe {
            self.start_result(|inv, out| {
                sys::SteamAPI_ISteamInventory_ConsumeItem(inv, out, raw, quantity)
            })
        }
    }

    /// Destroys items in exchange for newly generated ones.
    ///
    /// `generate`/`generate_quantity` and `destroy`/`destroy_quantity` must be
    /// the same length.
    pub fn exchange_items(
        &self,
        generate: &[ItemDef],
        generate_quantity: &[u32],
        destroy: &[ItemInstanceId],
        destroy_quantity: &[u32],
    ) -> Result<InventoryResult, SteamError> {
        if generate.len() != generate_quantity.len() || destroy.len() != destroy_quantity.len() {
            return Err(SteamError::InvalidParameter);
        }
        let raw_generate: Vec<i32> = generate.iter().map(|d| d.raw()).collect();
        let raw_destroy: Vec<u64> = destroy.iter().map(|d| d.raw()).collect();
        unsafe {
            self.start_result(|inv, out| {
                sys::SteamAPI_ISteamInventory_ExchangeItems(
                    inv,
                    out,
                    raw_generate.as_ptr(),
                    generate_quantity.as_ptr(),
                    raw_generate.len() as u32,
                    raw_destroy.as_ptr(),
                    destroy_quantity.as_ptr(),
                    raw_destroy.len() as u32,
                )
            })
        }
    }

    /// Transfers a quantity from one item instance to another.
    pub fn transfer_item_quantity(
        &self,
        source: ItemInstanceId,
        quantity: u32,
        dest: ItemInstanceId,
    ) -> Result<InventoryResult, SteamError> {
        let (source, dest) = (source.raw(), dest.raw());
        unsafe {
            self.start_result(|inv, out| {
                sys::SteamAPI_ISteamInventory_TransferItemQuantity(inv, out, source, quantity, dest)
            })
        }
    }

    /// Triggers an item drop from the given drop-list definition.
    ///
    /// Call [`Inventory::send_item_drop_heartbeat`] periodically while the
    /// drop list is active.
    pub fn trigger_item_drop(&self, drop_list: ItemDef) -> Result<InventoryResult, SteamError> {
        let raw = drop_list.raw();
        unsafe {
            self.start_result(|inv, out| {
                sys::SteamAPI_ISteamInventory_TriggerItemDrop(inv, out, raw)
            })
        }
    }

    /// Keeps the active item-drop session alive.
    pub fn send_item_drop_heartbeat(&self) {
        unsafe {
            sys::SteamAPI_ISteamInventory_SendItemDropHeartbeat(self.inv);
        }
    }

    /// Trades items with another user.
    ///
    /// `give`/`give_quantity` and `get`/`get_quantity` must be the same length.
    pub fn trade_items(
        &self,
        trade_partner: SteamId,
        give: &[ItemInstanceId],
        give_quantity: &[u32],
        get: &[ItemInstanceId],
        get_quantity: &[u32],
    ) -> Result<InventoryResult, SteamError> {
        if give.len() != give_quantity.len() || get.len() != get_quantity.len() {
            return Err(SteamError::InvalidParameter);
        }
        let raw_give: Vec<u64> = give.iter().map(|d| d.raw()).collect();
        let raw_get: Vec<u64> = get.iter().map(|d| d.raw()).collect();
        let partner = trade_partner.raw();
        unsafe {
            self.start_result(|inv, out| {
                sys::SteamAPI_ISteamInventory_TradeItems(
                    inv,
                    out,
                    partner,
                    raw_give.as_ptr(),
                    give_quantity.as_ptr(),
                    raw_give.len() as u32,
                    raw_get.as_ptr(),
                    get_quantity.as_ptr(),
                    raw_get.len() as u32,
                )
            })
        }
    }
}
