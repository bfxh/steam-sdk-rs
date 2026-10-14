use super::*;

use std::os::raw::c_char;

/// Access to the steam parties interface
///
/// A "party" is a lobby-like grouping advertised through a *beacon*: the host
/// creates a beacon, prospective joiners see it, their arrival is reported by
/// [`PartyReservationNotification`], and the host either accepts with
/// [`Parties::on_reservation_completed`] or declines with
/// [`Parties::cancel_reservation`].
pub struct Parties {
    pub(crate) parties: *mut sys::ISteamParties,
    pub(crate) _inner: Arc<Inner>,
}

/// Identifier of a beacon
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BeaconId(pub(crate) u64);

impl BeaconId {
    /// Creates a `BeaconId` from a raw 64 bit value.
    pub fn from_raw(id: u64) -> BeaconId {
        BeaconId(id)
    }

    /// Returns the raw 64 bit value.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

/// A place a beacon can be advertised.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BeaconLocationKind {
    /// A Steam chat group
    ChatGroup,
    /// Anything Steam reports that this build does not know
    Unknown,
}

/// A beacon location: a kind plus a Steam-assigned id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BeaconLocation {
    pub kind: BeaconLocationKind,
    pub id: u64,
}

impl BeaconLocation {
    fn from_raw(raw: sys::SteamPartyBeaconLocation_t) -> Self {
        BeaconLocation {
            kind: match raw.m_eType {
                sys::ESteamPartyBeaconLocationType::k_ESteamPartyBeaconLocationType_ChatGroup => {
                    BeaconLocationKind::ChatGroup
                }
                _ => BeaconLocationKind::Unknown,
            },
            id: raw.m_ulLocationID,
        }
    }

    fn to_raw(self) -> sys::SteamPartyBeaconLocation_t {
        sys::SteamPartyBeaconLocation_t {
            m_eType: match self.kind {
                BeaconLocationKind::ChatGroup => {
                    sys::ESteamPartyBeaconLocationType::k_ESteamPartyBeaconLocationType_ChatGroup
                }
                BeaconLocationKind::Unknown => {
                    sys::ESteamPartyBeaconLocationType::k_ESteamPartyBeaconLocationType_Invalid
                }
            },
            m_ulLocationID: self.id,
        }
    }
}

/// Which string to ask Steam for about a location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BeaconLocationData {
    Name,
    IconUrlSmall,
    IconUrlMedium,
    IconUrlLarge,
}

impl BeaconLocationData {
    fn as_raw(self) -> sys::ESteamPartyBeaconLocationData {
        match self {
            BeaconLocationData::Name => {
                sys::ESteamPartyBeaconLocationData::k_ESteamPartyBeaconLocationDataName
            }
            BeaconLocationData::IconUrlSmall => {
                sys::ESteamPartyBeaconLocationData::k_ESteamPartyBeaconLocationDataIconURLSmall
            }
            BeaconLocationData::IconUrlMedium => {
                sys::ESteamPartyBeaconLocationData::k_ESteamPartyBeaconLocationDataIconURLMedium
            }
            BeaconLocationData::IconUrlLarge => {
                sys::ESteamPartyBeaconLocationData::k_ESteamPartyBeaconLocationDataIconURLLarge
            }
        }
    }
}

/// Details of one beacon, as reported by [`Parties::beacon_details`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeaconDetails {
    /// The user advertising the party
    pub owner: SteamId,
    /// Where it is advertised
    pub location: BeaconLocation,
    /// The host's application-defined payload, if any
    pub metadata: Option<String>,
}

/// Buffer size used when Steam does not report a needed length itself.
///
/// `GetBeaconDetails` and `GetBeaconLocationData` take a caller-owned buffer
/// and give no size hint back, so this crate picks a fixed generous size and
/// reports `None` when Steam declines to fill it.
pub const STRING_BUFFER_CAPACITY: usize = 1024;

fn cstr(value: &str) -> Result<CString, SteamError> {
    CString::new(value).map_err(|_| SteamError::InvalidParameter)
}

