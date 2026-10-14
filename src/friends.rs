use super::*;
use crate::app::c_string_arg;
use std::net::Ipv4Addr;

bitflags! {
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[repr(C)]
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct FriendFlags: u16 {
        const NONE                  = 0x0000;
        const BLOCKED               = 0x0001;
        const FRIENDSHIP_REQUESTED  = 0x0002;
        const IMMEDIATE             = 0x0004;
        const CLAN_MEMBER           = 0x0008;
        const ON_GAME_SERVER        = 0x0010;
        // Unused
        // Unused
        const REQUESTING_FRIENDSHIP = 0x0080;
        const REQUESTING_INFO       = 0x0100;
        const IGNORED               = 0x0200;
        const IGNORED_FRIEND        = 0x0400;
        // Unused
        const CHAT_MEMBER           = 0x1000;
        const ALL                   = 0xFFFF;
    }
}

bitflags! {
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[repr(C)]
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct PersonaChange: i32 {
        const NAME                = 0x0001;
        const STATUS              = 0x0002;
        const COME_ONLINE         = 0x0004;
        const GONE_OFFLINE        = 0x0008;
        const GAME_PLAYED         = 0x0010;
        const GAME_SERVER         = 0x0020;
        const AVATAR              = 0x0040;
        const JOINED_SOURCE       = 0x0080;
        const LEFT_SOURCE         = 0x0100;
        const RELATIONSHIP_CHANGE = 0x0200;
        const NAME_FIRST_SET      = 0x0400;
        const FACEBOOK_INFO       = 0x0800;
        const NICKNAME            = 0x1000;
        const STEAM_LEVEL         = 0x2000;
    }
}

pub enum OverlayToStoreFlag {
    None = 0,
    AddToCart = 1,
    AddToCartAndShow = 2,
}

/// Access to the steam friends interface
pub struct Friends {
    pub(crate) friends: *mut sys::ISteamFriends,
    pub(crate) inner: Arc<Inner>,
}

impl Friends {
    /// Returns the (display) name of the current user
    pub fn name(&self) -> String {
        unsafe {
            let name = sys::SteamAPI_ISteamFriends_GetPersonaName(self.friends);
            let name = CStr::from_ptr(name);
            name.to_string_lossy().into_owned()
        }
    }

    pub fn get_friends(&self, flags: FriendFlags) -> Vec<Friend> {
        unsafe {
            let count = sys::SteamAPI_ISteamFriends_GetFriendCount(self.friends, flags.bits() as _);
            if count == -1 {
                return Vec::new();
            }
            let mut friends = Vec::with_capacity(count as usize);
            for idx in 0..count {
                let friend = SteamId(sys::SteamAPI_ISteamFriends_GetFriendByIndex(
                    self.friends,
                    idx,
                    flags.bits() as _,
                ));
                friends.push(self.get_friend(friend));
            }

            friends
        }
    }
    /// Returns recently played with players list
    pub fn get_coplay_friends(&self) -> Vec<Friend> {
        unsafe {
            let count = sys::SteamAPI_ISteamFriends_GetCoplayFriendCount(self.friends);
            if count == -1 {
                return Vec::new();
            }
            let mut friends = Vec::with_capacity(count as usize);
            for idx in 0..count {
                let friend = SteamId(sys::SteamAPI_ISteamFriends_GetCoplayFriend(
                    self.friends,
                    idx,
                ));
                friends.push(self.get_friend(friend));
            }
            friends
        }
    }

    pub fn get_friend(&self, friend: SteamId) -> Friend {
        Friend {
            id: friend,
            friends: self.friends,
            _inner: self.inner.clone(),
        }
    }

    pub fn request_user_information(&self, user: SteamId, name_only: bool) -> bool {
        unsafe {
            sys::SteamAPI_ISteamFriends_RequestUserInformation(self.friends, user.0, name_only)
        }
    }

    pub fn activate_game_overlay(&self, dialog: &str) {
        let dialog = CString::new(dialog).unwrap();
        unsafe {
            sys::SteamAPI_ISteamFriends_ActivateGameOverlay(self.friends, dialog.as_ptr());
        }
    }

    // I don't know why these are part of friends either
    pub fn activate_game_overlay_to_web_page(&self, url: &str) {
        unsafe {
            let url = CString::new(url).unwrap();
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayToWebPage(
                self.friends,
                url.as_ptr(),
                sys::EActivateGameOverlayToWebPageMode::k_EActivateGameOverlayToWebPageMode_Default,
            );
        }
    }

    pub fn activate_game_overlay_to_store(
        &self,
        app_id: AppId,
        overlay_to_store_flag: OverlayToStoreFlag,
    ) {
        unsafe {
            let overlay_to_store_flag = match overlay_to_store_flag {
                OverlayToStoreFlag::None => sys::EOverlayToStoreFlag::k_EOverlayToStoreFlag_None,
                OverlayToStoreFlag::AddToCart => {
                    sys::EOverlayToStoreFlag::k_EOverlayToStoreFlag_AddToCart
                }
                OverlayToStoreFlag::AddToCartAndShow => {
                    sys::EOverlayToStoreFlag::k_EOverlayToStoreFlag_AddToCartAndShow
                }
            };
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayToStore(
                self.friends,
                app_id.0,
                overlay_to_store_flag,
            );
        }
    }

    pub fn activate_game_overlay_to_user(&self, dialog: &str, user: SteamId) {
        let dialog = CString::new(dialog).unwrap();
        unsafe {
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayToUser(
                self.friends,
                dialog.as_ptr(),
                user.0,
            );
        }
    }

    /// Opens up an invite dialog for the given lobby
    pub fn activate_invite_dialog(&self, lobby: LobbyId) {
        unsafe {
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayInviteDialog(self.friends, lobby.0);
        }
    }

    /// Opens up an invite dialog that will send Rich Presence connect string to friends
    ///
    /// # Panics
    ///
    /// Panics if the `connect` str contains a null byte.
    pub fn activate_invite_dialog_connect_string(&self, connect: &str) {
        let connect = CString::new(connect).unwrap();
        unsafe {
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayInviteDialogConnectString(
                self.friends,
                connect.as_ptr(),
            );
        }
    }

