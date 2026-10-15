//! Prices and purchases -- what an item costs, asking Steam for the price table, and
//! starting (then completing) a purchase.

use super::*;

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
}
