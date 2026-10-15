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
