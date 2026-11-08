//! The three notifications `ISteamInventory` posts: a result is ready, the whole inventory
//! changed, or the item definitions changed.

use super::*;

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
