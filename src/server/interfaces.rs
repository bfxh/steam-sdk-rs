//! The sub-interface accessors -- `ugc`, `http`, `inventory`, `stats`, `utils`,
//! `networking`, `networking_messages`, `networking_sockets` -- each handing out the same
//! wrapper the client uses, built on this server's own interface pointer. The ninth one, `apps`,
//! travels here as the block comment the file already held -- it opens with the words "TODO: Buggy
//! currently?" and swallows the whole method -- because deleting it would be a diagnosis, not a
//! move. Any ruler that counts `pub fn` or `debug_assert` textually over this file therefore reads
//! one more site than actually compiles.

use super::*;

impl Server {
    /// Returns an accessor to the steam UGC interface (steam workshop)
    ///
    /// **For this to work properly, you need to call `UGC::init_for_game_server()`!**
    pub fn ugc(&self) -> UGC {
        unsafe {
            let ugc = sys::SteamAPI_SteamGameServerUGC_v021();
            debug_assert!(!ugc.is_null());
            UGC {
                ugc,
                inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the game server HTTP interface
    ///
    /// Steam hands back the same `ISteamHTTP` interface a client uses, so the
    /// request/cookie wrappers here are identical to [`crate::Client::http`].
    pub fn http(&self) -> Http {
        unsafe {
            let http = sys::SteamAPI_SteamGameServerHTTP_v003();
            debug_assert!(!http.is_null());
            Http {
                http,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the game server inventory interface
    ///
    /// As with [`Server::http`], Steam returns the same `ISteamInventory`
    /// interface, so [`crate::Inventory`] applies unchanged.
    pub fn inventory(&self) -> Inventory {
        unsafe {
            let inv = sys::SteamAPI_SteamGameServerInventory_v003();
            debug_assert!(!inv.is_null());
            Inventory {
                inv,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the game server stats interface
    ///
    /// Unlike [`Server::http`] this is a distinct Steam interface: it reads and
    /// writes stats of users who played on this server.
    pub fn stats(&self) -> GameServerStats {
        unsafe {
            let gs = sys::SteamAPI_SteamGameServerStats_v001();
            debug_assert!(!gs.is_null());
            GameServerStats {
                gs,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam utils interface
    pub fn utils(&self) -> Utils {
        unsafe {
            let utils = sys::SteamAPI_SteamGameServerUtils_v010();
            debug_assert!(!utils.is_null());
            Utils {
                utils: utils,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam networking interface
    pub fn networking(&self) -> Networking {
        unsafe {
            let net = sys::SteamAPI_SteamGameServerNetworking_v006();
            debug_assert!(!net.is_null());
            Networking {
                net: net,
                _inner: self.inner.clone(),
            }
        }
    }

    pub fn networking_messages(&self) -> networking_messages::NetworkingMessages {
        unsafe {
            let net = sys::SteamAPI_SteamGameServerNetworkingMessages_SteamAPI_v002();
            debug_assert!(!net.is_null());
            networking_messages::NetworkingMessages {
                net,
                inner: self.inner.clone(),
            }
        }
    }

    pub fn networking_sockets(&self) -> networking_sockets::NetworkingSockets {
        unsafe {
            let sockets = sys::SteamAPI_SteamGameServerNetworkingSockets_SteamAPI_v012();
            debug_assert!(!sockets.is_null());
            networking_sockets::NetworkingSockets {
                sockets,
                inner: self.inner.clone(),
            }
        }
    }

    /* TODO: Buggy currently?
    /// Returns an accessor to the steam apps interface
    pub fn apps(&self) -> Apps {
        unsafe {
            let apps = sys::steam_rust_get_server_apps();
            debug_assert!(!apps.is_null());
            Apps {
                apps: apps,
                _inner: self.inner.clone(),
            }
        }
    }
    */
}