/// Reads a NUL-terminated string Steam writes into a caller-owned buffer.
///
/// Returns `Ok(None)` when the call succeeded but produced no text.
unsafe fn read_fixed_string(
    mut fill: impl FnMut(*mut c_char, ::std::os::raw::c_int) -> bool,
) -> Result<Option<String>, SteamError> {
    let mut buf = vec![0 as c_char; STRING_BUFFER_CAPACITY];
    if !fill(buf.as_mut_ptr(), STRING_BUFFER_CAPACITY as i32) {
        return Ok(None);
    }
    let text = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

impl Parties {
    /// Number of beacons this user is currently advertising.
    pub fn num_active_beacons(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamParties_GetNumActiveBeacons(self.parties) }
    }

    /// Beacon id at `index`, or `None` when the index is out of range.
    pub fn beacon_by_index(&self, index: u32) -> Option<BeaconId> {
        let id = unsafe { sys::SteamAPI_ISteamParties_GetBeaconByIndex(self.parties, index) };
        // Steam uses 0 for "no such beacon".
        if id == 0 {
            None
        } else {
            Some(BeaconId(id))
        }
    }

    /// All beacons this user is advertising.
    pub fn active_beacons(&self) -> Vec<BeaconId> {
        (0..self.num_active_beacons())
            .filter_map(|i| self.beacon_by_index(i))
            .collect()
    }

    /// Reads a beacon's owner, location and metadata payload.
    ///
    /// Returns `Err` when Steam no longer knows the id.
    pub fn beacon_details(&self, beacon: BeaconId) -> Result<BeaconDetails, SteamError> {
        let mut owner = sys::CSteamID {
            m_steamid: sys::CSteamID_SteamID_t { m_unAll64Bits: 0 },
        };
        let mut location = sys::SteamPartyBeaconLocation_t {
            m_eType: sys::ESteamPartyBeaconLocationType::k_ESteamPartyBeaconLocationType_Invalid,
            m_ulLocationID: 0,
        };
        let mut metadata = vec![0 as c_char; STRING_BUFFER_CAPACITY];
        let ok = unsafe {
            sys::SteamAPI_ISteamParties_GetBeaconDetails(
                self.parties,
                beacon.raw(),
                &mut owner,
                &mut location,
                metadata.as_mut_ptr(),
                STRING_BUFFER_CAPACITY as i32,
            )
        };
        if !ok {
            return Err(SteamError::InvalidParameter);
        }
        let text = unsafe {
            CStr::from_ptr(metadata.as_ptr())
                .to_string_lossy()
                .into_owned()
        };
        Ok(BeaconDetails {
            owner: SteamId::from_raw(unsafe { owner.m_steamid.m_unAll64Bits }),
            location: BeaconLocation::from_raw(location),
            metadata: if text.is_empty() { None } else { Some(text) },
        })
    }

    /// Creates a beacon.
    ///
    /// The callback reports the new [`BeaconId`] once Steam has accepted it.
    pub fn create_beacon<F>(
        &self,
        open_slots: u32,
        location: &BeaconLocation,
        connect_string: &str,
        metadata: &str,
        callback: F,
    ) -> Result<(), SteamError>
    where
        F: FnOnce(Result<BeaconId, SteamError>) + 'static + Send,
    {
        let connect_string = cstr(connect_string)?;
        let metadata = cstr(metadata)?;
        let mut raw_location = location.to_raw();
        unsafe {
            let api_call = sys::SteamAPI_ISteamParties_CreateBeacon(
                self.parties,
                open_slots,
                &mut raw_location,
                connect_string.as_ptr(),
                metadata.as_ptr(),
            );
            register_call_result::<sys::CreateBeaconCallback_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult).map(|()| BeaconId(v.m_ulBeaconID))
                    });
                },
            );
        }
        Ok(())
    }

    /// Requests to join a beacon.
    ///
    /// On success the callback carries the host's connect string, which the
    /// caller uses to reach the host through its own transport.
    pub fn join_party<F>(&self, beacon: BeaconId, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<JoinTicket, SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamParties_JoinParty(self.parties, beacon.raw());
            register_call_result::<sys::JoinPartyCallback_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult).map(|()| JoinTicket {
                            beacon: BeaconId(v.m_ulBeaconID),
                            owner: SteamId::from_raw(
                                v.m_SteamIDBeaconOwner.m_steamid.m_unAll64Bits,
                            ),
                            connect_string: fixed_string(&v.m_rgchConnectString),
                        })
                    });
                },
            );
        }
        Ok(())
    }

    /// Accepts a would-be joiner, telling Steam to stop reserving slots for them.
    pub fn on_reservation_completed(&self, beacon: BeaconId, joiner: SteamId) {
        unsafe {
            sys::SteamAPI_ISteamParties_OnReservationCompleted(
                self.parties,
                beacon.raw(),
                joiner.raw(),
            );
        }
    }

    /// Declines a would-be joiner.
    pub fn cancel_reservation(&self, beacon: BeaconId, joiner: SteamId) {
        unsafe {
            sys::SteamAPI_ISteamParties_CancelReservation(self.parties, beacon.raw(), joiner.raw());
        }
    }

    /// Changes how many slots a beacon advertises.
    pub fn change_num_open_slots<F>(
        &self,
        beacon: BeaconId,
        open_slots: u32,
        callback: F,
    ) -> Result<(), SteamError>
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamParties_ChangeNumOpenSlots(
                self.parties,
                beacon.raw(),
                open_slots,
            );
            register_call_result::<sys::ChangeNumOpenSlotsCallback_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult)
                    });
                },
            );
        }
        Ok(())
    }

    /// Stops advertising a beacon.
    ///
    /// Returns `false` when Steam does not know the id.
    pub fn destroy_beacon(&self, beacon: BeaconId) -> bool {
        unsafe { sys::SteamAPI_ISteamParties_DestroyBeacon(self.parties, beacon.raw()) }
    }

    /// Locations Steam allows beacons to be advertised in.
    ///
    /// Steam reports the count separately, so this asks for it first and then
    /// sizes the list to match.
    pub fn available_locations(&self) -> Result<Vec<BeaconLocation>, SteamError> {
        let mut count: sys::uint32 = 0;
        unsafe {
            if !sys::SteamAPI_ISteamParties_GetNumAvailableBeaconLocations(self.parties, &mut count)
            {
                return Err(SteamError::IOFailure);
            }
            if count == 0 {
                return Ok(Vec::new());
            }
            let mut list = vec![
                sys::SteamPartyBeaconLocation_t {
                    m_eType:
                        sys::ESteamPartyBeaconLocationType::k_ESteamPartyBeaconLocationType_Invalid,
                    m_ulLocationID: 0,
                };
                count as usize
            ];
            if !sys::SteamAPI_ISteamParties_GetAvailableBeaconLocations(
                self.parties,
                list.as_mut_ptr(),
                count,
            ) {
                return Err(SteamError::IOFailure);
            }
            Ok(list.into_iter().map(BeaconLocation::from_raw).collect())
        }
    }

    /// A display string for a location, such as its name or an icon URL.
    pub fn location_data(
        &self,
        location: &BeaconLocation,
        data: BeaconLocationData,
    ) -> Result<Option<String>, SteamError> {
        let raw = location.to_raw();
        let kind = data.as_raw();
        unsafe {
            read_fixed_string(|buf, len| {
                sys::SteamAPI_ISteamParties_GetBeaconLocationData(self.parties, raw, kind, buf, len)
            })
        }
    }
}

