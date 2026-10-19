//! Where Steam thinks a route is: the ping-location blob and its text form, the estimates between
//! them, and the point-of-presence queries the estimates are built on.

use super::NetworkingUtils;
use std::ffi::CStr;

use steamworks_sys as sys;

/// A location on the Internet that Steam can estimate a ping to.
///
/// The binding's own note is the rule for this type: "This object should only be used in the
/// same process! Do not serialize it, send it over the wire, or persist it in a file or
/// database! ... convert it to a string representation using the methods in
/// ISteamNetworkingUtils()". So the text form is the transportable one, and this type
/// deliberately has no `Serialize` implementation.
#[derive(Clone, Copy, Debug)]
pub struct SteamNetworkPingLocation(pub(crate) sys::SteamNetworkPingLocation_t);

// Steam's own struct is plain bytes without an equality impl; two locations are equal when
// every byte of the marker matches, which is what "same route characteristics" means here.
impl PartialEq for SteamNetworkPingLocation {
    fn eq(&self, other: &Self) -> bool {
        self.0.m_data == other.0.m_data
    }
}

impl Eq for SteamNetworkPingLocation {}

/// How many POP entries this wrapper will read back: Steam's own count, never negative and
/// never more than the buffer it was given.
fn pops_to_take(reported: i32, capacity: usize) -> usize {
    (reported.max(0) as usize).min(capacity)
}

impl NetworkingUtils {
    /// Location info for the current host, with the approximate age of that data in seconds.
    ///
    /// `None` is Steam's "no data is available yet" answer, which is normal soon after
    /// [`NetworkingUtils::init_relay_network_access`]: "It takes a few seconds to initialize
    /// access to the relay network." Otherwise the answer is Steam's own "most up-to-date
    /// information we have available right now, even if we are in the middle of re-calculating
    /// ping times."
    pub fn local_ping_location(&self) -> Option<(SteamNetworkPingLocation, f32)> {
        let mut location: sys::SteamNetworkPingLocation_t = unsafe { std::mem::zeroed() };
        let age = unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_GetLocalPingLocation(self.utils, &mut location)
        };
        if age < 0.0 {
            None
        } else {
            Some((SteamNetworkPingLocation(location), age))
        }
    }

    /// Estimated round-trip latency between two locations, in milliseconds.
    pub fn estimate_ping_time(
        &self,
        first: &SteamNetworkPingLocation,
        second: &SteamNetworkPingLocation,
    ) -> PingEstimate {
        PingEstimate::from_raw(unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_EstimatePingTimeBetweenTwoLocations(
                self.utils, &first.0, &second.0,
            )
        })
    }

    /// The same estimate, assuming one end is this host. Steam says it "is a bit faster,
    /// especially if you need to calculate a bunch of these in a loop to find the fastest one",
    /// and that it can differ slightly from combining [`NetworkingUtils::local_ping_location`]
    /// with [`NetworkingUtils::estimate_ping_time`] because it knows more about the route.
    pub fn estimate_ping_time_from_local_host(
        &self,
        remote: &SteamNetworkPingLocation,
    ) -> PingEstimate {
        PingEstimate::from_raw(unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_EstimatePingTimeFromLocalHost(self.utils, &remote.0)
        })
    }

    /// The wire form of a location.
    ///
    /// Steam warns that the format "is subject to change so please do not parse it yourself",
    /// so [`NetworkingUtils::parse_ping_location`] is the supported way back. The buffer is
    /// exactly `k_cchMaxSteamNetworkingPingLocationString`, which is what Steam requires.
    pub fn ping_location_text(&self, location: &SteamNetworkPingLocation) -> String {
        let mut buffer = [0i8; sys::k_cchMaxSteamNetworkingPingLocationString as usize];
        unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_ConvertPingLocationToString(
                self.utils,
                &location.0,
                buffer.as_mut_ptr(),
                buffer.len() as std::os::raw::c_int,
            );
            // Steam writes into the buffer we just handed it, whose length is the size Steam
            // documents as required, so the C string it leaves there cannot run past it.
            CStr::from_ptr(buffer.as_ptr())
                .to_string_lossy()
                .into_owned()
        }
    }

    /// Reads back a location produced by [`NetworkingUtils::ping_location_text`].
    ///
    /// `Ok(None)` means Steam "couldn't understand the string"; `Err(InvalidParameter)` means
    /// the text never reached Steam because it contains an interior nul.
    pub fn parse_ping_location(
        &self,
        text: &str,
    ) -> Result<Option<SteamNetworkPingLocation>, crate::SteamError> {
        let text = crate::app::c_string_arg(text)?;
        let mut location: sys::SteamNetworkPingLocation_t = unsafe { std::mem::zeroed() };
        let parsed = unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_ParsePingLocationString(
                self.utils,
                text.as_ptr(),
                &mut location,
            )
        };
        Ok(parsed.then_some(SteamNetworkPingLocation(location)))
    }

    /// Whether Steam's ping data is at least this fresh -- and if it is not, this starts a
    /// refresh. Steam asks that it be called only "when you *really* do need to force an
    /// immediate refresh", not "just in case"; the library refreshes on its own otherwise.
    ///
    /// The `false` answer means measurement has been started (if it was not already running),
    /// and [`NetworkingUtils::relay_network_status`] or the `SteamRelayNetworkStatus_t`
    /// callback says when it finishes.
    pub fn ping_data_up_to_date(&self, max_age_seconds: f32) -> bool {
        unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_CheckPingDataUpToDate(self.utils, max_age_seconds)
        }
    }

    /// Ping of the best relayed route to a Valve data centre, and the relay PoP it goes through.
    pub fn ping_to_data_center(&self, pop: u32) -> DataCenterPing {
        let mut via: sys::SteamNetworkingPOPID = 0;
        let ping = unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_GetPingToDataCenter(self.utils, pop, &mut via)
        };
        DataCenterPing {
            ping: PingEstimate::from_raw(ping),
            via_relay_pop: via,
        }
    }

    /// The *direct* ping to the relays at a data centre, as opposed to the routed estimate from
    /// [`NetworkingUtils::ping_to_data_center`].
    pub fn direct_ping_to_pop(&self, pop: u32) -> PingEstimate {
        PingEstimate::from_raw(unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_GetDirectPingToPOP(self.utils, pop)
        })
    }

    /// How many points of presence Steam's config knows about.
    pub fn pop_count(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamNetworkingUtils_GetPOPCount(self.utils).max(0) as u32 }
    }

    /// Every POP ID in the config.
    ///
    /// The list is sized from [`NetworkingUtils::pop_count`], and Steam's "number of entries
    /// that were filled into your list" is clamped to that size before it truncates, so a
    /// count larger than the buffer cannot read past it.
    pub fn pop_list(&self) -> Vec<u32> {
        let wanted = self.pop_count() as usize;
        if wanted == 0 {
            return Vec::new();
        }
        let mut list = vec![0u32; wanted];
        let filled = unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_GetPOPList(
                self.utils,
                list.as_mut_ptr(),
                wanted as std::os::raw::c_int,
            )
        };
        list.truncate(pops_to_take(filled, wanted));
        list
    }

    /// Steam's monotonic clock, in microseconds. "This timer has the following properties" --
    /// see `isteamnetworkingutils.h`; it is not a wall clock and does not go backwards.
    pub fn local_timestamp(&self) -> i64 {
        unsafe { sys::SteamAPI_ISteamNetworkingUtils_GetLocalTimestamp(self.utils) }
    }
}

