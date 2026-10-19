//! What Steam says about this server right now: whether it is logged on and to what standard,
//! whether the master server wants it restarted, the address Steam advertises for it, and whether a
//! given user holds a licence for a given app.

use super::*;
use crate::utils::c_string_prefix;
use std::net::Ipv6Addr;

/// The address Steam advertises for this server.
///
/// Which of the two arms is the answer is carried by a tag, and the two numbers are documented in
/// *different* orders -- `m_unIPv4` as "Host order" (`steamtypes.h:115`) and `m_rgubIPv6` as
/// "Network order! Same as inaddr_in6" (`:116`) -- so a decode that assumes one rule for both is
/// wrong for one of them. `address_from_steam_parts` is where that is decided, and it is judged
/// without a client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerAddress {
    /// The tag said IPv4, and this is `m_unIPv4` read most-significant octet first.
    Ipv4(Ipv4Addr),
    /// The tag said IPv6, and this is the sixteen bytes read in the order they were written.
    Ipv6(Ipv6Addr),
    /// A tag the generated layer has no name for.
    ///
    /// Not reachable with anything the bindings declare today: `ESteamIPType` names exactly two
    /// values (`k_ESteamIPTypeIPv4 = 0`, `k_ESteamIPTypeIPv6 = 1`), and constructing a third would be
    /// out of range for the enum rather than a test of this table. So this arm has no test and no
    /// fake input -- the ledger in `scripts/ci/baselines/no_panic_sites.txt` and the count here are
    /// what guard it, which is the same treatment the nine future-reachable arms got.
    Unrecognized(i32),
}

/// Whether Steam thinks this user may play this app.
///
/// Three answers, each with Steam's own sentence on it -- the last reads "User has not been
/// authenticated" (`steamclientpublic.h:223`) -- and a fourth for a number the generated layer does
/// not name. The fourth is not reachable from anything the
/// bindings declare, so it is tested by nothing -- see [`ServerAddress::Unrecognized`] for why that is
/// the right handling rather than a fixture that invents an illegal discriminant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserLicense {
    /// "User has a license for specified app" (`steamclientpublic.h:221`).
    HasLicense,
    /// "User does not have a license for the specified app" (`:222`).
    DoesNotHaveLicense,
    /// "User has not been authenticated" (`:223`).
    NoAuth,
    /// A number with no declared name. Kept as a value: `NoAuth` means "nobody checked this user yet",
    /// while this means "Steam answered with something this build cannot read", and folding the two
    /// together would tell a caller to authenticate a user who already is.
    Unrecognized(i32),
}

/// The address, from the three pieces Steam stored.
fn address_from_steam_parts(
    tag: sys::ESteamIPType,
    ipv4_bits: u32,
    ipv6_octets: [u8; 16],
) -> ServerAddress {
    match tag {
        sys::ESteamIPType::k_ESteamIPTypeIPv4 => ServerAddress::Ipv4(Ipv4Addr::from(ipv4_bits)),
        sys::ESteamIPType::k_ESteamIPTypeIPv6 => ServerAddress::Ipv6(Ipv6Addr::from(ipv6_octets)),
        other => ServerAddress::Unrecognized(other as i32),
    }
}

/// The licence answer, from Steam's enum.
fn license_from_raw(raw: sys::EUserHasLicenseForAppResult) -> UserLicense {
    use sys::EUserHasLicenseForAppResult::*;
    match raw {
        k_EUserHasLicenseResultHasLicense => UserLicense::HasLicense,
        k_EUserHasLicenseResultDoesNotHaveLicense => UserLicense::DoesNotHaveLicense,
        k_EUserHasLicenseResultNoAuth => UserLicense::NoAuth,
        other => UserLicense::Unrecognized(other as i32),
    }
}

impl Server {
    /// Starts the process of logging this server out of Steam.
    ///
    /// "Begin process of logging game server out of steam" (`isteamgameserver.h:65`) -- it is a
    /// request, not a completion: nothing here says when the server is out, and Steam takes no answer
    /// back. Read [`Server::is_logged_on`] afterwards if the difference matters to the caller.
    pub fn log_off(&self) {
        unsafe { sys::SteamAPI_ISteamGameServer_LogOff(self.server) }
    }

