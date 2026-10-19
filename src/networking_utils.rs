use crate::networking_types::{NetworkingAvailabilityResult, NetworkingMessage};
use crate::utils::fixed_string;
use crate::{register_callback, Callback, Inner};
use std::convert::TryInto;
use std::ffi::{c_void, CStr};
use std::net::Ipv4Addr;
use std::sync::Arc;

use steamworks_sys as sys;

/// Access to the steam networking sockets interface
pub struct NetworkingUtils {
    pub(crate) utils: *mut sys::ISteamNetworkingUtils,
    pub(crate) inner: Arc<Inner>,
}

unsafe impl Send for NetworkingUtils {}
unsafe impl Sync for NetworkingUtils {}

impl NetworkingUtils {
    /// Allocate and initialize a message object.  Usually the reason
    /// you call this is to pass it to ISteamNetworkingSockets::SendMessages.
    /// The returned object will have all of the relevant fields cleared to zero.
    ///
    /// Optionally you can also request that this system allocate space to
    /// hold the payload itself.  If cbAllocateBuffer is nonzero, the system
    /// will allocate memory to hold a payload of at least cbAllocateBuffer bytes.
    /// m_pData will point to the allocated buffer, m_cbSize will be set to the
    /// size, and m_pfnFreeData will be set to the proper function to free up
    /// the buffer.
    ///
    /// If cbAllocateBuffer=0, then no buffer is allocated.  m_pData will be NULL,
    /// m_cbSize will be zero, and m_pfnFreeData will be NULL.  You will need to
    /// set each of these.
    pub fn allocate_message(&self, buffer_size: usize) -> NetworkingMessage {
        unsafe {
            let message =
                sys::SteamAPI_ISteamNetworkingUtils_AllocateMessage(self.utils, buffer_size as _);
            NetworkingMessage {
                message,
                _inner: self.inner.clone(),
            }
        }
    }

    /// If you know that you are going to be using the relay network (for example,
    /// because you anticipate making P2P connections), call this to initialize the
    /// relay network.  If you do not call this, the initialization will
    /// be delayed until the first time you use a feature that requires access
    /// to the relay network, which will delay that first access.
    ///
    /// You can also call this to force a retry if the previous attempt has failed.
    /// Performing any action that requires access to the relay network will also
    /// trigger a retry, and so calling this function is never strictly necessary,
    /// but it can be useful to call it a program launch time, if access to the
    /// relay network is anticipated.
    ///
    /// Use GetRelayNetworkStatus or listen for SteamRelayNetworkStatus_t
    /// callbacks to know when initialization has completed.
    /// Typically initialization completes in a few seconds.
    ///
    /// Note: dedicated servers hosted in known data centers do *not* need
    /// to call this, since they do not make routing decisions.  However, if
    /// the dedicated server will be using P2P functionality, it will act as
    /// a "client" and this should be called.
    pub fn init_relay_network_access(&self) {
        unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_InitRelayNetworkAccess(self.utils);
        }
    }

    /// Fetch current status of the relay network.
    ///
    /// If you want more detailed information use [`detailed_relay_network_status`](#method.detailed_relay_network_status) instead.
    pub fn relay_network_status(&self) -> NetworkingAvailabilityResult {
        unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_GetRelayNetworkStatus(
                self.utils,
                std::ptr::null_mut(),
            )
            .try_into()
        }
    }

    /// Fetch current detailed status of the relay network.
    pub fn detailed_relay_network_status(&self) -> RelayNetworkStatus {
        unsafe {
            let mut status = sys::SteamRelayNetworkStatus_t {
                m_eAvail: sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_Unknown,
                m_bPingMeasurementInProgress: 0,
                m_eAvailNetworkConfig:
                    sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_Unknown,
                m_eAvailAnyRelay:
                    sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_Unknown,
                m_debugMsg: [0; 256],
            };
            sys::SteamAPI_ISteamNetworkingUtils_GetRelayNetworkStatus(self.utils, &mut status);
            status.into()
        }
    }

    /// Register the callback for relay network status updates.
    ///
    /// Calling this more than once replaces the previous callback.
    pub fn relay_network_status_callback(
        &self,
        mut callback: impl FnMut(RelayNetworkStatus) + Send + 'static,
    ) {
        unsafe {
            std::mem::forget(register_callback(
                &self.inner,
                move |status: RelayNetworkStatusCallback| {
                    callback(status.status);
                },
            ));
        }
    }
}

