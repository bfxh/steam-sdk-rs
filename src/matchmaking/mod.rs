//! Lobby matchmaking: creating, joining and describing lobbies, the lobby-list query
//! builder, and the chat side of a lobby.
//!
//! Split by who owns the code -- this file holds `Matchmaking` and the lobby lifecycle,
//! `queries` holds the setter chain that configures a lobby-list request, `favorites` holds
//! the user's own stored server list, `filters` holds the
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
mod favorites;
mod filters;
mod queries;

pub use callbacks::*;
pub use chat::*;
pub use favorites::*;
pub use filters::*;
// `queries` adds no items of its own -- it is a second `impl Matchmaking` block, declared not
// re-exported.

/// Access to the steam matchmaking interface
pub struct Matchmaking {
    pub(crate) mm: *mut sys::ISteamMatchmaking,
    pub(crate) inner: Arc<Inner>,
}

/// The visibility of a lobby -- Steam's own heading for the list is "lobby type description"
/// (`isteammatchmaking.h:17`), and the five values it declares are at `:18-28`.
///
/// `#[non_exhaustive]` because the list is Steam's and not ours. It was four here while the header
/// declared five: `k_ELobbyTypePrivateUnique` (`:26`) had no name, which is the same gap this crate
/// closed for the connection-state and availability tables. A downstream `match` that lists these
/// without a wildcard is the thing that should stop compiling when Steam adds a sixth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum LobbyType {
    /// "only way to join the lobby is to invite to someone else" (`isteammatchmaking.h:20`).
    Private,
    /// "shows for friends or invitees, but not in public lobby list" (`:21`).
    FriendsOnly,
    /// "visible for friends and in lobby list" (`:22`).
    Public,
    /// "returned by search, but not visible to other friends" (`:23`).
    Invisible,
    /// "private, unique and does not delete when empty" (`:26`). Steam adds that one "can only create
    /// from webapi" (`:27`), which is why this value is handed over and Steam decides what to do with
    /// it -- the header says nothing about setting it on an existing lobby.
    PrivateUnique,
}

/// How many seats a lobby has, read from Steam's signed answer.
///
/// The header says the getter "returns 0 if no limit is defined" (`isteammatchmaking.h:227`), so 0 is
/// `None`. A negative is `None` too: Steam declares no such value, and casting one to `usize` would
/// report a lobby of 18446744073709551615 seats -- a wrong answer wearing the shape of a real one.
fn member_limit(count: i32) -> Option<usize> {
    usize::try_from(count).ok().filter(|limit| *limit > 0)
}

