//! The async result handle: `InventoryResult` (pollable, RAII, dropped back to Steam) and
//! the batch `InventoryUpdateHandle`, plus the one helper every request in this module
//! funnels through to get a result handle out of Steam.

use super::items::read_property_buffer;
use super::*;

/// RAII guard around an async inventory result handle.
///
/// Dropping the guard destroys the result server-side, so the handle stays
/// alive exactly as long as you can read items out of it.
pub struct InventoryResult {
    // `pub(super)`: the split moved the constructors that fill these into `mod.rs` (update
    // properties) and `definitions.rs` (deserialising a result from bytes).
    pub(super) inv: *mut sys::ISteamInventory,
    pub(super) handle: sys::SteamInventoryResult_t,
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
    // `pub(super)`: the split moved the constructors that fill these into `mod.rs` (update
    // properties) and `definitions.rs` (deserialising a result from bytes).
    pub(super) inv: *mut sys::ISteamInventory,
    pub(super) handle: sys::SteamInventoryUpdateHandle_t,
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

impl Inventory {
    /// Starts an async inventory request that reports its result handle.
    ///
    /// Steam returns `false` when the request could not be started at all
    /// (e.g. the user is not logged on).
    /// `pub(super)` because every request in this module funnels through it, and the split
    /// spread those requests over several files.
    pub(super) unsafe fn start_result(
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
}