    /// Set rich presence for the user. Unsets the rich presence if `value` is None or empty.
    ///
    /// See [Steam API](https://partner.steamgames.com/doc/api/ISteamFriends#SetRichPresence)
    ///
    /// # Panics
    ///
    /// Panics if the `key` or `value` str slices contain a null byte.
    pub fn set_rich_presence(&self, key: &str, value: Option<&str>) -> bool {
        let key = CString::new(key).unwrap();
        let value = value.map(|v| CString::new(v).unwrap());
        let value_ptr = value
            .as_ref()
            .map_or(std::ptr::null(), |value| value.as_ptr());
        unsafe {
            sys::SteamAPI_ISteamFriends_SetRichPresence(self.friends, key.as_ptr(), value_ptr)
        }
    }

    /// Clears all of the current user's Rich Presence key/values.
    pub fn clear_rich_presence(&self) {
        unsafe {
            sys::SteamAPI_ISteamFriends_ClearRichPresence(self.friends);
        }
    }

    /// The local player's own persona state, as Steam currently sees it.
    pub fn persona_state(&self) -> FriendState {
        unsafe { persona_state_from(sys::SteamAPI_ISteamFriends_GetPersonaState(self.friends)) }
    }

    /// How the local player relates to `friend` (friend, blocked, ignored, ...).
    pub fn friend_relationship(&self, friend: SteamId) -> FriendRelationship {
        unsafe {
            relationship_from(sys::SteamAPI_ISteamFriends_GetFriendRelationship(
                self.friends,
                friend.raw(),
            ))
        }
    }

    /// The friend's Steam level.
    pub fn friend_steam_level(&self, friend: SteamId) -> i32 {
        unsafe { sys::SteamAPI_ISteamFriends_GetFriendSteamLevel(self.friends, friend.raw()) }
    }

