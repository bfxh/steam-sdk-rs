//! The lobby-list query builder: the `add_*_filter` / `set_*` chain that configures the
//! next `request_lobby_list`. Split from the types it accepts (`filters.rs`) because this
//! half is the one that crosses into Steam.

use super::*;

impl Matchmaking {
    /// Adds a string comparison filter to the lobby list request.
    ///
    /// This method adds a filter that compares a specific string attribute in lobbies
    /// with the provided value. Lobbies matching this criterion will be included in the result.
    ///
    /// # Arguments
    ///
    /// * `key`: The attribute key to compare.
    /// * `value`: The value to compare against.
    ///
    pub fn add_request_lobby_list_string_filter(
        &self,
        StringFilter(LobbyKey(key), value, kind): StringFilter,
    ) -> &Self {
        let key = CString::new(key).unwrap();
        let value = CString::new(value).unwrap();
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListStringFilter(
                self.mm,
                key.as_ptr(),
                value.as_ptr(),
                kind.into(),
            );
        }
        self
    }

    /// Adds a numerical comparison filter to the lobby list request.
    ///
    /// This method adds a filter that compares a specific numerical attribute in lobbies
    /// with the provided value. Lobbies matching this criterion will be included in the result.
    ///
    /// # Arguments
    ///
    /// * `key`: The attribute key to compare.
    /// * `value`: The value to compare against.
    ///
    pub fn add_request_lobby_list_numerical_filter(
        &self,
        NumberFilter(LobbyKey(key), value, comparison): NumberFilter,
    ) -> &Self {
        let key = CString::new(key).unwrap();
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListNumericalFilter(
                self.mm,
                key.as_ptr(),
                value,
                comparison.into(),
            );
        }
        self
    }

    /// Adds a near value filter to the lobby list request.
    ///
    /// This method adds a filter that sorts the lobby results based on their closeness
    /// to a specific value. No actual filtering is performed; lobbies are sorted based on proximity.
    ///
    /// # Arguments
    ///
    /// * `key`: The attribute key to use for sorting.
    /// * `value`: The reference value for sorting.
    ///
    pub fn add_request_lobby_list_near_value_filter(
        &self,
        NearFilter(LobbyKey(key), value): NearFilter,
    ) -> &Self {
        let key = CString::new(key).unwrap();
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListNearValueFilter(
                self.mm,
                key.as_ptr(),
                value,
            );
        }
        self
    }

    /// Adds a filter for available open slots to the lobby list request.
    ///
    /// This method adds a filter that includes lobbies having a specific number of open slots.
    ///
    /// # Arguments
    ///
    /// * `open_slots`: The number of open slots in a lobby to filter by.
    ///
    pub fn set_request_lobby_list_slots_available_filter(&self, open_slots: u8) -> &Self {
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListFilterSlotsAvailable(
                self.mm,
                open_slots as i32,
            );
        }
        self
    }

    /// Adds a distance filter to the lobby list request.
    ///
    /// This method adds a filter that includes lobbies within a certain distance criterion.
    ///
    /// # Arguments
    ///
    /// * `distance`: The `DistanceFilter` indicating the distance criterion for the filter.
    ///
    pub fn set_request_lobby_list_distance_filter(&self, distance: DistanceFilter) -> &Self {
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListDistanceFilter(
                self.mm,
                distance.into(),
            );
        }
        self
    }

    /// Adds a result count filter to the lobby list request.
    ///
    /// This method adds a filter to limit the number of lobby results returned by the request.
    ///
    /// # Arguments
    ///
    /// * `count`: The maximum number of lobby results to include in the response.
    ///
    pub fn set_request_lobby_list_result_count_filter(&self, count: u64) -> &Self {
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListResultCountFilter(
                self.mm,
                count as i32,
            );
        }
        self
    }

    /// Sets filters for the lobbies to be returned from [`request_lobby_list`].
    ///
    /// This method is used to apply various filters to the lobby list retrieval process.
    /// Call this method before calling `request_lobby_list` to ensure that the specified filters
    /// are taken into account when fetching the list of available lobbies.
    ///
    /// # Arguments
    ///
    /// * `filter`: A [`LobbyListFilter`] struct containing the filter criteria to be applied.
    ///
    /// [`request_lobby_list`]: #method.request_lobby_list
    /// [`LobbyListFilter`]: struct.LobbyListFilter.html
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use steamworks::*;
    /// fn main() {
    ///     let client = Client::init().unwrap();
    ///     client.matchmaking().set_lobby_list_filter(
    ///         LobbyListFilter {
    ///             string: Some(vec![
    ///                 StringFilter(
    ///                     LobbyKey::new("name"), "My Lobby", StringFilterKind::Equal
    ///                 ),
    ///                 StringFilter(
    ///                     LobbyKey::new("gamemode"), "ffa", StringFilterKind::Equal
    ///                 ),
    ///             ]),
    ///             number: Some(vec![
    ///                 NumberFilter(LobbyKey::new("elo"), 1500, ComparisonFilter::GreaterThan),
    ///                 NumberFilter(LobbyKey::new("elo"), 2000, ComparisonFilter::LessThan)
    ///             ]),
    ///             ..Default::default()
    ///         }
    ///     ).request_lobby_list(|lobbies| {
    ///         println!("Lobbies: {:?}", lobbies);
    ///     });
    /// }
    /// ```
    pub fn set_lobby_list_filter(&self, filter: LobbyListFilter<'_>) -> &Self {
        filter.string.into_iter().flatten().for_each(|str_filter| {
            self.add_request_lobby_list_string_filter(str_filter);
        });
        filter.number.into_iter().flatten().for_each(|num_filter| {
            self.add_request_lobby_list_numerical_filter(num_filter);
        });
        filter
            .near_value
            .into_iter()
            .flatten()
            .for_each(|near_filter| {
                self.add_request_lobby_list_near_value_filter(near_filter);
            });
        if let Some(distance) = filter.distance {
            self.set_request_lobby_list_distance_filter(distance);
        }
        if let Some(open_slots) = filter.open_slots {
            self.set_request_lobby_list_slots_available_filter(open_slots);
        }
        if let Some(count) = filter.count {
            self.set_request_lobby_list_result_count_filter(count);
        }
        self
    }
}
