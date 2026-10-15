//! The two `ISteamUser` calls whose parameter vocabulary the SDK never declares:
//! `AdvertiseGame`'s raw `uint32` server IP (`isteamuser.h:156-159`) and
//! `TrackAppUsageEvent`'s `int eAppUsageEvent`, which sits under the header's own
//! `// Legacy functions` banner and has no matching enum in this binding (h:63-65).
//!
//! Both are wrapped anyway, because in each case the missing vocabulary can be *forwarded* without
//! inventing one -- but only if the wrapper says out loud which assumption it is making. Where the
//! header is silent, the assumption is stated in the method docs and the raw `sys::` entry stays the
//! escape hatch for anyone who reads it differently.

use super::*;
use crate::app::c_string_arg;
use std::net::Ipv4Addr;

impl User {
    /// Publishes this game server's address to the local user's friend list.
    ///
    /// The header's whole specification is "set data to be replicated to friends so that they can
    /// join your game", plus "`uint32 unIPServer, uint16 usPortServer` - the IP address of the game
    /// server" (`isteamuser.h:156-158`). Notably it never states the byte order of that `uint32`, so
    /// this wrapper adopts the convention the crate already uses in *both* directions and says so:
    ///
    ///   * outbound -- `MatchmakingServers::ping_server` passes `ip.into()` to Steam's `uint32`
    ///     address argument (`src/matchmaking_servers.rs:447-458`),
    ///   * inbound -- the server-list reader turns an address Steam reported back into
    ///     `Ipv4Addr::from(raw.m_NetAdr.m_unIP)` (`src/matchmaking_servers.rs:213`).
    ///
    /// Those two round-trip, and they treat the integer as the four octets with the first one most
    /// significant, so `192.168.1.1` is `0xC0A80101`. Measured on this host, `u32::from(addr)` and
    /// `addr.to_bits()` give that same value, which is why the choice is not a coin flip between two
    /// spellings but the shape Steam itself hands back.
    ///
    /// Steam returns nothing from this call, so there is no outcome to report: an unreachable server
    /// address is still "accepted" here. A caller who reads the word order differently should take
    /// `sys::SteamAPI_ISteamUser_AdvertiseGame` directly -- a deliberate choice beats a wrapper
    /// guessing.
    pub fn advertise_game(&self, server: SteamId, address: Ipv4Addr, port: u16) {
        unsafe {
            sys::SteamAPI_ISteamUser_AdvertiseGame(self.user, server.0, u32::from(address), port)
        }
    }

    /// Reports a usage event against `game_id`, with Steam's free-form `extra_info`.
    ///
    /// `eAppUsageEvent` is declared as a bare `int` and this SDK ships no `EAppUsageEvent` to name
    /// its values, so the number is forwarded unchanged and *not* wrapped in a Rust enum: an enum
    /// would be a table of names this binding cannot check, which is the same mistake the
    /// write-direction placeholders were called out for. The header's only remark on the entry is
    /// "used by only a few games to track usage events" (`isteamuser.h:63-64`), under its own
    /// `// Legacy functions` banner -- so this is a passthrough with a documented meaning, not a
    /// modelled API.
    ///
    /// Steam returns `void`, so the only failure available is on our side: `extra_info` reaches Steam
    /// as a C string, and an interior NUL is answered with [`SteamError::InvalidParameter`] instead
    /// of a panic or a silently truncated note. Passing `""` is what Steam's own default argument
    /// means, and is allowed.
    pub fn track_app_usage_event(
        &self,
        game_id: GameId,
        event: i32,
        extra_info: &str,
    ) -> Result<(), SteamError> {
        let extra = c_string_arg(extra_info)?;
        unsafe {
            sys::SteamAPI_ISteamUser_TrackAppUsageEvent(self.user, game_id.0, event, extra.as_ptr())
        }
        Ok(())
    }
}