    /// The key names present in a friend's rich presence; feed them to
    /// [`Friend::rich_presence`] to read the values back.
    ///
    /// Steam owns the strings it returns here, so each one is copied out before
    /// the next query; a null entry is skipped rather than pushed empty.
    ///
    /// Keys only appear once Steam has the data: see
    /// [`Friends::request_friend_rich_presence`].
    pub fn rich_presence_keys(&self, friend: SteamId) -> Vec<String> {
        let count = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendRichPresenceKeyCount(self.friends, friend.raw())
        };
        if count <= 0 {
            return Vec::new();
        }
        let mut keys = Vec::with_capacity(count as usize);
        for index in 0..count {
            let raw = unsafe {
                sys::SteamAPI_ISteamFriends_GetFriendRichPresenceKeyByIndex(
                    self.friends,
                    friend.raw(),
                    index,
                )
            };
            if raw.is_null() {
                continue;
            }
            let key = unsafe { CStr::from_ptr(raw) };
            keys.push(key.to_string_lossy().into_owned());
        }
        keys
    }

    /// Asks Steam to fetch a friend's rich presence.
    ///
    /// Without this, [`Friends::rich_presence_keys`] and
    /// [`Friend::rich_presence`] usually read nothing: Steam only pulls another
    /// user's presence on request, and then reports changes through
    /// [`FriendRichPresenceUpdate`]. Fire-and-forget, so there is no answer to
    /// check.
    pub fn request_friend_rich_presence(&self, friend: SteamId) {
        unsafe {
            sys::SteamAPI_ISteamFriends_RequestFriendRichPresence(self.friends, friend.raw());
        }
    }

    /// Tells Steam the player is speaking in in-game voice, so the overlay does
    /// not duck game audio twice.
    pub fn set_in_game_voice_speaking(&self, user: SteamId, speaking: bool) {
        unsafe {
            sys::SteamAPI_ISteamFriends_SetInGameVoiceSpeaking(self.friends, user.raw(), speaking)
        }
    }

    /// Chats holding unread priority messages (group chats and the like).
    pub fn unread_priority_chat_count(&self) -> i32 {
        unsafe { sys::SteamAPI_ISteamFriends_GetNumChatsWithUnreadPriorityMessages(self.friends) }
    }

    /// Intercepts friend messages so the game can render them; while enabled,
    /// Steam does not show them itself.
    ///
    /// Turn this on only while something is reading: each hidden message arrives
    /// as [`FriendChatMessageReceived`], is read back with
    /// [`Friends::friend_message`], and is answered with
    /// [`Friends::reply_to_friend_message`]. Enabling it with no reader on the
    /// other side swallows the messages.
    ///
    /// The return value is Steam's acceptance.
    pub fn set_listen_for_friends_messages(&self, intercept: bool) -> bool {
        unsafe { sys::SteamAPI_ISteamFriends_SetListenForFriendsMessages(self.friends, intercept) }
    }

    /// Opens the overlay's Remote Play Together invite dialog for a lobby.
    pub fn open_remote_play_together_invite_dialog(&self, lobby: SteamId) {
        unsafe {
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayRemotePlayTogetherInviteDialog(
                self.friends,
                lobby.raw(),
            )
        }
    }

    /// How many of the user's groups Steam classifies as clans.
    pub fn clan_count(&self) -> i32 {
        unsafe { sys::SteamAPI_ISteamFriends_GetClanCount(self.friends) }
    }

    /// The user's clans, by index.
    pub fn clans(&self) -> Vec<SteamId> {
        let count = self.clan_count();
        if count <= 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(count as usize);
        for index in 0..count {
            let raw = unsafe { sys::SteamAPI_ISteamFriends_GetClanByIndex(self.friends, index) };
            if raw != 0 {
                out.push(SteamId::from_raw(raw));
            }
        }
        out
    }

    /// A clan's full name, once Steam has it.
    ///
    /// Steam fills this in asynchronously, so right after sign-in it can still be
    /// empty: `None` means "no string available", not "the clan is unnamed".
    pub fn clan_name(&self, clan: SteamId) -> Option<String> {
        unsafe {
            steam_string(
                sys::SteamAPI_ISteamFriends_GetClanName(self.friends, clan.raw()),
                |s| s.to_string_lossy().into_owned(),
            )
        }
    }

    /// A clan's short tag as shown in the member list.
    pub fn clan_tag(&self, clan: SteamId) -> Option<String> {
        unsafe {
            steam_string(
                sys::SteamAPI_ISteamFriends_GetClanTag(self.friends, clan.raw()),
                |s| s.to_string_lossy().into_owned(),
            )
        }
    }

    /// The clan's owner.
    pub fn clan_owner(&self, clan: SteamId) -> SteamId {
        unsafe {
            SteamId::from_raw(sys::SteamAPI_ISteamFriends_GetClanOwner(
                self.friends,
                clan.raw(),
            ))
        }
    }

    /// Officer count, valid only after Steam delivered the clan officer list
    /// (`RequestClanOfficerList`, which this wrapper does not expose yet).
    pub fn clan_officer_count(&self, clan: SteamId) -> i32 {
        unsafe { sys::SteamAPI_ISteamFriends_GetClanOfficerCount(self.friends, clan.raw()) }
    }

    /// The clan's officers, indexed like [`Friends::clan_officer_count`] reports.
    pub fn clan_officers(&self, clan: SteamId) -> Vec<SteamId> {
        let count = self.clan_officer_count(clan);
        if count <= 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(count as usize);
        for index in 0..count {
            let raw = unsafe {
                sys::SteamAPI_ISteamFriends_GetClanOfficerByIndex(self.friends, clan.raw(), index)
            };
            if raw != 0 {
                out.push(SteamId::from_raw(raw));
            }
        }
        out
    }

    /// Whether the clan is joinable by anyone.
    pub fn is_clan_public(&self, clan: SteamId) -> bool {
        unsafe { sys::SteamAPI_ISteamFriends_IsClanPublic(self.friends, clan.raw()) }
    }

    /// Whether the clan is Valve-managed and tied to a game.
    pub fn is_clan_official_game_group(&self, clan: SteamId) -> bool {
        unsafe { sys::SteamAPI_ISteamFriends_IsClanOfficialGameGroup(self.friends, clan.raw()) }
    }

    /// Members in a clan's chat channel, before listing them with
    /// `GetChatMemberByIndex` (not wrapped yet).
    pub fn clan_chat_member_count(&self, clan_chat: SteamId) -> i32 {
        unsafe { sys::SteamAPI_ISteamFriends_GetClanChatMemberCount(self.friends, clan_chat.raw()) }
    }

    /// Whether `user` can moderate `clan_chat`.
    pub fn is_clan_chat_admin(&self, clan_chat: SteamId, user: SteamId) -> bool {
        unsafe {
            sys::SteamAPI_ISteamFriends_IsClanChatAdmin(self.friends, clan_chat.raw(), user.raw())
        }
    }

    /// Cached activity for a clan: `(online, in_game, chatting)`.
    ///
    /// `None` means Steam has no numbers for this clan yet -- it is not zero
    /// activity. Ask for fresh figures with
    /// [`Friends::download_clan_activity_counts`].
    pub fn clan_activity(&self, clan: SteamId) -> Option<ClanActivity> {
        let mut online = 0i32;
        let mut in_game = 0i32;
        let mut chatting = 0i32;
        let ok = unsafe {
            sys::SteamAPI_ISteamFriends_GetClanActivityCounts(
                self.friends,
                clan.raw(),
                &mut online,
                &mut in_game,
                &mut chatting,
            )
        };
        ok.then_some(ClanActivity {
            online,
            in_game,
            chatting,
        })
    }

    /// Asks Steam to refresh activity figures for several clans at once.
    ///
    /// The answer arrives as `Ok(true/false)` carrying Steam's own success flag;
    /// the figures themselves are then read back with
    /// [`Friends::clan_activity`], because Steam does not put them in the
    /// response.
    pub fn download_clan_activity_counts<F>(
        &self,
        clans: &[SteamId],
        callback: F,
    ) -> Result<(), SteamError>
    where
        F: FnOnce(Result<bool, SteamError>) + 'static + Send,
    {
        // The FFI takes a CSteamID array; CSteamID is a transparent 64-bit
        // wrapper, so the raw ids are laid out the same way.
        let ids: Vec<u64> = clans.iter().map(|id| id.raw()).collect();
        let api_call = unsafe {
            sys::SteamAPI_ISteamFriends_DownloadClanActivityCounts(
                self.friends,
                ids.as_ptr() as *mut sys::CSteamID,
                ids.len() as i32,
            )
        };
        unsafe {
            register_call_result::<sys::DownloadClanActivityCountsResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        // Steam's own success flag, not a made-up error variant:
                        // a rejected batch is reported as `Ok(false)`.
                        Ok(v.m_bSuccess)
                    })
                },
            );
        }
        Ok(())
    }

    /// Fetches a clan's officer list. Until the answer arrives,
    /// [`Friends::clan_officer_count`] and [`Friends::clan_officers`] report
    /// nothing useful for that clan.
    pub fn request_clan_officer_list<F>(&self, clan: SteamId, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<ClanOfficerListResponse, SteamError>) + 'static + Send,
    {
        let api_call =
            unsafe { sys::SteamAPI_ISteamFriends_RequestClanOfficerList(self.friends, clan.raw()) };
        unsafe {
            register_call_result::<sys::ClanOfficerListResponse_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        Ok(ClanOfficerListResponse {
                            clan: SteamId::from_raw(v.m_steamIDClan.m_steamid.m_unAll64Bits),
                            officer_count: v.m_cOfficers,
                            success: v.m_bSuccess != 0,
                        })
                    })
                },
            );
        }
        Ok(())
    }

    /// Asks Steam how many users follow `user`; the answer arrives as
    /// [`FollowerCount`]. The sibling question is [`Friends::is_following`] --
    /// they are separate round-trips because Steam answers them separately.
    pub fn follower_count<F>(&self, user: SteamId, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<FollowerCount, SteamError>) + 'static + Send,
    {
        let api_call =
            unsafe { sys::SteamAPI_ISteamFriends_GetFollowerCount(self.friends, user.raw()) };
        unsafe {
            register_call_result::<sys::FriendsGetFollowerCount_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult).map(|()| FollowerCount {
                            user: SteamId::from_raw(v.m_steamID.m_steamid.m_unAll64Bits),
                            count: v.m_nCount,
                        })
                    })
                },
            );
        }
        Ok(())
    }

    /// Asks whether the local user follows `other`; the answer arrives as
    /// [`IsFollowingResult`].
    pub fn is_following<F>(&self, other: SteamId, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<IsFollowingResult, SteamError>) + 'static + Send,
    {
        let api_call =
            unsafe { sys::SteamAPI_ISteamFriends_IsFollowing(self.friends, other.raw()) };
        unsafe {
            register_call_result::<sys::FriendsIsFollowing_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult).map(|()| IsFollowingResult {
                            target: SteamId::from_raw(v.m_steamID.m_steamid.m_unAll64Bits),
                            following: v.m_bIsFollowing,
                        })
                    })
                },
            );
        }
        Ok(())
    }

    /// Reads back one of the friend messages Steam stopped from being shown in
    /// the overlay, after [`Friends::set_listen_for_friends_messages`].
    ///
    /// Steam announces a message as [`FriendChatMessageReceived`] and this reads
    /// that announcement's `message_id` back; `max_bytes` is the receive buffer,
    /// capped at [`MAX_CHAT_MESSAGE_BYTES`].
    ///
    /// `None` when Steam copied no bytes for that id.
    pub fn friend_message(
        &self,
        friend: SteamId,
        message_id: i32,
        max_bytes: usize,
    ) -> Option<FriendChatMessage> {
        let capacity = chat_buffer_len(max_bytes);
        let mut buffer = vec![0u8; capacity];
        let mut entry_type: i32 = 0;
        let written = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendMessage(
                self.friends,
                friend.raw(),
                message_id,
                buffer.as_mut_ptr().cast::<c_void>(),
                // Lossless because `chat_buffer_len` caps the value.
                capacity as i32,
                &mut entry_type as *mut i32 as *mut sys::EChatEntryType,
            )
        };
        if written <= 0 {
            return None;
        }
        let written = (written as usize).min(capacity);
        Some(FriendChatMessage {
            kind: chat_entry_type_from(entry_type),
            text: chat_text(&buffer[..written]),
            possibly_truncated: written == capacity,
        })
    }

    /// Answers a friend message out of the game, while message interception is
    /// enabled.
    ///
    /// `Err(SteamError::InvalidParameter)` means the text never reached Steam
    /// (it contains an interior nul); `Ok(false)` is Steam's own refusal.
    pub fn reply_to_friend_message(&self, friend: SteamId, text: &str) -> Result<bool, SteamError> {
        let text = c_string_arg(text)?;
        Ok(unsafe {
            sys::SteamAPI_ISteamFriends_ReplyToFriendMessage(
                self.friends,
                friend.raw(),
                text.as_ptr(),
            )
        })
    }

    /// Joins a clan's chat room.
    ///
    /// Steam answers separately, as [`ClanChatRoomJoined`]; that answer's
    /// `clan_chat` is the id the room reads want
    /// ([`Friends::clan_chat_members`], [`Friends::clan_chat_message`]).
    ///
    /// The answer carries *why* a join failed in Steam's own enum rather than an
    /// error code, so a refused join still arrives as `Ok` with a non-success
    /// `response`.
    pub fn join_clan_chat_room<F>(&self, clan: SteamId, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<ClanChatRoomJoined, SteamError>) + 'static + Send,
    {
        let api_call =
            unsafe { sys::SteamAPI_ISteamFriends_JoinClanChatRoom(self.friends, clan.raw()) };
        unsafe {
            register_call_result::<sys::JoinClanChatRoomCompletionResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        Ok(ClanChatRoomJoined {
                            clan_chat: SteamId::from_raw(
                                v.m_steamIDClanChat.m_steamid.m_unAll64Bits,
                            ),
                            response: ChatRoomEnterResponse::from(
                                v.m_eChatRoomEnterResponse as u32,
                            ),
                        })
                    })
                },
            );
        }
        Ok(())
    }

    /// Leaves a clan chat room previously joined with
    /// [`Friends::join_clan_chat_room`].
    pub fn leave_clan_chat_room(&self, clan_chat: SteamId) -> bool {
        unsafe { sys::SteamAPI_ISteamFriends_LeaveClanChatRoom(self.friends, clan_chat.raw()) }
    }

    /// The members of a clan chat room, gathered by index up to
    /// [`Friends::clan_chat_member_count`]. An index Steam answers with the null
    /// id is skipped rather than returned as a player.
    pub fn clan_chat_members(&self, clan_chat: SteamId) -> Vec<SteamId> {
        let count = self.clan_chat_member_count(clan_chat);
        if count <= 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(count as usize);
        for index in 0..count {
            let raw = unsafe {
                sys::SteamAPI_ISteamFriends_GetChatMemberByIndex(
                    self.friends,
                    clan_chat.raw(),
                    index,
                )
            };
            if raw != 0 {
                out.push(SteamId::from_raw(raw));
            }
        }
        out
    }

    /// Sends a message to a clan chat room.
    ///
    /// `Err(SteamError::InvalidParameter)` means the text never reached Steam
    /// (it contains an interior nul); `Ok(false)` is Steam's own refusal, for
    /// example a room this client has not joined.
    pub fn send_clan_chat_message(
        &self,
        clan_chat: SteamId,
        text: &str,
    ) -> Result<bool, SteamError> {
        let text = c_string_arg(text)?;
        Ok(unsafe {
            sys::SteamAPI_ISteamFriends_SendClanChatMessage(
                self.friends,
                clan_chat.raw(),
                text.as_ptr(),
            )
        })
    }

    /// Reads one message from a clan chat room, by the id Steam announced.
    ///
    /// Unlike [`Friends::friend_message`], this hands back the sender as well --
    /// as an out-parameter Steam may leave empty, which surfaces as
    /// `sender: None` rather than as some other player.
    pub fn clan_chat_message(
        &self,
        clan_chat: SteamId,
        index: i32,
        max_bytes: usize,
    ) -> Option<ClanChatMessage> {
        let capacity = chat_buffer_len(max_bytes);
        let mut buffer = vec![0u8; capacity];
        let mut entry_type: i32 = 0;
        let mut chatter = sys::CSteamID {
            m_steamid: sys::CSteamID_SteamID_t { m_unAll64Bits: 0 },
        };
        let written = unsafe {
            sys::SteamAPI_ISteamFriends_GetClanChatMessage(
                self.friends,
                clan_chat.raw(),
                index,
                buffer.as_mut_ptr().cast::<c_void>(),
                // Lossless because `chat_buffer_len` caps the value.
                capacity as i32,
                &mut entry_type as *mut i32 as *mut sys::EChatEntryType,
                &mut chatter,
            )
        };
        if written <= 0 {
            return None;
        }
        let written = (written as usize).min(capacity);
        let sender = unsafe { chatter.m_steamid.m_unAll64Bits };
        Some(ClanChatMessage {
            sender: (sender != 0).then(|| SteamId::from_raw(sender)),
            kind: chat_entry_type_from(entry_type),
            text: chat_text(&buffer[..written]),
            possibly_truncated: written == capacity,
        })
    }

    /// Whether the overlay already has this clan chat's window open.
    pub fn is_clan_chat_window_open_in_steam(&self, clan_chat: SteamId) -> bool {
        unsafe {
            sys::SteamAPI_ISteamFriends_IsClanChatWindowOpenInSteam(self.friends, clan_chat.raw())
        }
    }

    /// Opens the clan chat in the Steam overlay rather than in the game.
    pub fn open_clan_chat_window_in_steam(&self, clan_chat: SteamId) -> bool {
        unsafe {
            sys::SteamAPI_ISteamFriends_OpenClanChatWindowInSteam(self.friends, clan_chat.raw())
        }
    }

    /// Closes a clan chat window previously opened in the overlay.
    pub fn close_clan_chat_window_in_steam(&self, clan_chat: SteamId) -> bool {
        unsafe {
            sys::SteamAPI_ISteamFriends_CloseClanChatWindowInSteam(self.friends, clan_chat.raw())
        }
    }
}

