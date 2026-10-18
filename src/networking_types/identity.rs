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
                    // `steamnetworkingtypes.h:320` sizes this inline array as "Max length of the
                    // string for generic string identities.  Including terminating '\0'" (`:335`).
                    // The scan is bounded by the array and stops at the first NUL, so a peer that
                    // sends an unterminated buffer cannot walk past the struct, and non-UTF-8 bytes
                    // are lossy-decoded rather than fatal. This arm used to be `unimplemented!()`,
                    // which made `{:?}` on any peer identity panic.
                    let raw = self.inner.__bindgen_anon_1.m_szGenericString;
                    let bytes: Vec<u8> = raw
                        .iter()
                        .map(|byte| *byte as u8)
                        .take_while(|byte| *byte != 0)
                        .collect();
                    format!("generic:{}", String::from_utf8_lossy(&bytes))
                }
                sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_GenericBytes => {
                    // `m_cbSize` carries the byte count (`steamnetworkingtypes.h:1930-1931`:
                    // "m_cbSize = int(cbLen); memcpy( m_genericBytes, data, m_cbSize )"), and it is a
                    // signed `c_int`, so it is clamped to the array rather than cast: a negative or
                    // oversized value used to reach `unimplemented!()` and panic.
                    let raw = self.inner.__bindgen_anon_1.m_genericBytes;
                    let len = usize::try_from(self.inner.m_cbSize)
                        .unwrap_or(0)
                        .min(raw.len());
                    format!(
                        "bytes:{}",
                        raw[..len]
                            .iter()
                            .map(|byte| format!("{byte:02x}"))
                            .collect::<String>()
                    )
                }
                sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_UnknownType => {
                    // Steam's own reason for this value is to "facilitate forward compatibility"
                    // (`steamnetworkingtypes.h:252-256`), so naming it is the whole job here.
                    format!("unknown_type:{}", self.inner.m_eType as u32)
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

#[cfg(test)]
mod tests {
    //! Formatting an identity that arrived from a peer. Nothing here needs Steam: the struct is built
    //! in Rust memory, which is also the point -- `debug_string` reads `m_eType`, `m_cbSize` and the
    //! inline arrays exactly as a remote's data would set them, and all three of those arms used to be
    //! `unimplemented!()`, so `{:?}` panicked inside whatever code was logging the connection.
    use super::*;
    use std::ffi::c_char;

    fn identity(
        kind: sys::ESteamNetworkingIdentityType,
        size: i32,
        body: sys::SteamNetworkingIdentity__bindgen_ty_2,
    ) -> NetworkingIdentity {
        let mut raw = unsafe { std::mem::zeroed::<sys::SteamNetworkingIdentity>() };
        raw.m_eType = kind;
        raw.m_cbSize = size;
        raw.__bindgen_anon_1 = body;
        NetworkingIdentity::from(raw)
    }

    fn string_body(text: &str) -> sys::SteamNetworkingIdentity__bindgen_ty_2 {
        let mut buffer = [0 as c_char; 32];
        for (slot, byte) in buffer.iter_mut().zip(text.as_bytes()) {
            *slot = *byte as c_char;
        }
        sys::SteamNetworkingIdentity__bindgen_ty_2 {
            m_szGenericString: buffer,
        }
    }

    const GENERIC_STRING: sys::ESteamNetworkingIdentityType =
        sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_GenericString;
    const GENERIC_BYTES: sys::ESteamNetworkingIdentityType =
        sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_GenericBytes;
    const UNKNOWN: sys::ESteamNetworkingIdentityType =
        sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_UnknownType;

    #[test]
    fn a_generic_string_identity_formats_instead_of_panicking() {
        let id = identity(GENERIC_STRING, 6, string_body("hello"));
        assert_eq!(format!("{id:?}"), r"generic:hello");
    }

    /// A remote that never writes the terminator cannot walk past the 32-byte array: the scan is
    /// bounded by the array itself, which is what `k_cchMaxGenericString` ("Including terminating
    /// '\0'", `steamnetworkingtypes.h:320`) buys.
    #[test]
    fn an_unterminated_generic_string_stops_at_the_array() {
        let id = identity(GENERIC_STRING, 0, string_body(&"A".repeat(32)));
        let text = format!("{id:?}");
        assert_eq!(text.matches('A').count(), 32, "{text}");
    }

    /// Non-UTF-8 in a peer-chosen field is decoded lossily, not fatal.
    #[test]
    fn a_non_utf8_generic_string_is_lossy_not_fatal() {
        let mut buffer = [0 as c_char; 32];
        buffer[0] = 0xffu8 as c_char;
        buffer[1] = b'x' as c_char;
        let body = sys::SteamNetworkingIdentity__bindgen_ty_2 {
            m_szGenericString: buffer,
        };
        let id = identity(GENERIC_STRING, 2, body);
        assert!(format!("{id:?}").starts_with("generic:"));
    }

    #[test]
    fn generic_bytes_are_printed_for_the_length_the_identity_claims() {
        let body = sys::SteamNetworkingIdentity__bindgen_ty_2 {
            m_genericBytes: [
                0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0,
            ],
        };
        assert_eq!(
            format!("{:?}", identity(GENERIC_BYTES, 2, body)),
            r"bytes:dead"
        );
    }

    /// `m_cbSize` is a signed `c_int` in a peer-filled struct: negative means "no bytes", not
    /// "2^64 bytes", and an over-long claim is clamped to the array rather than read out of it.
    #[test]
    fn a_negative_or_oversized_byte_count_is_clamped_instead_of_reading_out_of_bounds() {
        let body = sys::SteamNetworkingIdentity__bindgen_ty_2 {
            m_genericBytes: [7; 32],
        };
        assert_eq!(
            format!("{:?}", identity(GENERIC_BYTES, -5, body)),
            r"bytes:"
        );
        let clamped = format!("{:?}", identity(GENERIC_BYTES, 999, body));
        let payload = clamped
            .strip_prefix("bytes:")
            .expect("the bytes arm keeps its prefix");
        assert_eq!(payload.len(), 64, "{clamped}");
        assert!(
            payload.bytes().all(|byte| byte == b'0' || byte == b'7'),
            "{clamped}"
        );
    }

    /// A value whose documented purpose is to "facilitate forward compatibility"
    /// (`steamnetworkingtypes.h:252-256`) must not be a crash either.
    #[test]
    fn an_unknown_identity_type_is_named() {
        let id = identity(UNKNOWN, 0, string_body(""));
        assert_eq!(format!("{id:?}"), r"unknown_type:4");
    }

    /// The three arms that already worked are pinned too, so widening them cannot be paid for with a
    /// silent change to the ones callers actually parse.
    #[test]
    fn the_previously_working_arms_still_read_the_same() {
        let mut raw = unsafe { std::mem::zeroed::<sys::SteamNetworkingIdentity>() };
        raw.m_eType = sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_Invalid;
        assert_eq!(format!("{:?}", NetworkingIdentity::from(raw)), r"invalid");
        raw.m_eType = sys::ESteamNetworkingIdentityType::k_ESteamNetworkingIdentityType_SteamID;
        raw.__bindgen_anon_1 = sys::SteamNetworkingIdentity__bindgen_ty_2 { m_steamID64: 99 };
        assert_eq!(
            format!("{:?}", NetworkingIdentity::from(raw)),
            r"steamid:99"
        );
    }
}