/// Reads a fixed-size char array Steam embeds in a callback struct.
fn fixed_string(bytes: &[c_char]) -> String {
    // Steam NUL-pads the array, so stop at the first terminator.
    let len = bytes.iter().take_while(|c| **c != 0).count();
    let raw = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), len) };
    String::from_utf8_lossy(raw).into_owned()
}

/// What [`Parties::join_party`] delivers on success.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinTicket {
    /// The beacon joined
    pub beacon: BeaconId,
    /// The host advertising it
    pub owner: SteamId,
    /// Opaque host-defined string used to reach the host over your own transport
    pub connect_string: String,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// Somebody reserved a slot on one of this user's beacons
pub struct PartyReservationNotification {
    /// The beacon the reservation is for
    pub beacon: BeaconId,
    /// The user who reserved
    pub joiner: SteamId,
}

impl_callback!(cb: ReservationNotificationCallback_t => PartyReservationNotification {
    Self {
        beacon: BeaconId(cb.m_ulBeaconID),
        joiner: SteamId::from_raw(unsafe { cb.m_steamIDJoiner.m_steamid.m_unAll64Bits }),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// The list of locations Steam offers changed
pub struct AvailableBeaconLocationsUpdated;

impl_callback!(_cb: AvailableBeaconLocationsUpdated_t => AvailableBeaconLocationsUpdated {
    Self {}
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// This user's set of active beacons changed
pub struct ActiveBeaconsUpdated;

impl_callback!(_cb: ActiveBeaconsUpdated_t => ActiveBeaconsUpdated {
    Self {}
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_string_stops_at_the_first_nul() {
        let mut buf = [0 as c_char; 8];
        buf[0] = b'a' as c_char;
        buf[1] = b'b' as c_char;
        assert_eq!(fixed_string(&buf), "ab");

        // Steam pads the rest; nothing past the terminator may leak in.
        let empty = [0 as c_char; 8];
        assert_eq!(fixed_string(&empty), "");
    }

    #[test]
    fn location_kind_round_trips_only_for_known_types() {
        for kind in [BeaconLocationKind::ChatGroup, BeaconLocationKind::Unknown] {
            let loc = BeaconLocation { kind, id: 42 };
            let raw = loc.to_raw();
            assert_eq!(BeaconLocation::from_raw(raw), loc, "{kind:?} round trip");
        }
    }

    #[test]
    fn unknown_location_type_does_not_collide_with_chat_group() {
        // If both mapped to the same raw type, a future Steam location kind
        // would silently be treated as a chat group.
        let unknown = BeaconLocation {
            kind: BeaconLocationKind::Unknown,
            id: 1,
        }
        .to_raw();
        let chat = BeaconLocation {
            kind: BeaconLocationKind::ChatGroup,
            id: 1,
        }
        .to_raw();
        assert_ne!(unknown.m_eType, chat.m_eType);
    }

    #[test]
    fn location_data_selectors_are_distinct() {
        let all = [
            BeaconLocationData::Name,
            BeaconLocationData::IconUrlSmall,
            BeaconLocationData::IconUrlMedium,
            BeaconLocationData::IconUrlLarge,
        ];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(all[i].as_raw(), all[j].as_raw());
            }
        }
    }

    #[test]
    fn beacon_strings_reject_interior_nul() {
        assert!(matches!(cstr("ok"), Ok(_)));
        assert!(matches!(cstr("a\0b"), Err(SteamError::InvalidParameter)));
    }
}