/// Cached clan activity from [`Friends::clan_activity`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClanActivity {
    pub online: i32,
    pub in_game: i32,
    pub chatting: i32,
}

/// Answers [`Friends::request_clan_officer_list`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClanOfficerListResponse {
    pub clan: SteamId,
    /// How many officers to read back with [`Friends::clan_officers`].
    pub officer_count: i32,
    pub success: bool,
}

/// Answers [`Friends::follower_count`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FollowerCount {
    pub user: SteamId,
    pub count: i32,
}

/// Answers [`Friends::is_following`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsFollowingResult {
    pub target: SteamId,
    pub following: bool,
}

/// A friend's rich presence changed, typically after
/// [`Friends::request_friend_rich_presence`] or while they play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FriendRichPresenceUpdate {
    pub friend: SteamId,
    /// The game the presence belongs to; Steam reports the current app id here.
    pub app_id: AppId,
}

impl_callback!(cb: FriendRichPresenceUpdate_t => FriendRichPresenceUpdate {
    Self {
        friend: SteamId::from_raw(cb.m_steamIDFriend.m_steamid.m_unAll64Bits),
        app_id: AppId(cb.m_nAppID),
    }
});

/// Copies a Steam-owned C string out before the caller's next query, treating a
/// null pointer as "not available" instead of dereferencing it.
unsafe fn steam_string<T>(
    raw: *const std::os::raw::c_char,
    read: impl FnOnce(&CStr) -> T,
) -> Option<T> {
    if raw.is_null() {
        return None;
    }
    Some(read(CStr::from_ptr(raw)))
}