#[derive(Debug)]
pub struct RelayNetworkStatus {
    availability: NetworkingAvailabilityResult,
    is_ping_measurement_in_progress: bool,
    network_config: NetworkingAvailabilityResult,
    any_relay: NetworkingAvailabilityResult,

    debugging_message: String,
}

impl RelayNetworkStatus {
    /// Summary status.  When this is "current", initialization has
    /// completed.  Anything else means you are not ready yet, or
    /// there is a significant problem.
    pub fn availability(&self) -> NetworkingAvailabilityResult {
        self.availability.clone()
    }

    /// True if latency measurement is in progress (or pending, awaiting a prerequisite).
    pub fn is_ping_measurement_in_progress(&self) -> bool {
        self.is_ping_measurement_in_progress
    }

    /// Status obtaining the network config.  This is a prerequisite
    /// for relay network access.
    ///
    /// Failure to obtain the network config almost always indicates
    /// a problem with the local internet connection.
    pub fn network_config(&self) -> NetworkingAvailabilityResult {
        self.network_config.clone()
    }

    /// Current ability to communicate with ANY relay.  Note that
    /// the complete failure to communicate with any relays almost
    /// always indicates a problem with the local Internet connection.
    /// (However, just because you can reach a single relay doesn't
    /// mean that the local connection is in perfect health.)
    pub fn any_relay(&self) -> NetworkingAvailabilityResult {
        self.any_relay.clone()
    }

    /// Non-localized English language status.  For diagnostic/debugging
    /// purposes only.
    pub fn debugging_message(&self) -> &str {
        &self.debugging_message
    }
}

impl From<sys::SteamRelayNetworkStatus_t> for RelayNetworkStatus {
    fn from(status: steamworks_sys::SteamRelayNetworkStatus_t) -> Self {
        Self {
            availability: status.m_eAvail.try_into(),
            is_ping_measurement_in_progress: status.m_bPingMeasurementInProgress != 0,
            network_config: status.m_eAvailNetworkConfig.try_into(),
            any_relay: status.m_eAvailAnyRelay.try_into(),
            debugging_message: fixed_string(&status.m_debugMsg),
        }
    }
}

#[derive(Debug)]
/// The relay network status callback.
pub struct RelayNetworkStatusCallback {
    status: RelayNetworkStatus,
}

impl_callback!(cb: SteamRelayNetworkStatus_t => RelayNetworkStatusCallback {
    Self {
        status: cb.into(),
    }
});

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

    /// Which kind of FakeIP Steam considers this IPv4 address, if any.
    ///
    /// The number goes over in the order Steam documents for the same field elsewhere -- "Returns
    /// IP in host byte order (e.g. aa.bb.cc.dd as 0xaabbccdd)" (`steamnetworkingtypes.h:205`),
    /// produced by `fake_ip_probe_argument` and pinned there, because a swapped argument here is a
    /// quietly wrong answer about some other host.
    ///
    /// Steam bills this as a pure test on the number: "This function is fast; it just does some
    /// logical tests on the IP and does not need to do any lookup operations"
    /// (`isteamnetworkingutils.h:251-252`). So it answers for an address Steam has never heard of,
    /// which is the opposite of `GetRealIdentityForFakeIP` next to it in the same header -- that one
    /// is a lookup, and this crate does not wrap it.
    pub fn ipv4_fake_ip_type(&self, ip: Ipv4Addr) -> FakeIpType {
        fake_ip_type_from_raw(unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_GetIPv4FakeIPType(
                self.utils,
                fake_ip_probe_argument(ip),
            )
        })
    }
}