/// The type as Steam wants the number, apart from the calls that need a live interface.
pub(crate) fn lobby_type_to_raw(ty: LobbyType) -> sys::ELobbyType {
    match ty {
        LobbyType::Private => sys::ELobbyType::k_ELobbyTypePrivate,
        LobbyType::FriendsOnly => sys::ELobbyType::k_ELobbyTypeFriendsOnly,
        LobbyType::Public => sys::ELobbyType::k_ELobbyTypePublic,
        LobbyType::Invisible => sys::ELobbyType::k_ELobbyTypeInvisible,
        LobbyType::PrivateUnique => sys::ELobbyType::k_ELobbyTypePrivateUnique,
    }
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

    /// Creates a lobby on Steam's servers, and reports what Steam decided through `cb`.
    ///
    /// Steam's own block says it will "Create a lobby on the Steam servers.", that for a private type
    /// "the lobby will not be returned by any RequestLobbyList() call" (`isteammatchmaking.h:131-133`),
    /// that "this is an asynchronous request" and its "results will be returned by LobbyCreated_t
    /// callback and call result" (`:134-135`), and that a `LobbyEnter_t` also arrives "since the local
    /// user is joining their own lobby" (`:136`).
    ///
    /// `max_members` is Steam's `int` (`:138`) rather than a wider number narrowed on the way in. The
    /// `max_members <= 250` assertion this method used to run before asking is **gone**: 250 appears
    /// nowhere in the vendored header (measured: no `250` anywhere in `isteammatchmaking.h`), so the
    /// ceiling was this crate's own claim, and its failure mode was an abort in the caller's frame --
    /// while the answer Steam actually gives is `LobbyCreated_t`'s `m_eResult`, mapped through
    /// `to_steam_result` just below. An over-large limit now arrives as `Err` in `cb` like any other
    /// refusal, and a caller that wants a local bound can check one before calling.
    pub fn create_lobby<F>(&self, ty: LobbyType, max_members: i32, cb: F)
    where
        F: FnOnce(SteamResult<LobbyId>) + 'static + Send,
    {
        unsafe {
            let ty = lobby_type_to_raw(ty);
            let api_call = sys::SteamAPI_ISteamMatchmaking_CreateLobby(self.mm, ty, max_members);
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
    /// A key containing a nul byte answers `None` rather than querying its prefix.
    pub fn lobby_data(&self, lobby: LobbyId, key: &str) -> Option<String> {
        let key = try_c_string(key)?;
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
        let key = match try_c_string(key) {
            Some(key) => key,
            None => return None,
        };
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
    /// Steam's own reading of the number is "returns 0 if no limit is defined"
    /// (`isteammatchmaking.h:227`), so 0 comes back as `None`. So does a negative answer, through
    /// `member_limit`: the header names no such value, and handing one to a `usize` would turn a
    /// sentence this wrapper cannot read into a lobby of 18446744073709551615 seats.
    ///
    /// Returns `[None]` if no metadata is available for the specified lobby.
    pub fn lobby_member_limit(&self, lobby: LobbyId) -> Option<usize> {
        member_limit(unsafe {
            sys::SteamAPI_ISteamMatchmaking_GetLobbyMemberLimit(self.mm, lobby.0)
        })
    }

    /// Sets the limit on how many users may join this lobby.
    ///
    /// Steam's sentence for the call is "set the limit on the # of users who can join the lobby"
    /// (`isteammatchmaking.h:225`) and it takes a signed int (`:226`), so this wrapper takes one too
    /// rather than accepting a number it cannot hand over faithfully. Read the result back with
    /// [`Matchmaking::lobby_member_limit`], which is where 0 means "no limit is defined"
    /// (`isteammatchmaking.h:227`).
    ///
    /// The header documents no failure code for this call, so the `bool` is Steam's answer unchanged.
    pub fn set_lobby_member_limit(&self, lobby: LobbyId, max_members: i32) -> bool {
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_SetLobbyMemberLimit(self.mm, lobby.0, max_members)
        }
    }

    /// Changes which type of lobby this is.
    ///
    /// The type is what decides whether a search can see the lobby: "only lobbies that are
    /// k_ELobbyTypePublic or k_ELobbyTypeInvisible, and are set to joinable, will be returned by
    /// RequestLobbyList() calls" (`isteammatchmaking.h:230-231`). [`LobbyType::PrivateUnique`] is
    /// accepted because Steam declares the value; its note that one "can only create from webapi"
    /// (`:27`) is about creating, so this call hands it over and reports whatever Steam answers.
    ///
    /// The header documents no failure code here either, so the `bool` is Steam's.
    pub fn set_lobby_type(&self, lobby: LobbyId, ty: LobbyType) -> bool {
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_SetLobbyType(self.mm, lobby.0, lobby_type_to_raw(ty))
        }
    }

    /// Hands the lobby's ownership to another member.
    ///
    /// Two preconditions are Steam's, not this wrapper's: "you must be the lobby owner for this to
    /// succeed, and steamIDNewOwner must be in the lobby", and "after completion, the local user will
    /// no longer be the owner" (`isteammatchmaking.h:244-246`). Both are why the answer is a `bool`
    /// and not an error type -- a refusal is one of those two facts about the caller, not a code.
    /// Read the current owner with [`Matchmaking::lobby_owner`].
    pub fn set_lobby_owner(&self, lobby: LobbyId, new_owner: SteamId) -> bool {
        unsafe { sys::SteamAPI_ISteamMatchmaking_SetLobbyOwner(self.mm, lobby.0, new_owner.0) }
    }

    /// Links two lobbies so that player-compatibility checks see them together.
    ///
    /// "link two lobbies for the purposes of checking player compatibility", and the precondition the
    /// header states is "you must be the lobby owner of both lobbies" (`isteammatchmaking.h:249-250`).
    /// Nothing is reported about a link that Steam later drops, so the `bool` is only Steam's answer
    /// to this call.
    pub fn set_linked_lobby(&self, lobby: LobbyId, dependent: LobbyId) -> bool {
        unsafe { sys::SteamAPI_ISteamMatchmaking_SetLinkedLobby(self.mm, lobby.0, dependent.0) }
    }

    /// Sends an invite for this lobby to another user.
    ///
    /// Steam is explicit about what the answer does and does not mean: "will return true if the
    /// invite is successfully sent, whether or not the target responds", and "returns false if the
    /// local user is not connected to the Steam servers" (`isteammatchmaking.h:153-154`). So `true`
    /// is a send, not an acceptance.
    ///
    /// The header also promises that the target "will receive a LobbyInvite_t callback" (`:152`); this
    /// crate does not deliver that payload today, so the invite is one-way from here -- measured by
    /// the absence of `LobbyInvite` anywhere under `src/`, which is why this doc points at the header
    /// rather than at a callback type.
    pub fn invite_user_to_lobby(&self, lobby: LobbyId, invitee: SteamId) -> bool {
        unsafe { sys::SteamAPI_ISteamMatchmaking_InviteUserToLobby(self.mm, lobby.0, invitee.0) }
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

#[cfg(test)]
mod lobby_type_tests {
    //! The lobby-type table and the member-limit reader, both judged without a client. What is pinned
    //! is that the table covers the five values under Steam's heading "lobby type description"
    //! (`isteammatchmaking.h:17-28`), and that the numbers under those names are the ones the header
    //! gives.

    use super::{lobby_type_to_raw, member_limit, LobbyType};
    use steamworks_sys as sys;
    use sys::ELobbyType as Raw;

    #[test]
    fn every_lobby_type_steam_declares_has_a_name_here() {
        let pairs = [
            (LobbyType::Private, Raw::k_ELobbyTypePrivate),
            (LobbyType::FriendsOnly, Raw::k_ELobbyTypeFriendsOnly),
            (LobbyType::Public, Raw::k_ELobbyTypePublic),
            (LobbyType::Invisible, Raw::k_ELobbyTypeInvisible),
            (LobbyType::PrivateUnique, Raw::k_ELobbyTypePrivateUnique),
        ];
        for (named, raw) in pairs {
            assert_eq!(lobby_type_to_raw(named), raw, "{:?}", named);
        }
        // The header's five types are what the variant docs cite; a row count only catches a row
        // dropped from this table, and its message says so rather than claiming to watch Steam.
        assert_eq!(pairs.len(), 5, "this table enumerates five types");
    }

    /// The premise the table rests on: Steam's numbering, straight from the declarations behind it --
    /// the fifth of them, `k_ELobbyTypePrivateUnique`, is "private, unique and does not delete when
    /// empty" (`isteammatchmaking.h:26`). If a regeneration ever moves one of these, the names above stop meaning what the
    /// header says, and this is the test that notices -- no mutation of this crate's code can redden
    /// it, which is exactly why it is written as numbers rather than as names.
    #[test]
    fn the_numbers_under_those_names_are_the_ones_the_header_gives() {
        assert_eq!(Raw::k_ELobbyTypePrivate as i32, 0);
        assert_eq!(Raw::k_ELobbyTypeFriendsOnly as i32, 1);
        assert_eq!(Raw::k_ELobbyTypePublic as i32, 2);
        assert_eq!(Raw::k_ELobbyTypeInvisible as i32, 3);
        assert_eq!(Raw::k_ELobbyTypePrivateUnique as i32, 4);
    }

    #[test]
    fn only_a_positive_limit_is_reported_as_a_limit() {
        assert_eq!(
            member_limit(0),
            None,
            "0 is Steam's own 'no limit is defined'"
        );
        assert_eq!(
            member_limit(-1),
            None,
            "a value Steam declares no answer for is not a seat count"
        );
        assert_eq!(member_limit(1), Some(1));
        assert_eq!(member_limit(250), Some(250));
        // The wrap this refuses: -1 read as usize would be the largest number the type holds, and a
        // caller comparing it against a real lobby size would get an answer that looks plausible.
        assert_ne!(member_limit(-1), Some(usize::MAX));
    }
}