/// Information about a friend's current state in a game
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FriendGame {
    /// The id of the game that the friend is
    /// playing
    pub game: GameId,
    /// The address of the server the player is in
    pub game_address: Ipv4Addr,
    /// The game port of the server the player is in
    pub game_port: u16,
    /// The query port of the server the player is in
    pub query_port: u16,
    /// Optional id of the lobby the player is in
    pub lobby: LobbyId,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PersonaStateChange {
    pub steam_id: SteamId,
    pub flags: PersonaChange,
}

impl_callback!(cb: PersonaStateChange_t => PersonaStateChange {
    Self {
        steam_id: SteamId(cb.m_ulSteamID),
        flags: PersonaChange::from_bits_truncate(cb.m_nChangeFlags as i32),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GameOverlayActivated {
    pub active: bool,
}

impl_callback!(cb: GameOverlayActivated_t => GameOverlayActivated {
    Self {
        active: cb.m_bActive == 1,
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GameLobbyJoinRequested {
    pub lobby_steam_id: LobbyId,
    pub friend_steam_id: SteamId,
}

impl_callback!(cb: GameLobbyJoinRequested_t => GameLobbyJoinRequested {
    Self {
        lobby_steam_id: LobbyId(cb.m_steamIDLobby.m_steamid.m_unAll64Bits),
        friend_steam_id: SteamId(cb.m_steamIDFriend.m_steamid.m_unAll64Bits),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GameRichPresenceJoinRequested {
    /// If you are joining a friend/being invited from a friend, this `SteamId` will be of said friend.
    /// If it's not coming from a friend, this `SteamId` will be invalid, which you can check with the `.is_invalid()`
    /// method.
    pub friend_steam_id: SteamId,
    /// the connect string, holding custom data to join a game or friend
    pub connect: String,
}

impl_callback!(cb: GameRichPresenceJoinRequested_t => GameRichPresenceJoinRequested {
    // Convert from &[i8] to &[u8] because c_char in C is signed.
    // Technically, this C string does not have to be UTF-8, but I think for all realistic uses it will be.
    let as_bytes = cb.m_rgchConnect.map(|c| c as u8);
    let connect = CStr::from_bytes_until_nul(&as_bytes)
        .expect("Connect string payload was not a valid C string");
    let connect = connect
        .to_str()
        .expect("Connect string payload was not valid UTF-8")
        .to_string();

    let friend_steam_id = SteamId::from_raw(cb.m_steamIDFriend.m_steamid.m_unAll64Bits);

    GameRichPresenceJoinRequested {
        friend_steam_id,
        connect,
    }
});

pub struct Friend {
    id: SteamId,
    friends: *mut sys::ISteamFriends,
    _inner: Arc<Inner>,
}

impl Debug for Friend {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Friend({:?})", self.id)
    }
}

impl Friend {
    pub fn id(&self) -> SteamId {
        self.id
    }

    pub fn name(&self) -> String {
        unsafe {
            let name = sys::SteamAPI_ISteamFriends_GetFriendPersonaName(self.friends, self.id.0);
            let name = CStr::from_ptr(name);
            name.to_string_lossy().into_owned()
        }
    }
    /// Gets the nickname that the current user has set for the specified user.
    pub fn nick_name(&self) -> Option<String> {
        unsafe {
            let name = sys::SteamAPI_ISteamFriends_GetPlayerNickname(self.friends, self.id.0);
            if name.is_null() {
                return None;
            }

            let name = CStr::from_ptr(name);
            if name.is_empty() {
                None
            } else {
                Some(name.to_string_lossy().into_owned())
            }
        }
    }

    pub fn state(&self) -> FriendState {
        unsafe {
            persona_state_from(sys::SteamAPI_ISteamFriends_GetFriendPersonaState(
                self.friends,
                self.id.0,
            ))
        }
    }

    /// Returns information about the game the player is current playing if any
    pub fn game_played(&self) -> Option<FriendGame> {
        unsafe {
            let mut info: sys::FriendGameInfo_t = std::mem::zeroed();
            if sys::SteamAPI_ISteamFriends_GetFriendGamePlayed(self.friends, self.id.0, &mut info) {
                Some(FriendGame {
                    game: GameId(std::mem::transmute(info.m_gameID)),
                    game_address: info.m_unGameIP.into(),
                    game_port: info.m_usGamePort,
                    query_port: info.m_usQueryPort,
                    lobby: LobbyId(info.m_steamIDLobby.m_steamid.m_unAll64Bits),
                })
            } else {
                None
            }
        }
    }
    /// Gets the app ID of the game that user played with someone on their recently-played-with list.
    pub fn coplay_game_played(&self) -> AppId {
        unsafe {
            let app_id = sys::SteamAPI_ISteamFriends_GetFriendCoplayGame(self.friends, self.id.0);
            AppId(app_id)
        }
    }

    /// Gets the timestamp of when the user played with someone on their recently-played-with list.
    pub fn coplay_time(&self) -> i32 {
        unsafe { sys::SteamAPI_ISteamFriends_GetFriendCoplayTime(self.friends, self.id.0) }
    }

    /// Returns a small (32x32) avatar for the user in RGBA format
    pub fn small_avatar(&self) -> Option<Vec<u8>> {
        unsafe {
            let utils = sys::SteamAPI_SteamUtils_v010();
            let img = sys::SteamAPI_ISteamFriends_GetSmallFriendAvatar(self.friends, self.id.0);
            if img == 0 {
                return None;
            }
            let mut width = 0;
            let mut height = 0;
            if !sys::SteamAPI_ISteamUtils_GetImageSize(utils, img, &mut width, &mut height) {
                return None;
            }
            let mut dest = vec![0; 32 * 32 * 4];
            if !sys::SteamAPI_ISteamUtils_GetImageRGBA(utils, img, dest.as_mut_ptr(), 32 * 32 * 4) {
                return None;
            }
            Some(dest)
        }
    }

    /// Returns a medium (64x64) avatar for the user in RGBA format
    pub fn medium_avatar(&self) -> Option<Vec<u8>> {
        unsafe {
            let utils = sys::SteamAPI_SteamUtils_v010();
            let img = sys::SteamAPI_ISteamFriends_GetMediumFriendAvatar(self.friends, self.id.0);
            if img == 0 {
                return None;
            }
            let mut width = 0;
            let mut height = 0;
            if !sys::SteamAPI_ISteamUtils_GetImageSize(utils, img, &mut width, &mut height) {
                return None;
            }
            let mut dest = vec![0; 64 * 64 * 4];
            if !sys::SteamAPI_ISteamUtils_GetImageRGBA(utils, img, dest.as_mut_ptr(), 64 * 64 * 4) {
                return None;
            }
            Some(dest)
        }
    }

    /// Returns a large (184x184) avatar for the user in RGBA format
    pub fn large_avatar(&self) -> Option<Vec<u8>> {
        unsafe {
            let utils = sys::SteamAPI_SteamUtils_v010();
            let img = sys::SteamAPI_ISteamFriends_GetLargeFriendAvatar(self.friends, self.id.0);
            if img == 0 {
                return None;
            }
            let mut width = 0;
            let mut height = 0;
            if !sys::SteamAPI_ISteamUtils_GetImageSize(utils, img, &mut width, &mut height) {
                return None;
            }
            let mut dest = vec![0; 184 * 184 * 4];
            if !sys::SteamAPI_ISteamUtils_GetImageRGBA(utils, img, dest.as_mut_ptr(), 184 * 184 * 4)
            {
                return None;
            }
            Some(dest)
        }
    }

    /// Checks if the user meets the specified criteria. (Friends, blocked, users on the same server, etc)
    pub fn has_friend(&self, flags: FriendFlags) -> bool {
        unsafe { sys::SteamAPI_ISteamFriends_HasFriend(self.friends, self.id.0, flags.bits() as _) }
    }

    /// Invites a friend or clan member to the current game using a special invite string.
    /// If the target user accepts the invite then the ConnectString gets added to the command-line when launching the game.
    /// If the game is already running for that user, then they will receive a GameRichPresenceJoinRequested_t callback with the connect string.
    pub fn invite_user_to_game(&self, connect_string: &str) {
        unsafe {
            let connect_string = CString::new(connect_string).unwrap();
            sys::SteamAPI_ISteamFriends_InviteUserToGame(
                self.friends,
                self.id.0,
                connect_string.as_ptr(),
            );
        }
    }

    /// Mark a target user as 'played with'.
    /// NOTE: The current user must be in game with the other player for the association to work.
    pub fn set_played_with(&self) {
        unsafe {
            sys::SteamAPI_ISteamFriends_SetPlayedWith(self.friends, self.id.0);
        }
    }

    /// Get a Rich Presence value from a specified friend.
    pub fn rich_presence(&self, key: &str) -> Option<String> {
        let key = CString::new(key).unwrap();
        let value = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendRichPresence(self.friends, self.id.0, key.as_ptr())
        };
        if value.is_null() {
            return None;
        }
        let cstr = unsafe { CStr::from_ptr(value) };
        if cstr.is_empty() {
            return None;
        }
        Some(cstr.to_string_lossy().into_owned())
    }
}

/// Maps Steam's persona-state answer.
///
/// `k_EPersonaStateMax` is a real answer for a state this binding does not name
/// -- the previous code reached `unreachable!()` on it, which panicked.
fn persona_state_from(state: sys::EPersonaState) -> FriendState {
    match state {
        sys::EPersonaState::k_EPersonaStateOffline => FriendState::Offline,
        sys::EPersonaState::k_EPersonaStateOnline => FriendState::Online,
        sys::EPersonaState::k_EPersonaStateInvisible => FriendState::Invisible,
        sys::EPersonaState::k_EPersonaStateBusy => FriendState::Busy,
        sys::EPersonaState::k_EPersonaStateAway => FriendState::Away,
        sys::EPersonaState::k_EPersonaStateSnooze => FriendState::Snooze,
        sys::EPersonaState::k_EPersonaStateLookingToPlay => FriendState::LookingToPlay,
        sys::EPersonaState::k_EPersonaStateLookingToTrade => FriendState::LookingToTrade,
        _ => FriendState::Unknown,
    }
}

/// How the local player relates to another user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FriendRelationship {
    /// Not friends, and no request pending.
    None,
    Blocked,
    /// They asked to be friends; a reply is owed.
    RequestRecipient,
    Friend,
    /// We asked to be friends; waiting on their reply.
    RequestInitiator,
    Ignored,
    /// Ignored, but still a friend.
    IgnoredFriend,
    /// A relationship value this binding does not name.
    Unknown,
}

fn relationship_from(relationship: sys::EFriendRelationship) -> FriendRelationship {
    match relationship {
        sys::EFriendRelationship::k_EFriendRelationshipNone => FriendRelationship::None,
        sys::EFriendRelationship::k_EFriendRelationshipBlocked => FriendRelationship::Blocked,
        sys::EFriendRelationship::k_EFriendRelationshipRequestRecipient => {
            FriendRelationship::RequestRecipient
        }
        sys::EFriendRelationship::k_EFriendRelationshipFriend => FriendRelationship::Friend,
        sys::EFriendRelationship::k_EFriendRelationshipRequestInitiator => {
            FriendRelationship::RequestInitiator
        }
        sys::EFriendRelationship::k_EFriendRelationshipIgnored => FriendRelationship::Ignored,
        sys::EFriendRelationship::k_EFriendRelationshipIgnoredFriend => {
            FriendRelationship::IgnoredFriend
        }
        // Includes `k_EFriendRelationshipSuggested_DEPRECATED` (Valve retired it)
        // and `k_EFriendRelationshipMax`, so neither can panic the caller.
        _ => FriendRelationship::Unknown,
    }
}

/// The most one chat read will ask Steam to fill.
///
/// Chat messages are short, and `cubData`/`cchTextMax` are `int` arguments: an
/// unchecked `usize` would wrap negative and Steam would then be handed a
/// nonsense length.
pub const MAX_CHAT_MESSAGE_BYTES: usize = 64 * 1024;

/// One intercepted friend message, read back with
/// [`Friends::friend_message`].
#[derive(Clone, Debug, PartialEq)]
pub struct FriendChatMessage {
    /// What Steam says this entry is: text, a typing notification, an emote.
    pub kind: ChatEntryType,
    pub text: String,
    /// Steam filled the whole buffer, so `text` may be a prefix of a longer
    /// message.
    pub possibly_truncated: bool,
}

/// One clan chat message, read back with [`Friends::clan_chat_message`].
#[derive(Clone, Debug, PartialEq)]
pub struct ClanChatMessage {
    /// `None` when Steam left the sender empty.
    pub sender: Option<SteamId>,
    pub kind: ChatEntryType,
    pub text: String,
    pub possibly_truncated: bool,
}

/// The answer to [`Friends::join_clan_chat_room`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClanChatRoomJoined {
    pub clan_chat: SteamId,
    pub response: ChatRoomEnterResponse,
}

/// A friend message arrived and Steam has hidden it from the overlay, because
/// [`Friends::set_listen_for_friends_messages`] is on. Read it back with
/// [`Friends::friend_message`] using this callback's `message_id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FriendChatMessageReceived {
    pub sender: SteamId,
    /// Pass this to [`Friends::friend_message`].
    pub message_id: i32,
}

impl_callback!(cb: GameConnectedFriendChatMsg_t => FriendChatMessageReceived {
    Self {
        sender: SteamId::from_raw(cb.m_steamIDUser.m_steamid.m_unAll64Bits),
        message_id: cb.m_iMessageID,
    }
});

/// A message arrived in a clan chat this client is connected to. Read it back
/// with [`Friends::clan_chat_message`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClanChatMessageReceived {
    pub clan_chat: SteamId,
    pub sender: SteamId,
    /// The id [`Friends::clan_chat_message`] reads back.
    pub message_id: i32,
}

impl_callback!(cb: GameConnectedClanChatMsg_t => ClanChatMessageReceived {
    Self {
        clan_chat: SteamId::from_raw(cb.m_steamIDClanChat.m_steamid.m_unAll64Bits),
        sender: SteamId::from_raw(cb.m_steamIDUser.m_steamid.m_unAll64Bits),
        message_id: cb.m_iMessageID,
    }
});

/// Some chat's unread count changed; Steam gives no numbers with it, so re-read
/// [`Friends::unread_priority_chat_count`] and the messages themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnreadChatMessagesChanged;

impl_callback!(_cb: UnreadChatMessagesChanged_t => UnreadChatMessagesChanged {
    Self
});

/// How much of a chat message to ask Steam for, kept inside `int` range.
fn chat_buffer_len(requested: usize) -> usize {
    requested.clamp(1, MAX_CHAT_MESSAGE_BYTES)
}

/// The text of a chat entry, cut where Steam ended it.
///
/// Steam does not promise a nul inside the bytes it reports, and does not
/// promise valid UTF-8, so this stops at the first nul and replaces the rest.
fn chat_text(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Maps Steam's `EChatEntryType` answer.
///
/// The wrapper reads the out-parameter as a plain 32-bit integer instead of a
/// `sys::EChatEntryType`: the generated enum is `#[repr(i32)]` on Windows and
/// `#[repr(u32)]` elsewhere, and both are `#[non_exhaustive]`, so a value Steam
/// has not named here must not be turned into one of its variants.
fn chat_entry_type_from(raw: i32) -> ChatEntryType {
    u8::try_from(raw)
        .map(ChatEntryType::from)
        .unwrap_or(ChatEntryType::Invalid)
}

#[cfg(test)]
mod state_mapping_tests {
    use super::*;

    #[test]
    fn steam_owned_strings_survive_a_null_answer() {
        // A null answer is Steam saying "not cached yet"; reading it as an empty
        // name would be a different claim than the one it made.
        let null = std::ptr::null::<std::os::raw::c_char>();
        assert_eq!(
            unsafe { steam_string(null, |s| s.to_string_lossy().into_owned()) },
            None
        );

        let name = CString::new("Delta Squad").unwrap();
        assert_eq!(
            unsafe { steam_string(name.as_ptr(), |s| s.to_string_lossy().into_owned()) }.as_deref(),
            Some("Delta Squad")
        );
    }
    #[test]
    fn unknown_persona_state_is_not_a_panic() {
        // The previous mapping ended in `unreachable!()`, and Steam does answer
        // with the sentinel for states this binding does not name.
        assert_eq!(
            persona_state_from(sys::EPersonaState::k_EPersonaStateMax),
            FriendState::Unknown
        );
        let named = [
            (
                sys::EPersonaState::k_EPersonaStateOffline,
                FriendState::Offline,
            ),
            (
                sys::EPersonaState::k_EPersonaStateOnline,
                FriendState::Online,
            ),
            (
                sys::EPersonaState::k_EPersonaStateInvisible,
                FriendState::Invisible,
            ),
            (sys::EPersonaState::k_EPersonaStateBusy, FriendState::Busy),
            (sys::EPersonaState::k_EPersonaStateAway, FriendState::Away),
            (
                sys::EPersonaState::k_EPersonaStateSnooze,
                FriendState::Snooze,
            ),
            (
                sys::EPersonaState::k_EPersonaStateLookingToPlay,
                FriendState::LookingToPlay,
            ),
            (
                sys::EPersonaState::k_EPersonaStateLookingToTrade,
                FriendState::LookingToTrade,
            ),
        ];
        let mut seen = vec![];
        for (raw, expected) in named {
            assert_eq!(persona_state_from(raw), expected);
            seen.push(persona_state_from(raw));
        }
        // Distinct Steam states must not collapse into one game-visible state.
        for i in 0..seen.len() {
            for j in (i + 1)..seen.len() {
                assert_ne!(seen[i], seen[j]);
            }
        }
    }

    #[test]
    fn relationships_map_onto_distinct_values_and_the_retired_one_is_unknown() {
        use sys::EFriendRelationship as R;
        let mapped: Vec<FriendRelationship> = [
            R::k_EFriendRelationshipNone,
            R::k_EFriendRelationshipBlocked,
            R::k_EFriendRelationshipRequestRecipient,
            R::k_EFriendRelationshipFriend,
            R::k_EFriendRelationshipRequestInitiator,
            R::k_EFriendRelationshipIgnored,
            R::k_EFriendRelationshipIgnoredFriend,
        ]
        .iter()
        .map(|raw| relationship_from(*raw))
        .collect();
        assert_eq!(mapped[0], FriendRelationship::None);
        assert_eq!(mapped[3], FriendRelationship::Friend);
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(mapped[i], mapped[j]);
            }
        }
        assert_eq!(
            relationship_from(R::k_EFriendRelationshipSuggested_DEPRECATED),
            FriendRelationship::Unknown,
            "a retired Valve value must not masquerade as a live relationship"
        );
        assert_eq!(
            relationship_from(R::k_EFriendRelationshipMax),
            FriendRelationship::Unknown
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FriendState {
    /// Steam answered with a state this binding does not name (including its
    /// `k_EPersonaStateMax` sentinel) rather than one of the known values below.
    Unknown,
    Offline,
    Online,
    Invisible,
    Busy,
    Away,
    Snooze,
    LookingToTrade,
    LookingToPlay,
}

#[cfg(test)]
mod chat_tests {
    use super::*;

    #[test]
    fn chat_buffer_len_keeps_the_length_argument_an_int() {
        // `cubData`/`cchTextMax` are `int` arguments: an uncapped `usize` would
        // wrap negative, and Steam would be handed a nonsense length for a
        // buffer this side never allocated.
        assert_eq!(chat_buffer_len(0), 1);
        assert_eq!(chat_buffer_len(64), 64);
        assert_eq!(
            chat_buffer_len(MAX_CHAT_MESSAGE_BYTES),
            MAX_CHAT_MESSAGE_BYTES
        );
        assert_eq!(chat_buffer_len(usize::MAX), MAX_CHAT_MESSAGE_BYTES);
        for requested in [
            0,
            1,
            7,
            MAX_CHAT_MESSAGE_BYTES,
            MAX_CHAT_MESSAGE_BYTES + 1,
            MAX_CHAT_MESSAGE_BYTES * 2,
            i32::MAX as usize,
            usize::MAX,
        ] {
            let len = chat_buffer_len(requested);
            assert!(
                i32::try_from(len).is_ok(),
                "{requested} -> {len} does not fit the int argument"
            );
            assert!(
                len >= 1,
                "{requested} -> {len} leaves Steam a zero-length buffer"
            );
        }
    }

    #[test]
    fn chat_text_stops_where_steam_ended_it() {
        assert_eq!(chat_text(b"hello\0trailing"), "hello");
        assert_eq!(chat_text(b"no terminator"), "no terminator");
        assert_eq!(chat_text(b""), "");
        assert_eq!(chat_text(b"\0"), "");
        // Steam hands back bytes, not promised UTF-8: a lone 0xFF must not panic
        // someone reading a chat message.
        assert_eq!(chat_text(&[0xff, b'a']), "\u{fffd}a");
    }

    #[test]
    fn entry_type_answer_is_never_wrapped_to_fit() {
        use ChatEntryType::*;
        let named = [
            (1, ChatMsg),
            (2, Typing),
            (3, InviteGame),
            (4, Emote),
            (6, LeftConversation),
            (7, Entered),
            (8, WasKicked),
            (9, WasBanned),
            (10, Disconnected),
            (11, HistoricalChat),
            (14, LinkBlocked),
        ];
        for (code, expected) in named {
            assert_eq!(chat_entry_type_from(code), expected);
        }
        let mapped: Vec<ChatEntryType> = named.iter().map(|(_, value)| *value).collect();
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(mapped[i], mapped[j], "distinct Steam codes must not merge");
            }
        }
        // Gaps in Valve's numbering are not aliases for a neighbour.
        for gap in [0, 5, 12, 13, 15, 200] {
            assert_eq!(
                chat_entry_type_from(gap),
                Invalid,
                "{gap} is not a named code"
            );
        }
        // 257 would read back as `ChatMsg` if the raw int were cast down to `u8`
        // instead of range-checked first.
        assert_eq!(chat_entry_type_from(257), Invalid);
        assert_eq!(chat_entry_type_from(-1), Invalid);
        assert_eq!(chat_entry_type_from(i32::MAX), Invalid);
    }

    #[test]
    fn chat_text_argument_with_an_interior_nul_is_refused_not_panicked() {
        assert!(c_string_arg("plain").is_ok());
        assert_eq!(
            c_string_arg("bad\0text").err(),
            Some(SteamError::InvalidParameter)
        );
    }
}