/// The number Steam's classifier takes for an IPv4 address.
///
/// Its own accessor documents the shape as "aa.bb.cc.dd as 0xaabbccdd"
/// (`steamnetworkingtypes.h:205`) -- most-significant octet first -- which is what
/// [`Ipv4Addr::to_bits`] produces. Kept as a function of this crate rather than the bare method
/// call at the FFI edge so that the pairing has a lock: reading the octets the other way round
/// asks Steam about a different host, and every answer still looks like a valid reply.
fn fake_ip_probe_argument(ip: Ipv4Addr) -> u32 {
    ip.to_bits()
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

/// How Steam classifies an address as one of its fake ones.
///
/// The four answers are Steam's, listed in the order it declares them, because that order is load
/// bearing -- see [`FakeIpType::is_fake`]. The purpose the header gives the whole scheme is that
/// "Fake IPs" are handed out "to make it easier to interface with older code that assumed all hosts
/// will have an IPv4 address" (`steamnetworkingtypes.h:181-182`).
///
/// The fifth declared value, `k_ESteamNetworkingFakeIPType__Force32Bit`, is not a classification: it
/// is the sentinel that fixes the width of the C enum, and it is what [`Self::Unrecognized`] reads
/// today. Whether the payload is `i32` is a bindings fact that differs by platform (the generated
/// layer emits `#[repr(i32)]` in `windows_bindings.rs` and `#[repr(u32)]` in the three other binding
/// files), so this wrapper hands over the number it was given as the width it can always say.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum FakeIpType {
    /// "Error, argument was not even an IP address, etc." (`steamnetworkingtypes.h:185`). Reachable
    /// from the entry point this crate wraps, which takes a bare number; the header only promises
    /// never to return this for the *struct* form ("This function never returns" that value, `:244`).
    Invalid,
    /// "Argument was a valid IP, but was not from the reserved" fake range
    /// (`steamnetworkingtypes.h:186`) -- an ordinary address.
    NotFake,
    /// "Globally unique (for a given app) IPv4 address" (`steamnetworkingtypes.h:187`), with the
    /// address space managed by Steam.
    GlobalIpv4,
    /// "Locally unique IPv4 address" (`steamnetworkingtypes.h:188`): allocated by this process, and
    /// Steam's own warning on it is "For internal use only; should not be shared!"
    LocalIpv4,
    /// A number the generated layer has no name for. Kept as a value rather than folded into
    /// [`Self::Invalid`] because the two ask for opposite things: `Invalid` is Steam saying the
    /// argument was not an address, while this is Steam having changed its answer.
    Unrecognized(i32),
}

impl FakeIpType {
    /// Whether Steam would treat the address as a fake one.
    ///
    /// This is Steam's rule, which it writes as a comparison on the enum's own order:
    /// "GetIPv4FakeIPType( nIPv4 ) > k_ESteamNetworkingFakeIPType_NotFake"
    /// (`isteamnetworkingutils.h:253`), the same line the struct form uses
    /// ("GetFakeIPType() > k_ESteamNetworkingFakeIPType_NotFake", `steamnetworkingtypes.h:249`). So
    /// the two listed kinds are the only true answers, and [`Self::Unrecognized`] is false: a number
    /// this table has no name for is not evidence that an address is fake.
    pub fn is_fake(self) -> bool {
        matches!(self, Self::GlobalIpv4 | Self::LocalIpv4)
    }
}

