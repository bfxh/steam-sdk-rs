//! `SteamIpAddr` -- the generated `SteamNetworkingIPAddr`, with Rust's `SocketAddr` on the
//! other side of the conversion.

use super::*;

#[derive(Copy, Clone)]
pub(crate) struct SteamIpAddr {
    // `pub(super)`: `identity.rs` builds one of these from a Steam struct field, which the split
    // put in another file.
    pub(super) inner: sys::SteamNetworkingIPAddr,
}

#[allow(dead_code)]
impl SteamIpAddr {
    pub fn new() -> Self {
        unsafe {
            let mut ip = sys::SteamNetworkingIPAddr {
                __bindgen_anon_1: sys::SteamNetworkingIPAddr__bindgen_ty_2 {
                    m_ipv4: sys::SteamNetworkingIPAddr_IPv4MappedAddress {
                        m_8zeros: 0,
                        m_0000: 0,
                        m_ffff: 0,
                        m_ip: [0; 4],
                    },
                },
                m_port: 0,
            };
            sys::SteamAPI_SteamNetworkingIPAddr_Clear(&mut ip);
            Self { inner: ip }
        }
    }

    pub fn new_ip(ip: IpAddr, port: u16) -> Self {
        SocketAddr::new(ip, port).into()
    }

    pub fn set_ip(&mut self, ip: SocketAddr) {
        match ip {
            SocketAddr::V4(ip) => {
                self.set_ipv4(ip);
            }
            SocketAddr::V6(ip) => {
                self.set_ipv6(ip);
            }
        }
    }

    pub fn set_ipv4(&mut self, ip: SocketAddrV4) {
        unsafe {
            sys::SteamAPI_SteamNetworkingIPAddr_SetIPv4(
                &mut self.inner,
                (*ip.ip()).into(),
                ip.port(),
            );
        }
    }

    pub fn set_ipv6(&mut self, ip: SocketAddrV6) {
        unsafe {
            sys::SteamAPI_SteamNetworkingIPAddr_SetIPv6(
                &mut self.inner,
                ip.ip().octets().as_ptr(),
                ip.port(),
            );
        }
    }

    pub fn get_ipv4(&self) -> Option<Ipv4Addr> {
        let ip = unsafe { sys::SteamAPI_SteamNetworkingIPAddr_GetIPv4(self.as_ptr() as *mut _) };
        if ip == 0 {
            None
        } else {
            Some(Ipv4Addr::from(ip))
        }
    }

    pub fn is_ipv4(&self) -> bool {
        unsafe { sys::SteamAPI_SteamNetworkingIPAddr_IsIPv4(self.as_ptr() as *mut _) }
    }

    /// The address in Rust's own type, decoded from Steam's bytes rather than asked of Steam.
    ///
    /// Both arms of the union are 16 bytes ("Store an IP and port", `steamnetworkingtypes.h:195`),
    /// so reading `m_ipv6` reads the same bytes a mapped IPv4 was written into, and the classification
    /// is the one Steam documents: "IPv4 aa.bb.cc.dd => IPv6 ::ffff:aabb:ccdd" (`:196`). Going through
    /// the generated accessors instead -- [`Self::is_ipv4`], then `GetIPv4` -- would need a live client for
    /// a question that is answered by twelve bytes of prefix, and `GetIPv4` "Returns 0 if IP is not
    /// mapped IPv4" (`:205`) conflates 0.0.0.0 with "not IPv4" at the exact address a cleared struct
    /// holds (`:200` says "Set everything to zero"). See [`socket_addr_from_octets`] for the rule.
    pub(crate) fn to_socket_addr(&self) -> SocketAddr {
        // Every field read below copies out of a `#[repr(C, packed)]` struct; none of them takes a
        // reference to a field, which is the same reason `to_string` keeps a `port` local.
        unsafe {
            let octets = self.inner.__bindgen_anon_1.m_ipv6;
            socket_addr_from_octets(octets, self.inner.m_port)
        }
    }

