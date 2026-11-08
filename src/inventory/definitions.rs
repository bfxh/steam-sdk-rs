//! The item-schema side: loading definitions and reading a property off a definition, plus
//! rebuilding a result from bytes Steam handed to another client.

use super::items::read_property_buffer;
use super::*;
use std::os::raw::c_char;

impl Inventory {
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
            _inner: self._inner.clone(),
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
}