    /// Whether Steam currently considers this server logged on.
    ///
    /// One of the three under Steam's heading "status functions" (`isteamgameserver.h:68-70`), which
    /// report state and take no arguments.
    pub fn is_logged_on(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamGameServer_BLoggedOn(self.server) }
    }

    /// Whether this server is running in secure mode.
    ///
    /// Steam's declaration gives no further sentence than the heading "status functions"
    /// (`isteamgameserver.h:68-70`), so this wrapper states no more about what secure means than the
    /// header does.
    pub fn is_secure(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamGameServer_BSecure(self.server) }
    }

    /// Whether the master server has asked for a restart.
    ///
    /// "Returns true if the master server has requested a restart.  Only returns true once per
    /// request" (`isteamgameserver.h:73-74`) -- so a second read right after a `true` says `false`,
    /// and a poller that logs every `true` it sees will miss nothing but will not see the request
    /// twice either.
    pub fn restart_requested(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamGameServer_WasRestartRequested(self.server) }
    }

    /// The public address Steam advertises for this server.
    ///
    /// Steam's own reason for the call is on the declaration: it is "useful when the server is behind
    /// NAT and you want to advertise its IP in a lobby for other clients to directly connect to"
    /// (`isteamgameserver.h:184-186`). The answer carries its own family tag, so an address that is
    /// IPv6 in Steam's struct does not arrive as IPv4 here -- see [`ServerAddress`] for the two
    /// documented byte orders, and note that the header names no tag for an unknown family: a
    /// zero-initialised struct reads as IPv4 `0.0.0.0` because that tag is the value zero.
    pub fn public_address(&self) -> ServerAddress {
        let raw = unsafe { sys::SteamAPI_ISteamGameServer_GetPublicIP(self.server) };
        // Both arms are copied out before the tag is consulted, because the struct is
        // `#[repr(C, packed)]`: reading the active arm through a reference would take an unaligned
        // borrow of bytes whose order depends on which arm Steam wrote.
        let (ipv4_bits, ipv6_octets) = unsafe {
            (
                raw.__bindgen_anon_1.m_unIPv4,
                raw.__bindgen_anon_1.m_rgubIPv6,
            )
        };
        address_from_steam_parts(raw.m_eType, ipv4_bits, ipv6_octets)
    }

    /// Whether this user holds a licence for `app`.
    ///
    /// Steam's instruction is to use it after "receiving a user's authentication data"
    /// (`isteamgameserver.h:169`) to determine "if the user owns downloadable content specified by
    /// the provided AppID" (`:169-170`). Note what that sentence still points at: the header names
    /// `SendUserConnectAndAuthenticate` as the step before it, and that function is under
    /// "These will be removed in a future version of the SDK" (`isteamgameserver.h:259`) -- this crate wraps neither, so the input here is a user authenticated by whatever
    /// path the server uses today. The three answers stay distinguishable (`UserLicense`), and the
    /// answer that means the user was never authenticated is not folded into the one that says there is
    /// no licence.
    pub fn user_has_license(&self, user: SteamId, app: AppId) -> UserLicense {
        license_from_raw(unsafe {
            sys::SteamAPI_ISteamGameServer_UserHasLicenseForApp(self.server, user.0, app.0)
        })
    }

    /// Sets the region identifier Steam advertises for this server.
    ///
    /// "Region identifier" (`isteamgameserver.h:136`) is optional, and the same sentence adds that
    /// the default value is empty, meaning the world region. Steam returns nothing, so a string carrying an
    /// interior NUL is handed over as its prefix rather than reported: the same rule the other void
    /// setters in this module follow, written out on `crate::utils::c_string_prefix`.
    pub fn set_region(&self, region: &str) {
        let region = c_string_prefix(region);
        unsafe { sys::SteamAPI_ISteamGameServer_SetRegion(self.server, region.as_ptr()) }
    }

    /// Advertises a spectator port for this server.
    ///
    /// Steam is explicit that this only moves what is advertised: "This function merely controls the
    /// values that are advertised -- it's up to you to configure the server to actually listen on this
    /// port and handle any spectator traffic" (`isteamgameserver.h:109-110`). With a FakeIP allocation
    /// the number that shows up on the master server is not this one but "the second fake port number
    /// (index 1) assigned to your server" (`:105-107`), and only "if you set this value to any nonzero
    /// value" (`:107`).
    pub fn set_spectator_port(&self, port: u16) {
        unsafe { sys::SteamAPI_ISteamGameServer_SetSpectatorPort(self.server, port) }
    }

    /// Names the spectator server that the advertised port belongs to.
    ///
    /// "Name of the spectator server.  (Only used if spectator port is nonzero.)"
    /// (`isteamgameserver.h:113`), so the port above decides whether this is read at all. Void in,
    /// void out, so an interior NUL truncates by the same rule as [`Server::set_region`].
    pub fn set_spectator_server_name(&self, name: &str) {
        let name = c_string_prefix(name);
        unsafe { sys::SteamAPI_ISteamGameServer_SetSpectatorServerName(self.server, name.as_ptr()) }
    }
}