    pub fn as_ptr(&self) -> *const sys::SteamNetworkingIPAddr {
        &self.inner
    }

    pub fn as_mut_ptr(&mut self) -> *mut sys::SteamNetworkingIPAddr {
        &mut self.inner
    }

    pub fn to_string(&self, with_port: bool) -> String {
        // Similar as with NetworkIdentity, I wasn't able to get the C function to work,
        // so I'm recreating it from https://github.com/ValveSoftware/GameNetworkingSockets/blob/529901e7c1caf50928ac8814cad205d192bbf27d/src/steamnetworkingsockets/steamnetworkingsockets_shared.cpp
        // let mut buffer = vec![0; sys::SteamNetworkingIPAddr_k_cchMaxString as usize];
        // let c_str;
        // unsafe {
        //     sys::SteamAPI_SteamNetworkingIPAddr_ToString(
        //         &self.inner as *const _ as *mut _,
        //         buffer.as_mut_ptr(),
        //         buffer.len() as _,
        //         false,
        //     );
        //     c_str = CStr::from_ptr(buffer.as_ptr());
        // }
        // let str_slice = c_str.to_str().unwrap();
        // str_slice.to_owned()

        unsafe {
            if self.is_ipv4() {
                let ip4 = self.inner.__bindgen_anon_1.m_ipv4.m_ip;
                if with_port {
                    // This variable is necessary, format will create a unaligned reference to m_port, which can cause undefined bahavior
                    let port = self.inner.m_port;
                    format!("{}.{}.{}.{}:{}", ip4[0], ip4[1], ip4[2], ip4[3], port)
                } else {
                    format!("{}.{}.{}.{}", ip4[0], ip4[1], ip4[2], ip4[3])
                }
            } else {
                // I'm just assuming that steam and rust have the same representation of ip6
                let ip6 = Ipv6Addr::from(self.inner.__bindgen_anon_1.m_ipv6);
                if with_port {
                    // Same as with ipv4, don't remove this temp variable
                    let port = self.inner.m_port;
                    format!("[{}]:{}", ip6, port)
                } else {
                    format!("{}", ip6)
                }
            }
        }
    }
}

/// The twelve bytes that make a 16-byte address an IPv4 one: ten zeros then `ff ff`, which is how
/// Steam writes the mapped form and how its own struct spells it -- `uint64 m_8zeros`,
/// `uint16 m_0000`, `uint16 m_ffff` (`steamnetworkingtypes.h:226-231`, the layout "RFC4038,
/// section 4.2" names).
const IPV4_MAPPED_PREFIX: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff];

/// Decode the 16 address bytes and the port Steam stored beside them.
///
/// Two documented byte orders meet here and neither needs swapping: the four IPv4 octets are
/// stored "As bytes, i.e. network byte order" (`steamnetworkingtypes.h:231`), so they are read in
/// writing order, and the port is "Host byte order" (`:239`), so it is handed over as the number it
/// already is. Anything that is not the mapped prefix stays IPv6 -- including the all-zero address,
/// which `Clear` leaves as "Set everything to zero" (`:200`) and which is therefore the IPv6
/// any-address on port 0 here, and not IPv4 0.0.0.0.
///
/// The `u32` flow info and scope id of the IPv6 answer are 0: this struct stores an IP and a port
/// (`:195`) and has no room for either, so a Steam address cannot carry them.
pub(crate) fn socket_addr_from_octets(octets: [u8; 16], port: u16) -> SocketAddr {
    if octets[..IPV4_MAPPED_PREFIX.len()] == IPV4_MAPPED_PREFIX {
        SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(octets[12], octets[13], octets[14], octets[15]),
            port,
        ))
    } else {
        SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(octets), port, 0, 0))
    }
}

impl Debug for SteamIpAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string(true))
    }
}

impl Display for SteamIpAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string(true))
    }
}

