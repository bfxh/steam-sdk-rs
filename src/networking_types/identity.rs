//! `NetworkingIdentity` -- Steam's tagged "who is this" value, with its conversions in and
//! out of the generated struct.

use super::*;

/// A safe wrapper for SteamNetworkingIdentity
#[derive(Clone)]
pub struct NetworkingIdentity {
    // Using a enum for NetworkingIdentity with variants for each identity type would be more idiomatic to use,
    // but would require converting between the internal and the rust representation whenever the API is used.
    // Maybe a second type could be used for matching to avoid get_ip, get_steam_id, etc.
    // `pub(super)` because `message.rs` fills a Steam identity struct straight out of this
    // field; the split made two files of what was one.
    pub(super) inner: sys::SteamNetworkingIdentity,
}

// const NETWORK_IDENTITY_STRING_BUFFER_SIZE: usize =
//     sys::SteamNetworkingIdentity__bindgen_ty_1::k_cchMaxString as usize;

impl NetworkingIdentity {
    pub fn new() -> Self {
        unsafe {
            let mut id = sys::SteamNetworkingIdentity {
                m_eType: sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_Invalid,
                m_cbSize: 0,
                __bindgen_anon_1: sys::SteamNetworkingIdentity__bindgen_ty_2 { m_steamID64: 0 },
            };
            sys::SteamAPI_SteamNetworkingIdentity_Clear(&mut id);
            Self { inner: id }
        }
    }

    pub fn new_steam_id(id: SteamId) -> Self {
        let mut identity = Self::new();
        identity.set_steam_id(id);
        identity
    }

    pub fn new_ip(addr: SocketAddr) -> Self {
        let mut identity = Self::new();
        identity.set_ip_addr(addr);
        identity
    }

    pub fn steam_id(&self) -> Option<SteamId> {
        unsafe {
            let id = sys::SteamAPI_SteamNetworkingIdentity_GetSteamID64(self.as_ptr() as *mut _);
            if id == 0 {
                None
            } else {
                Some(SteamId(id))
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.is_invalid()
    }

    pub fn is_invalid(&self) -> bool {
        unsafe { sys::SteamAPI_SteamNetworkingIdentity_IsInvalid(self.as_ptr() as *mut _) }
    }

    pub fn set_steam_id(&mut self, id: SteamId) {
        unsafe { sys::SteamAPI_SteamNetworkingIdentity_SetSteamID64(self.as_mut_ptr(), id.0) }
    }

    pub fn set_ip_addr(&mut self, addr: SocketAddr) {
        let addr = SteamIpAddr::from(addr);
        unsafe {
            sys::SteamAPI_SteamNetworkingIdentity_SetIPAddr(self.as_mut_ptr(), addr.as_ptr());
        }
    }

    #[allow(dead_code)]
    pub(crate) fn ip_addr(&self) -> Option<SteamIpAddr> {
        unsafe {
            let ip = sys::SteamAPI_SteamNetworkingIdentity_GetIPAddr(self.as_ptr() as *mut _);
            if ip.is_null() {
                None
            } else {
                Some(SteamIpAddr { inner: (*ip) })
            }
        }
    }

    pub fn set_local_host(&mut self) {
        unsafe { sys::SteamAPI_SteamNetworkingIdentity_SetLocalHost(self.as_mut_ptr()) }
    }

    pub fn is_local_host(&self) -> bool {
        unsafe { sys::SteamAPI_SteamNetworkingIdentity_IsLocalHost(self.as_ptr() as *mut _) }
    }

    pub fn debug_string(&self) -> String {
        // For some reason I can't get the original function to work,
        // so I decided to recreate the original from https://github.com/ValveSoftware/GameNetworkingSockets/blob/529901e7c1caf50928ac8814cad205d192bbf27d/src/steamnetworkingsockets/steamnetworkingsockets_shared.cpp

        // let mut buffer = vec![0i8; NETWORK_IDENTITY_STRING_BUFFER_SIZE];
        // let string = unsafe {
        //     sys::SteamAPI_SteamNetworkingIdentity_ToString(
        //         self.as_ptr() as *mut sys::SteamNetworkingIdentity,
        //         buffer.as_mut_ptr(),
        //         NETWORK_IDENTITY_STRING_BUFFER_SIZE as u32,
        //     );
        //     CString::from_raw(buffer.as_mut_ptr())
        // };
        // string.into_string().unwrap()

        unsafe {
            match self.inner.m_eType {
                sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_Invalid => {
                    "invalid".to_string()
                }
                sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_SteamID => {
                    let id = self.inner.__bindgen_anon_1.m_steamID64;
                    format!("steamid:{}", id)
                }
                sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_IPAddress => {
                    let ip = SteamIpAddr::from(self.inner.__bindgen_anon_1.m_ip);
                    format!("ip:{}", ip)
                }
                sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_GenericString => {
                    unimplemented!()
                }
                sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_GenericBytes => {
                    unimplemented!()
                }
                sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_UnknownType => {
                    unimplemented!()
                }
                ty => format!("bad_type:{}", ty as u32),
            }
        }
    }

    pub fn is_equal_to(&self, other: &Self) -> bool {
        unsafe {
            sys::SteamAPI_SteamNetworkingIdentity_IsEqualTo(self.as_ptr() as *mut _, other.as_ptr())
        }
    }

    pub(crate) fn as_ptr(&self) -> *const sys::SteamNetworkingIdentity {
        &self.inner
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut sys::SteamNetworkingIdentity {
        &mut self.inner
    }
}

impl PartialEq for NetworkingIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.is_equal_to(other)
    }
}

impl Eq for NetworkingIdentity {}

impl Debug for NetworkingIdentity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.debug_string())
    }
}

impl From<sys::SteamNetworkingIdentity> for NetworkingIdentity {
    fn from(id: steamworks_sys::SteamNetworkingIdentity) -> Self {
        NetworkingIdentity { inner: id }
    }
}

impl From<SteamId> for NetworkingIdentity {
    fn from(id: SteamId) -> Self {
        Self::new_steam_id(id)
    }
}

impl Default for NetworkingIdentity {
    fn default() -> Self {
        Self::new()
    }
}
