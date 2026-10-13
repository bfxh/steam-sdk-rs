use super::*;

use std::os::raw::c_char;

/// Access to the steam inventory interface
pub struct Inventory {
    pub(crate) inv: *mut sys::ISteamInventory,
    pub(crate) _inner: Arc<Inner>,
}

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
/// Widens to `i32` rather than narrowing the flag: the item's flag field is
/// only 16 bits, so casting the constant down would silently truncate it if
/// Steam ever assigns a value above `u16::MAX`.
fn has_flag(item_flags: u16, flag: sys::ESteamItemFlags) -> bool {
    i32::from(item_flags) & flag.0 != 0
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
unsafe fn read_property_buffer(
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

/// RAII guard around an async inventory result handle.
///
/// Dropping the guard destroys the result server-side, so the handle stays
/// alive exactly as long as you can read items out of it.
pub struct InventoryResult {
    inv: *mut sys::ISteamInventory,
    handle: sys::SteamInventoryResult_t,
}

impl InventoryResult {
    /// Checks whether the async operation has finished.
    ///
    /// Returns `false` while the request is still pending.
    pub fn is_ready(&self) -> bool {
        unsafe {
            sys::SteamAPI_ISteamInventory_GetResultStatus(self.inv, self.handle)
                != sys::EResult::k_EResultPending
        }
    }

    /// Returns the status of the async operation.
    ///
    /// `Ok(())` means the operation succeeded and items are available
    /// through [`InventoryResult::items`].
    pub fn status(&self) -> SteamResult {
        unsafe {
            to_steam_result(sys::SteamAPI_ISteamInventory_GetResultStatus(
                self.inv,
                self.handle,
            ))
        }
    }

    /// Returns the items in this result.
    ///
    /// Returns `Err` if the underlying operation failed; see
    /// [`InventoryResult::status`] for the specific cause.
    pub fn items(&self) -> Result<Vec<ItemDetail>, SteamError> {
        self.status()?;
        unsafe {
            let mut size: sys::uint32 = 0;
            // A null output buffer makes Steam report the required size.
            if !sys::SteamAPI_ISteamInventory_GetResultItems(
                self.inv,
                self.handle,
                std::ptr::null_mut(),
                &mut size,
            ) {
                return Err(SteamError::IOFailure);
            }
            if size == 0 {
                return Ok(Vec::new());
            }
            let mut buf = vec![std::mem::zeroed(); size as usize];
            let mut written = size;
            if !sys::SteamAPI_ISteamInventory_GetResultItems(
                self.inv,
                self.handle,
                buf.as_mut_ptr(),
                &mut written,
            ) {
                return Err(SteamError::IOFailure);
            }
            buf.truncate(written as usize);
            Ok(buf
                .into_iter()
                .map(|item| ItemDetail {
                    item_id: ItemInstanceId(item.m_itemId),
                    definition: ItemDef(item.m_iDefinition),
                    quantity: item.m_unQuantity,
                    flags: item.m_unFlags,
                })
                .collect())
        }
    }

    /// Returns a string property of the item at `index` in this result.
    ///
    /// Property names are Steam-defined strings such as `"name"` or `"type"`.
    /// `Ok(None)` means the property is absent or empty.
    pub fn item_property(&self, index: u32, property: &str) -> Result<Option<String>, SteamError> {
        let cproperty = CString::new(property).map_err(|_| SteamError::InvalidParameter)?;
        unsafe {
            read_property_buffer(|buf, size| {
                sys::SteamAPI_ISteamInventory_GetResultItemProperty(
                    self.inv,
                    self.handle,
                    index,
                    cproperty.as_ptr(),
                    buf,
                    size,
                )
            })
        }
    }

    /// Returns the time at which this result was generated, in Unix seconds.
    pub fn timestamp(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamInventory_GetResultTimestamp(self.inv, self.handle) }
    }

    /// Checks that the result belongs to the expected user.
    ///
    /// Call this before trusting the contents of a result that came from
    /// [`InventoryResult::serialize`] / [`Inventory::deserialize_result`].
    pub fn check_steam_id(&self, expected: SteamId) -> bool {
        unsafe {
            sys::SteamAPI_ISteamInventory_CheckResultSteamID(self.inv, self.handle, expected.raw())
        }
    }

    /// Serializes this result into a byte buffer for persistence.
    ///
    /// Restore it with [`Inventory::deserialize_result`].
    pub fn serialize(&self) -> Result<Vec<u8>, SteamError> {
        unsafe {
            let mut size: sys::uint32 = 0;
            if !sys::SteamAPI_ISteamInventory_SerializeResult(
                self.inv,
                self.handle,
                std::ptr::null_mut(),
                &mut size,
            ) {
                return Err(SteamError::IOFailure);
            }
            let mut buf = vec![0u8; size as usize];
            let mut written = size;
            if !sys::SteamAPI_ISteamInventory_SerializeResult(
                self.inv,
                self.handle,
                buf.as_mut_ptr().cast(),
                &mut written,
            ) {
                return Err(SteamError::IOFailure);
            }
            buf.truncate(written as usize);
            Ok(buf)
        }
    }

    /// Returns the raw Steam result handle.
    pub fn raw(&self) -> sys::SteamInventoryResult_t {
        self.handle
    }

    /// Releases the handle without destroying the underlying result.
    ///
    /// # Safety
    ///
    /// The caller must destroy the returned handle with
    /// `SteamAPI_ISteamInventory_DestroyResult`, or the result leaks.
    pub unsafe fn into_raw(self) -> sys::SteamInventoryResult_t {
        let handle = self.handle;
        std::mem::forget(self);
        handle
    }
}

impl Drop for InventoryResult {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamInventory_DestroyResult(self.inv, self.handle);
        }
    }
}