/// The classification table, apart from the call that needs a live interface.
pub(crate) fn fake_ip_type_from_raw(raw: sys::ESteamNetworkingFakeIPType) -> FakeIpType {
    use sys::ESteamNetworkingFakeIPType::*;
    match raw {
        k_ESteamNetworkingFakeIPType_Invalid => FakeIpType::Invalid,
        k_ESteamNetworkingFakeIPType_NotFake => FakeIpType::NotFake,
        k_ESteamNetworkingFakeIPType_GlobalIPv4 => FakeIpType::GlobalIpv4,
        k_ESteamNetworkingFakeIPType_LocalIPv4 => FakeIpType::LocalIpv4,
        other => FakeIpType::Unrecognized(other as i32),
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use std::time::Duration;

    use serial_test::serial;

    #[test]
    #[serial]
    #[ignore = "requires a running Steam client"]
    fn test_get_networking_status() {
        let client = Client::init().unwrap();
        let callback_client = client.clone();
        std::thread::spawn(move || callback_client.run_callbacks());

        let utils = client.networking_utils();
        let status = utils.detailed_relay_network_status();
        println!(
            "status: {:?}, network_config: {:?}, any_relay: {:?}, message: {}",
            status.availability(),
            status.network_config(),
            status.any_relay(),
            status.debugging_message()
        );

        utils.init_relay_network_access();

        let status = utils.detailed_relay_network_status();
        println!(
            "status: {:?}, network_config: {:?}, any_relay: {:?}, message: {}",
            status.availability(),
            status.network_config(),
            status.any_relay(),
            status.debugging_message()
        );

        std::thread::sleep(Duration::from_millis(500));

        let status = utils.detailed_relay_network_status();
        println!(
            "status: {:?}, network_config: {:?}, any_relay: {:?}, message: {}",
            status.availability(),
            status.network_config(),
            status.any_relay(),
            status.debugging_message()
        );
    }
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

#[cfg(test)]
mod fake_ip_tests {
    //! The FakeIP classifier without a client: what is pinned here is the table and the one rule
    //! that reads it, both of which decide how a caller treats an address.

    use super::{fake_ip_type_from_raw, FakeIpType as Kind};
    use std::net::Ipv4Addr;
    use steamworks_sys as sys;
    use sys::ESteamNetworkingFakeIPType as Raw;

    #[test]
    fn every_classification_steam_declares_has_a_name_of_its_own() {
        let pairs = [
            (Raw::k_ESteamNetworkingFakeIPType_Invalid, Kind::Invalid),
            (Raw::k_ESteamNetworkingFakeIPType_NotFake, Kind::NotFake),
            (
                Raw::k_ESteamNetworkingFakeIPType_GlobalIPv4,
                Kind::GlobalIpv4,
            ),
            (Raw::k_ESteamNetworkingFakeIPType_LocalIPv4, Kind::LocalIpv4),
        ];
        for (raw, named) in pairs {
            assert_eq!(fake_ip_type_from_raw(raw), named, "{:?}", raw as i32);
        }
        assert_eq!(pairs.len(), 4, "the header declares four classifications");
        // Folding the declared zero into the fallback would keep `is_fake` correct and lose the
        // answer to "was my argument not even an address", which is the pair the header separates.
        assert_ne!(
            fake_ip_type_from_raw(Raw::k_ESteamNetworkingFakeIPType_Invalid),
            Kind::Unrecognized(0)
        );
    }

    /// The declared value that is not a classification is reachable today without inventing an
    /// illegal discriminant, which is what makes the fallback arm testable at all.
    #[test]
    fn the_width_sentinel_lands_on_the_fallback_and_keeps_its_number() {
        let read = fake_ip_type_from_raw(Raw::k_ESteamNetworkingFakeIPType__Force32Bit);
        assert_eq!(read, Kind::Unrecognized(0x7fff_ffff));
        assert!(
            !read.is_fake(),
            "an unnamed number is not evidence of a fake address"
        );
    }

    /// `is_fake` is a restatement of a comparison, so the lock is the comparison itself: run Steam's
    /// own rule on the raw value and require the two to agree for every declared classification.
    #[test]
    fn is_fake_agrees_with_the_comparison_steam_writes() {
        for raw in [
            Raw::k_ESteamNetworkingFakeIPType_Invalid,
            Raw::k_ESteamNetworkingFakeIPType_NotFake,
            Raw::k_ESteamNetworkingFakeIPType_GlobalIPv4,
            Raw::k_ESteamNetworkingFakeIPType_LocalIPv4,
        ] {
            let steam_says = raw as i32 > Raw::k_ESteamNetworkingFakeIPType_NotFake as i32;
            assert_eq!(
                fake_ip_type_from_raw(raw).is_fake(),
                steam_says,
                "{:?} disagrees with the header's rule",
                raw as i32
            );
        }
        assert!(Kind::GlobalIpv4.is_fake() && Kind::LocalIpv4.is_fake());
        assert!(!Kind::Invalid.is_fake() && !Kind::NotFake.is_fake());
    }

    /// The number handed to `GetIPv4FakeIPType` is the address written most-significant octet
    /// first, per "aa.bb.cc.dd as 0xaabbccdd" (`steamnetworkingtypes.h:205`). The mutation this
    /// locks is `u32::from_le_bytes(ip.octets())`, which asks Steam about a different host and still
    /// gets back an answer that looks well-formed.
    #[test]
    fn the_octets_go_over_as_the_order_the_header_documents() {
        use super::fake_ip_probe_argument;
        assert_eq!(
            fake_ip_probe_argument(Ipv4Addr::new(1, 2, 3, 4)),
            0x0102_0304
        );
        assert_eq!(
            fake_ip_probe_argument(Ipv4Addr::new(203, 0, 113, 7)),
            0xcb00_7107
        );
        assert_ne!(
            fake_ip_probe_argument(Ipv4Addr::new(1, 2, 3, 4)),
            u32::from_le_bytes(Ipv4Addr::new(1, 2, 3, 4).octets()),
            "the reversed order is the defect, not an alternative reading"
        );
    }
}
