//! Lobby matchmaking: creating, joining and describing lobbies, the lobby-list query
//! builder, and the chat side of a lobby.
//!
//! Split by who owns the code -- this file holds `Matchmaking` and the lobby lifecycle,
//! `queries` holds the setter chain that configures a lobby-list request, `filters` holds the
//! types that chain accepts, `chat` holds the chat-entry vocabulary, and `callbacks` holds the
//! payload structs Steam fires.

use std::net::{Ipv4Addr, SocketAddrV4};

use super::*;
use crate::utils::c_string_prefix;
use crate::utils::try_c_string;
#[cfg(test)]
use serial_test::serial;

mod callbacks;
mod chat;
mod filters;
mod queries;

pub use callbacks::*;
pub use chat::*;
pub use filters::*;
// `queries` adds no items of its own -- it is a second `impl Matchmaking` block, declared not
// re-exported.

/// Access to the steam matchmaking interface
pub struct Matchmaking {
    pub(crate) mm: *mut sys::ISteamMatchmaking,
    pub(crate) inner: Arc<Inner>,
}

/// The visibility of a lobby
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LobbyType {
    Private,
    FriendsOnly,
    Public,
    Invisible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LobbyId(pub(crate) u64);

impl LobbyId {
    /// Creates a `LobbyId` from a raw 64 bit value.
    ///
    /// May be useful for deserializing lobby ids from
    /// a network or save format.
    pub fn from_raw(id: u64) -> LobbyId {
        LobbyId(id)
    }

    /// Returns the raw 64 bit value of the lobby id
    ///
    /// May be useful for serializing lobby ids over a
    /// network or to a save format.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl Matchmaking {
    pub fn request_lobby_list<F>(&self, cb: F)
    where
        F: FnOnce(SteamResult<Vec<LobbyId>>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamMatchmaking_RequestLobbyList(self.mm);
            register_call_result::<sys::LobbyMatchList_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        let mut out = Vec::with_capacity(v.m_nLobbiesMatching as usize);
                        for idx in 0..v.m_nLobbiesMatching {
                            out.push(LobbyId(sys::SteamAPI_ISteamMatchmaking_GetLobbyByIndex(
                                sys::SteamAPI_SteamMatchmaking_v009(),
                                idx as _,
                            )));
                        }
                        Ok(out)
                    })
                },
            );
        }
    }

    /// Attempts to create a new matchmaking lobby
    ///
    /// The lobby with have the visibility of the of the passed
    /// `LobbyType` and a limit of `max_members` inside it.
    /// The `max_members` may not be higher than 250.
    ///
    /// # Triggers
    ///
    /// * `LobbyEnter`
    /// * `LobbyCreated`
    pub fn create_lobby<F>(&self, ty: LobbyType, max_members: u32, cb: F)
    where
        F: FnOnce(SteamResult<LobbyId>) + 'static + Send,
    {
        assert!(max_members <= 250); // Steam API limits
        unsafe {
            let ty = match ty {
                LobbyType::Private => sys::ELobbyType::k_ELobbyTypePrivate,
                LobbyType::FriendsOnly => sys::ELobbyType::k_ELobbyTypeFriendsOnly,
                LobbyType::Public => sys::ELobbyType::k_ELobbyTypePublic,
                LobbyType::Invisible => sys::ELobbyType::k_ELobbyTypeInvisible,
            };
            let api_call =
                sys::SteamAPI_ISteamMatchmaking_CreateLobby(self.mm, ty, max_members as _);
            register_call_result::<sys::LobbyCreated_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        crate::to_steam_result(v.m_eResult).map(|_| LobbyId(v.m_ulSteamIDLobby))
                    })
                },
            );
        }
    }

    /// Tries to join the lobby with the given ID
    pub fn join_lobby<F>(&self, lobby: LobbyId, cb: F)
    where
        F: FnOnce(Result<LobbyId, ()>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamMatchmaking_JoinLobby(self.mm, lobby.0);
            register_call_result::<sys::LobbyEnter_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error || v.m_EChatRoomEnterResponse != 1 {
                        Err(())
                    } else {
                        Ok(LobbyId(v.m_ulSteamIDLobby))
                    })
                },
            );
        }
    }

    /// Refreshes all of the metadata for a lobby that you're not in.
    ///
    /// You never need to call this for lobbies you're a member of, that data
    /// will always be up to date. This will send down all of the metadata for
    /// the lobby, overwriting local data.
    ///
    /// # Triggers
    ///
    /// * `LobbyDataUpdate`
    pub fn request_lobby_data(&self, lobby: LobbyId) -> bool {
        unsafe { sys::SteamAPI_ISteamMatchmaking_RequestLobbyData(self.mm, lobby.0) }
    }

    /// Returns the number of data keys in the lobby
    pub fn lobby_data_count(&self, lobby: LobbyId) -> u32 {
        unsafe { sys::SteamAPI_ISteamMatchmaking_GetLobbyDataCount(self.mm, lobby.0) as _ }
    }

    /// Returns the lobby metadata associated with the specified key from the
    /// specified lobby.
    pub fn lobby_data(&self, lobby: LobbyId, key: &str) -> Option<String> {
        let key = CString::new(key).unwrap();
        unsafe {
            let data = sys::SteamAPI_ISteamMatchmaking_GetLobbyData(self.mm, lobby.0, key.as_ptr());
            CStr::from_ptr(data)
        }
        .to_str()
        .ok()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
    }

    /// Returns the lobby metadata associated with the specified index
    pub fn lobby_data_by_index(&self, lobby: LobbyId, idx: u32) -> Option<(String, String)> {
        let mut key = [0 as c_char; sys::k_nMaxLobbyKeyLength as usize];
        let mut value = [0 as c_char; sys::k_cubChatMetadataMax as usize];
        unsafe {
            let success = sys::SteamAPI_ISteamMatchmaking_GetLobbyDataByIndex(
                self.mm,
                lobby.0,
                idx as _,
                key.as_mut_ptr() as _,
                key.len() as _,
                value.as_mut_ptr() as _,
                value.len() as _,
            );
            match success {
                true => Some((
                    CStr::from_ptr(key.as_ptr()).to_string_lossy().into_owned(),
                    CStr::from_ptr(value.as_ptr())
                        .to_string_lossy()
                        .into_owned(),
                )),
                false => None,
            }
        }
    }

    /// Sets the lobby metadata associated with the specified key in the specified lobby.
    /// A key or value containing a nul byte is refused with `false`; nothing is sent to Steam, because
    /// the metadata that would result is not the pair the caller named.
    pub fn set_lobby_data(&self, lobby: LobbyId, key: &str, value: &str) -> bool {
        let key = match try_c_string(key) {
            Some(key) => key,
            None => return false,
        };
        let value = match try_c_string(value) {
            Some(value) => value,
            None => return false,
        };
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_SetLobbyData(
                self.mm,
                lobby.0,
                key.as_ptr(),
                value.as_ptr(),
            )
        }
    }

    /// Deletes the lobby metadata associated with the specified key in the specified lobby.
    pub fn delete_lobby_data(&self, lobby: LobbyId, key: &str) -> bool {
        let key = match try_c_string(key) {
            Some(key) => key,
            None => return false,
        };
        unsafe { sys::SteamAPI_ISteamMatchmaking_DeleteLobbyData(self.mm, lobby.0, key.as_ptr()) }
    }

    /// Sets per-user metadata for the local user.
    ///
    /// Triggers a LobbyDataUpdate callback.
    pub fn set_lobby_member_data(&self, lobby: LobbyId, key: &str, value: &str) {
        let key = c_string_prefix(key);
        let value = c_string_prefix(value);
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_SetLobbyMemberData(
                self.mm,
                lobby.0,
                key.as_ptr(),
                value.as_ptr(),
            )
        }
    }

    /// Gets per-user metadata from another player in the specified lobby.
    ///
    /// This can only be queried from members in lobbies that you are currently in.
    /// Returns None if lobby is invalid, user is not in the lobby, or key is not set.
    pub fn get_lobby_member_data(
        &self,
        lobby: LobbyId,
        user: SteamId,
        key: &str,
    ) -> Option<String> {
        let key = CString::new(key).unwrap();
        unsafe {
            let data = sys::SteamAPI_ISteamMatchmaking_GetLobbyMemberData(
                self.mm,
                lobby.0,
                user.0,
                key.as_ptr(),
            );
            CStr::from_ptr(data)
        }
        .to_str()
        .map(str::to_owned)
        .ok()
    }

    /// Exits the passed lobby
    pub fn leave_lobby(&self, lobby: LobbyId) {
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_LeaveLobby(self.mm, lobby.0);
        }
    }

    /// Returns the current limit on the number of players in a lobby.
    ///
    /// Returns `[None]` if no metadata is available for the specified lobby.
    pub fn lobby_member_limit(&self, lobby: LobbyId) -> Option<usize> {
        unsafe {
            let count = sys::SteamAPI_ISteamMatchmaking_GetLobbyMemberLimit(self.mm, lobby.0);
            match count {
                0 => None,
                _ => Some(count as usize),
            }
        }
    }

    /// Returns the steam id of the current owner of the passed lobby
    pub fn lobby_owner(&self, lobby: LobbyId) -> SteamId {
        unsafe {
            SteamId(sys::SteamAPI_ISteamMatchmaking_GetLobbyOwner(
                self.mm, lobby.0,
            ))
        }
    }

    /// Returns the number of players in a lobby.
    ///
    /// Useful if you are not currently in the lobby
    pub fn lobby_member_count(&self, lobby: LobbyId) -> usize {
        unsafe {
            let count = sys::SteamAPI_ISteamMatchmaking_GetNumLobbyMembers(self.mm, lobby.0);
            count as usize
        }
    }

    /// Returns a list of members currently in the lobby
    pub fn lobby_members(&self, lobby: LobbyId) -> Vec<SteamId> {
        unsafe {
            let count = sys::SteamAPI_ISteamMatchmaking_GetNumLobbyMembers(self.mm, lobby.0);
            let mut members = Vec::with_capacity(count as usize);
            for idx in 0..count {
                members.push(SteamId(
                    sys::SteamAPI_ISteamMatchmaking_GetLobbyMemberByIndex(self.mm, lobby.0, idx),
                ))
            }
            members
        }
    }

    /// Sets whether or not a lobby is joinable by other players. This always defaults to enabled
    /// for a new lobby.
    ///
    /// If joining is disabled, then no players can join, even if they are a friend or have been
    /// invited.
    ///
    /// Lobbies with joining disabled will not be returned from a lobby search.
    ///
    /// Returns true on success, false if the current user doesn't own the lobby.
    pub fn set_lobby_joinable(&self, lobby: LobbyId, joinable: bool) -> bool {
        unsafe { sys::SteamAPI_ISteamMatchmaking_SetLobbyJoinable(self.mm, lobby.0, joinable) }
    }
}

