//! The two FakeIP questions Steam answers about an address: which kind of fake one it is, and whose
//! real identity sits behind it. See `crate::networking_sockets::fake_ip` for the allocation side.

use super::NetworkingUtils;
use crate::networking_types::{NetworkingIdentity, SteamIpAddr};
use std::net::{Ipv4Addr, SocketAddr};

use steamworks_sys as sys;

impl NetworkingUtils {
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
    /// which is the opposite of the lookup next to it in the same header,
    /// [`NetworkingUtils::real_identity_for_fake_ip`] -- that one only answers for a FakeIP Steam
    /// actually assigned.
    pub fn ipv4_fake_ip_type(&self, ip: Ipv4Addr) -> FakeIpType {
        fake_ip_type_from_raw(unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_GetIPv4FakeIPType(
                self.utils,
                fake_ip_probe_argument(ip),
            )
        })
    }

    /// The identity Steam has behind a FakeIP address, when it still knows one.
    ///
    /// This is the lookup that [`NetworkingUtils::ipv4_fake_ip_type`] is not: Steam says of the
    /// classifier "it just does some logical tests on the IP and does not need to do any lookup
    /// operations" (`isteamnetworkingutils.h:251-252`), while this one consults what it has actually
    /// assigned. The answer is durable for a live connection -- "FakeIP's used by active connections,
    /// or the FakeIPs assigned to local identities, will always work" (`:262-263`) -- and not durable
    /// otherwise: Steam forgets FakeIPs to save space, though "you can read back the real identity
    /// of a connection very soon after it is destroyed" (`:265-266`).
    ///
    /// Both refusals arrive as [`crate::SteamError`] and neither is read as an identity:
    /// "k_EResultInvalidParam: the IP is not a FakeIP." and "k_EResultNoMatch: we don't recognize that
    /// FakeIP and don't know the corresponding identity." (`:259-260`). The out parameter is filled
    /// only on the success path because a struct Steam never wrote holds zeros, and zero is
    /// `k_ESteamNetworkingIdentityType_Invalid` -- the type that is not anyone's identity. The test
    /// `the_two_identity_lookup_refusals_keep_their_names` pins that the two refusals stay tellable
    /// apart; whether a given address has an answer needs a running client and an address Steam has
    /// actually assigned, so that half is registered as unverified rather than counted as covered.
    pub fn real_identity_for_fake_ip(
        &self,
        fake_ip: SocketAddr,
    ) -> Result<NetworkingIdentity, crate::SteamError> {
        let held = SteamIpAddr::from(fake_ip);
        let mut out = std::mem::MaybeUninit::<sys::SteamNetworkingIdentity>::zeroed();
        let code = unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_GetRealIdentityForFakeIP(
                self.utils,
                held.as_ptr(),
                out.as_mut_ptr(),
            )
        };
        crate::to_steam_result(code)?;
        Ok(NetworkingIdentity::from(unsafe { out.assume_init() }))
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
    /// "Error, argument was not even an IP address, etc." (`steamnetworkingtypes.h:185`). Steam
    /// promises only that the *struct* form never returns it ("This function never returns" that
    /// value, `:244`); for the bare-number entry point this crate wraps the header neither promises
    /// nor excludes it -- and since every `uint32` is an address, the sentence arguably points the
    /// other way. Whether Steam ever answers this for a number is registered as unverified: it needs
    /// a running client.
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
        // A row count cannot be reddened by Steam changing its list -- the four names are pinned
        // against the header by the citations on the variants and by
        // `is_fake_agrees_with_the_comparison_steam_writes`, which recomputes Steam's own rule. This
        // only stops a row being dropped from the table unnoticed.
        assert_eq!(pairs.len(), 4, "this table enumerates four classifications");
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

    /// The identity lookup names two refusals, "the IP is not a FakeIP" and "we don't recognize that
    /// FakeIP" (`isteamnetworkingutils.h:259-260`), and they are different facts: one indicts the
    /// argument, the other Steam's memory. Folding either into `Generic` makes the pair unreadable,
    /// so each is asserted on its own and then asserted to differ.
    #[test]
    fn the_two_identity_lookup_refusals_keep_their_names() {
        let not_a_fake_ip = crate::to_steam_result(sys::EResult::k_EResultInvalidParam);
        let forgotten = crate::to_steam_result(sys::EResult::k_EResultNoMatch);
        assert_eq!(not_a_fake_ip, Err(crate::SteamError::InvalidParameter));
        assert_eq!(forgotten, Err(crate::SteamError::NoMatch));
        assert_ne!(not_a_fake_ip, forgotten);
    }

    /// A premise pin rather than a lock on this crate's code: the wrapper reads the out parameter only
    /// after the status answers, and the reason that matters is that an untouched struct holds the
    /// invalid identity. If the generated layer ever stops numbering that as zero, the argument in the
    /// method's docs has to be re-read -- which is what this test is for.
    #[test]
    fn an_untouched_identity_struct_holds_the_type_that_is_nobody() {
        assert_eq!(
            sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_Invalid as i32,
            0,
            "zero is what the refused call leaves behind, so it must be the invalid type"
        );
    }
}