impl Default for SteamIpAddr {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for SteamIpAddr {
    fn eq(&self, other: &Self) -> bool {
        unsafe {
            sys::SteamAPI_SteamNetworkingIPAddr_IsEqualTo(self.as_ptr() as *mut _, &other.inner)
        }
    }
}

impl Eq for SteamIpAddr {}

impl From<SocketAddr> for SteamIpAddr {
    fn from(ip: SocketAddr) -> Self {
        let mut steam_ip = Self::new();
        steam_ip.set_ip(ip);
        steam_ip
    }
}

impl From<SocketAddrV4> for SteamIpAddr {
    fn from(ip: SocketAddrV4) -> Self {
        let mut steam_ip = Self::new();
        steam_ip.set_ipv4(ip);
        steam_ip
    }
}

impl From<SocketAddrV6> for SteamIpAddr {
    fn from(ip: SocketAddrV6) -> Self {
        let mut steam_ip = Self::new();
        steam_ip.set_ipv6(ip);
        steam_ip
    }
}

impl From<sys::SteamNetworkingIPAddr> for SteamIpAddr {
    fn from(inner: sys::SteamNetworkingIPAddr) -> Self {
        Self { inner }
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;
    use crate::Client;
    use std::net::Ipv4Addr;

    #[test]
    fn test_new_ip() {
        let ip = SteamIpAddr::new();
        assert_eq!(&ip.to_string(true), "[::]:0");
    }

    #[test]
    fn test_set_ipv4() {
        let mut ip = SteamIpAddr::new();
        let addr = Ipv4Addr::new(192, 168, 0, 123);
        ip.set_ipv4(SocketAddrV4::new(addr, 5555));
        assert_eq!(Some(addr), ip.get_ipv4());
        assert_eq!(&ip.to_string(true), "192.168.0.123:5555");
    }

    #[test]
    fn test_network_identity_steam_id() {
        let id = NetworkingIdentity::new_steam_id(SteamId(123456));
        assert_eq!("steamid:123456", &id.debug_string())
    }

    #[test]
    fn test_network_identity_ip() {
        let id =
            NetworkingIdentity::new_ip(SocketAddr::new(Ipv4Addr::new(192, 168, 0, 5).into(), 1234));
        assert_eq!("ip:192.168.0.5:1234", &id.debug_string())
    }

    #[test]
    #[serial]
    #[ignore = "requires a running Steam client"]
    fn test_allocate_and_free_message() {
        let client = Client::init().unwrap();
        let utils = client.networking_utils();

        // With C buffer
        {
            let _message = utils.allocate_message(200);
            // Drop it immediately
        }

        // With rust buffer
        {
            let mut message = utils.allocate_message(0);
            message.set_data(vec![1, 2, 3, 4, 5]).unwrap();

            // Drop it immediately
        }
    }
}

#[cfg(test)]
mod decode_tests {
    //! The decode on its own, with no client and no FFI call. This is the part that decides whether
    //! an address is IPv4 and which four bytes are its octets, and a byte-order mistake here does not
    //! fail loudly -- it reports a different host.

    use super::*;

    /// The 16 bytes Steam's own struct spells as `m_8zeros` / `m_0000` / `m_ffff` / `m_ip`.
    fn mapped(octets: [u8; 4]) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[10] = 0xff;
        bytes[11] = 0xff;
        bytes[12..].copy_from_slice(&octets);
        bytes
    }

    #[test]
    fn a_mapped_ipv4_keeps_its_octets_in_writing_order() {
        assert_eq!(
            socket_addr_from_octets(mapped([1, 2, 3, 4]), 5555),
            SocketAddr::from(([1, 2, 3, 4], 5555))
        );
        // Steam stores them "As bytes, i.e. network byte order" (`steamnetworkingtypes.h:231`), so
        // reversing the four is the mutation the second half of this pair exists for.
        assert_ne!(
            socket_addr_from_octets(mapped([1, 2, 3, 4]), 5555),
            SocketAddr::from(([4, 3, 2, 1], 5555))
        );
    }