impl Matchmaking {
    /// Sets the game server associated with the lobby.
    ///
    /// This is used to tell other lobby members which game server to connect to.
    ///
    /// # Parameters
    /// - `lobby`: The lobby ID
    /// - `server_addr`: The IP address and port of the game server
    /// - `server_steam_id`: The Steam ID of the game server (optional)
    ///
    /// # Returns
    /// Returns `true` if successful, `false` otherwise
    pub fn set_lobby_game_server(
        &self,
        lobby: LobbyId,
        server_addr: SocketAddrV4,
        server_steam_id: Option<SteamId>,
    ) -> () {
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_SetLobbyGameServer(
                self.mm,
                lobby.0,
                server_addr.ip().to_bits(),
                server_addr.port(),
                server_steam_id.map(|id| id.0).unwrap_or(0),
            )
        }
    }

    /// Gets the game server associated with the lobby.
    ///
    /// # Parameters
    /// - `lobby`: The lobby ID
    ///
    /// # Returns
    /// Returns `None` if no game server is associated, otherwise returns a tuple containing:
    /// - `server_addr`: The IP address and port of the game server
    /// - `server_steam_id`: The Steam ID of the game server (if available)
    pub fn get_lobby_game_server(&self, lobby: LobbyId) -> Option<(SocketAddrV4, Option<SteamId>)> {
        unsafe {
            let mut server_ip = 0;
            let mut server_port = 0;

            let mut server_steam_id = sys::CSteamID {
                m_steamid: sys::CSteamID_SteamID_t { m_unAll64Bits: 0 },
            };

            let success = sys::SteamAPI_ISteamMatchmaking_GetLobbyGameServer(
                self.mm,
                lobby.0,
                &mut server_ip,
                &mut server_port,
                &mut server_steam_id,
            );

            let server_addr = SocketAddrV4::new(Ipv4Addr::from_bits(server_ip), server_port);
            let server_id = SteamId::from_raw(server_steam_id.m_steamid.m_unAll64Bits);

            // Return None if no game server is associated with the lobby
            let server_id = (!server_id.is_invalid()).then_some(server_id);

            if success {
                Some((server_addr, server_id))
            } else {
                None
            }
        }
    }
}