/// What Steam said about a route's latency.
///
/// Steam names exactly two negative answers, declared as
/// `steamnetworkingtypes.h:1089-1090`: "const int k_nSteamNetworkingPing_Failed = -1;" and
/// "const int k_nSteamNetworkingPing_Unknown = -2;"; anything else
/// it returns is taken as a millisecond figure, because the header names no other sentinel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PingEstimate {
    /// Round-trip latency in milliseconds. Steam calls its own estimate "conservative" and "a
    /// reasonable upper bound on the ping time".
    Milliseconds(i32),
    /// "some networking difficulty. (Failure to ping, etc)"
    Failed,
    /// "we cannot currently answer the question for some other reason"
    Unknown,
}

impl PingEstimate {
    fn from_raw(raw: i32) -> Self {
        match raw {
            sys::k_nSteamNetworkingPing_Failed => Self::Failed,
            sys::k_nSteamNetworkingPing_Unknown => Self::Unknown,
            other => Self::Milliseconds(other),
        }
    }

    /// The milliseconds, if Steam answered. `None` for either named sentinel.
    pub fn milliseconds(self) -> Option<i32> {
        match self {
            Self::Milliseconds(ms) => Some(ms),
            _ => None,
        }
    }
}

/// The best relayed route to a Valve data centre.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataCenterPing {
    pub ping: PingEstimate,
    /// The relay PoP the route goes through, as Steam wrote it. The header documents only
    /// "Fetch ping time of best available relayed route from this host to the specified data
    /// center", so what Steam leaves here when it has no route is not specified -- reading 0
    /// means "Steam wrote 0", not "there is no relay".
    pub via_relay_pop: u32,
}

#[cfg(test)]
mod ping_tests {
    use super::*;

    #[test]
    fn the_two_sentinels_steam_names_get_variants_of_their_own() {
        assert_eq!(
            PingEstimate::from_raw(sys::k_nSteamNetworkingPing_Failed),
            PingEstimate::Failed
        );
        assert_eq!(
            PingEstimate::from_raw(sys::k_nSteamNetworkingPing_Unknown),
            PingEstimate::Unknown
        );
        assert_eq!(PingEstimate::from_raw(0), PingEstimate::Milliseconds(0));
        assert_eq!(PingEstimate::from_raw(42), PingEstimate::Milliseconds(42));
        // The header names no third negative answer, so anything else stays a number --
        // including an unexpected negative, which a caller can still see.
        assert_eq!(PingEstimate::from_raw(-3), PingEstimate::Milliseconds(-3));
    }

    #[test]
    fn milliseconds_answers_only_for_a_real_number() {
        assert_eq!(PingEstimate::from_raw(17).milliseconds(), Some(17));
        assert_eq!(PingEstimate::Failed.milliseconds(), None);
        assert_eq!(PingEstimate::Unknown.milliseconds(), None);
    }

    #[test]
    fn a_pop_list_never_takes_more_than_the_buffer_holds() {
        assert_eq!(pops_to_take(3, 10), 3);
        assert_eq!(pops_to_take(10, 10), 10);
        assert_eq!(
            pops_to_take(11, 10),
            10,
            "Steam's filled count is clamped to the list we allocated"
        );
        assert_eq!(
            pops_to_take(-1, 10),
            0,
            "a negative count must not wrap into a huge one"
        );
    }
}