/// Handle to a batch of item property updates that has not been submitted yet.
///
/// Created by [`Inventory::start_update_properties`]. Queue changes with the
/// `set_property_*` / `remove_property` methods, then apply them with
/// [`InventoryUpdateHandle::submit`].
pub struct InventoryUpdateHandle {
    inv: *mut sys::ISteamInventory,
    handle: sys::SteamInventoryUpdateHandle_t,
}

impl InventoryUpdateHandle {
    fn check_property(property: &str) -> Result<CString, SteamError> {
        CString::new(property).map_err(|_| SteamError::InvalidParameter)
    }

    /// Removes a property from an item.
    pub fn remove_property(&self, item: ItemInstanceId, property: &str) -> Result<(), SteamError> {
        let property = Self::check_property(property)?;
        unsafe {
            if !sys::SteamAPI_ISteamInventory_RemoveProperty(
                self.inv,
                self.handle,
                item.raw(),
                property.as_ptr(),
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Sets a string property on an item.
    pub fn set_property_string(
        &self,
        item: ItemInstanceId,
        property: &str,
        value: &str,
    ) -> Result<(), SteamError> {
        let property = Self::check_property(property)?;
        let value = CString::new(value).map_err(|_| SteamError::InvalidParameter)?;
        unsafe {
            if !sys::SteamAPI_ISteamInventory_SetPropertyString(
                self.inv,
                self.handle,
                item.raw(),
                property.as_ptr(),
                value.as_ptr(),
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Sets a boolean property on an item.
    pub fn set_property_bool(
        &self,
        item: ItemInstanceId,
        property: &str,
        value: bool,
    ) -> Result<(), SteamError> {
        let property = Self::check_property(property)?;
        unsafe {
            if !sys::SteamAPI_ISteamInventory_SetPropertyBool(
                self.inv,
                self.handle,
                item.raw(),
                property.as_ptr(),
                value,
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Sets an integer property on an item.
    pub fn set_property_i64(
        &self,
        item: ItemInstanceId,
        property: &str,
        value: i64,
    ) -> Result<(), SteamError> {
        let property = Self::check_property(property)?;
        unsafe {
            if !sys::SteamAPI_ISteamInventory_SetPropertyInt64(
                self.inv,
                self.handle,
                item.raw(),
                property.as_ptr(),
                value,
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Sets a float property on an item.
    pub fn set_property_f32(
        &self,
        item: ItemInstanceId,
        property: &str,
        value: f32,
    ) -> Result<(), SteamError> {
        let property = Self::check_property(property)?;
        unsafe {
            if !sys::SteamAPI_ISteamInventory_SetPropertyFloat(
                self.inv,
                self.handle,
                item.raw(),
                property.as_ptr(),
                value,
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Submits all queued property updates.
    ///
    /// Observe [`InventoryResultReady`] or poll [`InventoryResult::is_ready`]
    /// to learn when the update has been applied.
    pub fn submit(&self) -> Result<InventoryResult, SteamError> {
        let inv = self.inv;
        let handle = self.handle;
        unsafe {
            let mut result: sys::SteamInventoryResult_t = 0;
            if !sys::SteamAPI_ISteamInventory_SubmitUpdateProperties(inv, handle, &mut result) {
                return Err(SteamError::InvalidParameter);
            }
            Ok(InventoryResult {
                inv,
                handle: result,
            })
        }
    }
}

/// The three-letter currency code reported by [`Inventory::request_prices`]
/// (e.g. `"USD"`, `"EUR"`).
pub type PriceCurrency = String;

/// Details of a purchase started with [`Inventory::start_purchase`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PurchaseOrder {
    /// The order id of the purchase
    pub order_id: u64,
    /// The transaction id of the purchase
    pub transaction_id: u64,
}

impl Inventory {
    /// Starts an async inventory request that reports its result handle.
    ///
    /// Steam returns `false` when the request could not be started at all
    /// (e.g. the user is not logged on).
    unsafe fn start_result(
        &self,
        request: impl FnOnce(*mut sys::ISteamInventory, *mut sys::SteamInventoryResult_t) -> bool,
    ) -> Result<InventoryResult, SteamError> {
        let mut handle: sys::SteamInventoryResult_t = 0;
        if !request(self.inv, &mut handle) {
            return Err(SteamError::InvalidParameter);
        }
        Ok(InventoryResult {
            inv: self.inv,
            handle,
        })
    }

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

    /// Restores a previously serialized result.
    ///
    /// Verify ownership with [`InventoryResult::check_steam_id`] before using
    /// the restored contents.
    pub fn deserialize_result(&self, data: &[u8]) -> Result<InventoryResult, SteamError> {
        let mut handle: sys::SteamInventoryResult_t = 0;
        unsafe {
            // Steam requires this flag to be false; it is reserved for future use.
            if !sys::SteamAPI_ISteamInventory_DeserializeResult(
                self.inv,
                &mut handle,
                data.as_ptr().cast(),
                data.len() as u32,
                false,
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(InventoryResult {
            inv: self.inv,
            handle,
        })
    }

    /// Begins loading all item definitions.
    ///
    /// [`Inventory::get_item_definition_ids`] and
    /// [`Inventory::get_item_definition_property`] only work once the
    /// [`InventoryDefinitionUpdate`] callback has fired.
    pub fn load_item_definitions(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamInventory_LoadItemDefinitions(self.inv) }
    }

    /// Returns every loaded item definition id.
    pub fn get_item_definition_ids(&self) -> Result<Vec<ItemDef>, SteamError> {
        unsafe {
            let mut size: sys::uint32 = 0;
            if !sys::SteamAPI_ISteamInventory_GetItemDefinitionIDs(
                self.inv,
                std::ptr::null_mut(),
                &mut size,
            ) {
                return Err(SteamError::InvalidParameter);
            }
            if size == 0 {
                return Ok(Vec::new());
            }
            let mut buf = vec![0i32; size as usize];
            let mut written = size;
            if !sys::SteamAPI_ISteamInventory_GetItemDefinitionIDs(
                self.inv,
                buf.as_mut_ptr(),
                &mut written,
            ) {
                return Err(SteamError::IOFailure);
            }
            buf.truncate(written as usize);
            Ok(buf.into_iter().map(ItemDef).collect())
        }
    }

    /// Returns a string property of an item definition.
    ///
    /// Property names are Steam-defined strings such as `"name"` or `"type"`.
    /// `Ok(None)` means the property is absent or empty.
    pub fn get_item_definition_property(
        &self,
        def: ItemDef,
        property: &str,
    ) -> Result<Option<String>, SteamError> {
        let cproperty = CString::new(property).map_err(|_| SteamError::InvalidParameter)?;
        let raw = def.raw();
        unsafe {
            read_property_buffer(|buf, size| {
                sys::SteamAPI_ISteamInventory_GetItemDefinitionProperty(
                    self.inv,
                    raw,
                    cproperty.as_ptr(),
                    buf,
                    size,
                )
            })
        }
    }

    /// Reads a string property out of the fixed-size currency array of a
    /// Steam callback struct.
    pub(crate) unsafe fn currency_code(bytes: &[c_char]) -> String {
        CStr::from_ptr(bytes.as_ptr())
            .to_string_lossy()
            .into_owned()
    }

    /// Returns the number of items that have prices set.
    ///
    /// Only meaningful after [`Inventory::request_prices`] has succeeded.
    pub fn get_num_items_with_prices(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamInventory_GetNumItemsWithPrices(self.inv) }
    }

    /// Returns `(definition, current_price, base_price)` for priced items.
    ///
    /// Prices are in the smallest unit of the reported currency (e.g. cents).
    /// Only meaningful after [`Inventory::request_prices`] has succeeded.
    pub fn get_items_with_prices(&self) -> Result<Vec<(ItemDef, u64, u64)>, SteamError> {
        unsafe {
            let count = sys::SteamAPI_ISteamInventory_GetNumItemsWithPrices(self.inv);
            if count == 0 {
                return Ok(Vec::new());
            }
            let mut defs = vec![0i32; count as usize];
            let mut current = vec![0u64; count as usize];
            let mut base = vec![0u64; count as usize];
            if !sys::SteamAPI_ISteamInventory_GetItemsWithPrices(
                self.inv,
                defs.as_mut_ptr(),
                current.as_mut_ptr(),
                base.as_mut_ptr(),
                count,
            ) {
                return Err(SteamError::IOFailure);
            }
            Ok(defs
                .into_iter()
                .zip(current)
                .zip(base)
                .map(|((d, c), b)| (ItemDef(d), c, b))
                .collect())
        }
    }

    /// Returns the `(current_price, base_price)` of a single item.
    ///
    /// Only meaningful after [`Inventory::request_prices`] has succeeded.
    pub fn get_item_price(&self, def: ItemDef) -> Result<(u64, u64), SteamError> {
        let raw = def.raw();
        let mut current = 0u64;
        let mut base = 0u64;
        unsafe {
            if !sys::SteamAPI_ISteamInventory_GetItemPrice(self.inv, raw, &mut current, &mut base) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok((current, base))
    }

    /// Requests the latest item prices.
    ///
    /// The callback fires with the three-letter currency code on success.
    /// Prices are readable afterwards via [`Inventory::get_items_with_prices`].
    pub fn request_prices<F>(&self, callback: F)
    where
        F: FnOnce(Result<PriceCurrency, SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamInventory_RequestPrices(self.inv);
            register_call_result::<sys::SteamInventoryRequestPricesResult_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_result)
                            .map(|()| Inventory::currency_code(&v.m_rgchCurrency))
                    })
                },
            );
        }
    }

    /// Starts a purchase of the given item definitions.
    ///
    /// The callback fires with the order id once the user has completed (or
    /// cancelled) the Steam overlay purchase flow.
    pub fn start_purchase<F>(&self, items: &[(ItemDef, u32)], callback: F)
    where
        F: FnOnce(Result<PurchaseOrder, SteamError>) + 'static + Send,
    {
        let defs: Vec<i32> = items.iter().map(|(d, _)| d.raw()).collect();
        let quantities: Vec<u32> = items.iter().map(|(_, q)| *q).collect();
        unsafe {
            let api_call = sys::SteamAPI_ISteamInventory_StartPurchase(
                self.inv,
                defs.as_ptr(),
                quantities.as_ptr(),
                defs.len() as u32,
            );
            register_call_result::<sys::SteamInventoryStartPurchaseResult_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_result).map(|()| PurchaseOrder {
                            order_id: v.m_ulOrderID,
                            transaction_id: v.m_ulTransID,
                        })
                    })
                },
            );
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

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// An async inventory request finished
pub struct InventoryResultReady {
    /// The result handle that became ready
    pub handle: i32,
    /// The status of the finished operation
    pub result: Result<(), SteamError>,
}

impl_callback!(cb: SteamInventoryResultReady_t => InventoryResultReady {
    Self {
        handle: cb.m_handle,
        result: to_steam_result(cb.m_result),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A full update of the user's inventory was received
pub struct InventoryFullUpdate {
    /// The result handle containing the updated inventory
    pub handle: i32,
}

impl_callback!(cb: SteamInventoryFullUpdate_t => InventoryFullUpdate {
    Self {
        handle: cb.m_handle,
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// Item definitions finished loading
pub struct InventoryDefinitionUpdate {}

impl_callback!(_cb: SteamInventoryDefinitionUpdate_t => InventoryDefinitionUpdate {
    Self {}
});

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