    #[test]
    fn the_port_is_the_number_it_already_is() {
        // `m_port` is "Host byte order" (`steamnetworkingtypes.h:239`): 0x1234 has to read back as
        // 4660, not as the byte-swapped 13330 a second `from_be_bytes` would give.
        assert_eq!(
            socket_addr_from_octets(mapped([203, 0, 113, 7]), 0x1234).port(),
            4660
        );
    }

    #[test]
    fn the_cleared_address_is_ipv6_zero_and_not_four_zeros() {
        // `Clear` leaves "Set everything to zero" (`steamnetworkingtypes.h:200`): the IPv6
        // any-address on port 0.
        // A prefix test that forgot the two `ff` bytes would call that IPv4 0.0.0.0, and the two
        // answers differ for every caller that branches on the family.
        assert_eq!(
            socket_addr_from_octets([0u8; 16], 0),
            SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0))
        );
    }

    #[test]
    fn anything_outside_the_mapped_prefix_stays_ipv6() {
        let mut bytes = [0u8; 16];
        bytes[0] = 0x20;
        bytes[1] = 0x01;
        bytes[2] = 0x0d;
        bytes[3] = 0xb8;
        bytes[15] = 0x01;
        // Comparing the whole value, not its parts: Steam's struct stores an IP and a port and has
        // no room for flow info or a scope id, so the answer has to carry 0 for both, and a wrong
        // family has to read here as an assertion rather than as a panic on the way to a field.
        assert_eq!(
            socket_addr_from_octets(bytes, 443),
            SocketAddr::from((Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1), 443))
        );
        // The marker is exact: one byte off it and the same tail is no longer IPv4.
        assert!(matches!(
            socket_addr_from_octets(mapped([9, 9, 9, 9]), 1),
            SocketAddr::V4(_)
        ));
        let mut nearly_mapped = mapped([9, 9, 9, 9]);
        nearly_mapped[11] = 0xfe;
        assert!(matches!(
            socket_addr_from_octets(nearly_mapped, 1),
            SocketAddr::V6(_)
        ));
    }

    #[test]
    fn the_four_octets_start_at_byte_twelve_of_the_struct_steam_declares() {
        // The decode indexes `octets[12..]`. That index is only correct while the mapped struct puts
        // `m_ip` at offset 12 and covers the same 16 bytes as the array arm.
        let value = sys::SteamNetworkingIPAddr_IPv4MappedAddress {
            m_8zeros: 0,
            m_0000: 0,
            m_ffff: u16::MAX,
            m_ip: [1, 2, 3, 4],
        };
        let base = std::ptr::addr_of!(value);
        let ip_at = std::ptr::addr_of!(value.m_ip) as usize - base as usize;
        assert_eq!(ip_at, 12);
        assert_eq!(
            std::mem::size_of::<sys::SteamNetworkingIPAddr_IPv4MappedAddress>(),
            std::mem::size_of::<sys::SteamNetworkingIPAddr>() - std::mem::size_of::<u16>(),
            "the union arm and the byte array must cover exactly the address, not the port too"
        );
    }

    #[test]
    fn a_struct_filled_the_way_steam_fills_it_reads_back_as_that_address() {
        // The other half of the pairing: what `From<SocketAddr>` writes through `SetIPv4` is this
        // shape, and the wrapper reads it without going through the generated layer at all.
        let raw = sys::SteamNetworkingIPAddr {
            __bindgen_anon_1: sys::SteamNetworkingIPAddr__bindgen_ty_2 {
                m_ipv4: sys::SteamNetworkingIPAddr_IPv4MappedAddress {
                    m_8zeros: 0,
                    m_0000: 0,
                    m_ffff: u16::MAX,
                    m_ip: [192, 168, 0, 7],
                },
            },
            m_port: 5555,
        };
        assert_eq!(
            SteamIpAddr::from(raw).to_socket_addr(),
            SocketAddr::from(([192, 168, 0, 7], 5555))
        );
    }
}