#[test]
#[serial]
#[ignore = "requires a running Steam client"]
fn test_lobby() {
    let client = Client::init().unwrap();
    let mm = client.matchmaking();

    mm.request_lobby_list(|v| {
        println!("List: {:?}", v);
    });

    mm.create_lobby(LobbyType::Private, 4, |v| {
        println!("Create: {:?}", v);
    });

    mm.set_lobby_list_filter(LobbyListFilter {
        string: Some(vec![StringFilter(
            LobbyKey::new("name"),
            "My Lobby",
            StringFilterKind::Equal,
        )]),
        ..Default::default()
    });

    for _ in 0..100 {
        client.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(100));
    }
}

#[test]
#[serial]
#[ignore = "requires a running Steam client"]
fn test_set_lobby_game_server() {
    let client = Client::init().unwrap();
    let mm = client.matchmaking();

    let client2 = client.clone();
    mm.create_lobby(LobbyType::Private, 4, move |v| {
        println!("Create: {:?}", v);
        let mm2 = client2.matchmaking();

        // Test setting and getting game server information
        let lobby_id = v.unwrap(); // Example lobby ID, in real test should use actual ID from create_lobby
        let test_addr = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 27015); // Example IP and port
        let test_steam_id = Some(SteamId(76561197960287930)); // Example SteamID

        // Set game server information
        mm2.set_lobby_game_server(lobby_id, test_addr, test_steam_id);

        // Retrieve and verify game server information
        if let Some((addr, steam_id)) = mm2.get_lobby_game_server(lobby_id) {
            assert_eq!(test_addr, addr, "Server address mismatch");
            assert_eq!(steam_id, test_steam_id, "Server SteamID mismatch");
            println!("Game server info verified: {addr} {steam_id:?}");
        } else {
            panic!("Failed to retrieve lobby game server info");
        }

        // Test case for lobby with no game server set
        let empty_lobby = LobbyId(54321);
        assert!(
            mm2.get_lobby_game_server(empty_lobby).is_none(),
            "Expected None for lobby with no game server set"
        );
    });

    for _ in 0..100 {
        client.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(100));
    }
}
