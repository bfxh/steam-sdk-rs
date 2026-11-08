//! Clans and their metadata -- count, roster, name, tag, owner, officers, public and official
//! game group flags, chat member counts and the activity feed.

use super::*;

impl Friends {
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
}