#[cfg(test)]
mod status_tests {
    //! The two tables and the byte orders, judged without a server. What Steam answers over FFI is
    //! registered as unverified: every method in this module needs a logged-on game server.

    use super::{
        address_from_steam_parts, license_from_raw, ServerAddress as Addr, UserLicense as License,
    };
    use std::net::{Ipv4Addr, Ipv6Addr};
    use steamworks_sys as sys;

    #[test]
    fn each_address_family_is_read_in_the_order_its_own_field_documents() {
        // "Host order" for IPv4 (`steamtypes.h:115`), so the first octet is the most significant;
        // "Network order!" for IPv6 (`:116`), so the bytes are already in written order.
        assert_eq!(
            address_from_steam_parts(sys::ESteamIPType::k_ESteamIPTypeIPv4, 0xC0A8_0101, [0; 16]),
            Addr::Ipv4(Ipv4Addr::new(192, 168, 1, 1))
        );
        assert_eq!(
            address_from_steam_parts(
                sys::ESteamIPType::k_ESteamIPTypeIPv6,
                0,
                [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01]
            ),
            Addr::Ipv6(Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1))
        );
        // The two readings are not interchangeable: applying the IPv4 rule to the IPv6 bytes (or the
        // other way round) gives a different host, and both halves of that are checked above.
        assert_ne!(
            address_from_steam_parts(sys::ESteamIPType::k_ESteamIPTypeIPv4, 0x0100_0000, [0; 16]),
            Addr::Ipv4(Ipv4Addr::new(0, 0, 0, 1))
        );
    }

    #[test]
    fn the_zeroed_struct_reads_as_ipv4_zero_and_that_is_steam_numbering() {
        // `k_ESteamIPTypeIPv4` is 0, so a struct nobody wrote has the IPv4 tag and no address --
        // the reason the docs say so instead of letting `0.0.0.0` look like an answer.
        assert_eq!(sys::ESteamIPType::k_ESteamIPTypeIPv4 as i32, 0);
        assert_eq!(
            address_from_steam_parts(sys::ESteamIPType::k_ESteamIPTypeIPv4, 0, [0; 16]),
            Addr::Ipv4(Ipv4Addr::UNSPECIFIED)
        );
    }

    #[test]
    fn every_licence_answer_steam_declares_has_a_name() {
        let pairs = [
            (
                sys::EUserHasLicenseForAppResult::k_EUserHasLicenseResultHasLicense,
                License::HasLicense,
            ),
            (
                sys::EUserHasLicenseForAppResult::k_EUserHasLicenseResultDoesNotHaveLicense,
                License::DoesNotHaveLicense,
            ),
            (
                sys::EUserHasLicenseForAppResult::k_EUserHasLicenseResultNoAuth,
                License::NoAuth,
            ),
        ];
        for (raw, named) in pairs {
            assert_eq!(license_from_raw(raw), named, "{:?}", raw as i32);
        }
        assert_eq!(pairs.len(), 3, "the header declares three answers");
        // "not authenticated" and "has no licence" are different facts and must not collapse.
        assert_ne!(License::NoAuth, License::DoesNotHaveLicense);
    }

    /// A premise pin, not a lock on this crate's code: the numbers above are Steam's numbering, and
    /// if a regeneration ever reorders them the table has to be re-read. Checked by renumbering one
    /// discriminant in the generated layer, which turns this red -- no change of mine can.
    #[test]
    fn the_licence_numbers_are_the_ones_the_header_gives() {
        assert_eq!(
            sys::EUserHasLicenseForAppResult::k_EUserHasLicenseResultHasLicense as i32,
            0
        );
        assert_eq!(
            sys::EUserHasLicenseForAppResult::k_EUserHasLicenseResultDoesNotHaveLicense as i32,
            1
        );
        assert_eq!(
            sys::EUserHasLicenseForAppResult::k_EUserHasLicenseResultNoAuth as i32,
            2
        );
    }
}
