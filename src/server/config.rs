//! Everything the server announces about itself before and while it runs -- product,
//! description, map, name, player caps, tags, key/value pairs, heartbeat and advertise flags
//! -- all fire-and-forget calls into `ISteamGameServer`.

use super::*;

impl Server {
    /// Sets the game product identifier. This is currently used by the master server for version
    /// checking purposes. Converting the games app ID to a string for this is recommended.
    ///
    /// This is required for all game servers and can only be set before calling
    /// log_on() or log_on_anonymous().
    pub fn set_product(&self, product: &str) {
        let product = CString::new(product).unwrap();
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetProduct(self.server, product.as_ptr());
        }
    }

    /// Sets the game description. Setting this to the full name of your game is recommended.
    ///
    /// This is required for all game servers and can only be set before calling
    /// log_on() or log_on_anonymous().
    pub fn set_game_description(&self, desc: &str) {
        let desc = CString::new(desc).unwrap();
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetGameDescription(self.server, desc.as_ptr());
        }
    }

    /// Sets a string defining the "gamedata" for this server, this is optional, but if set it
    /// allows users to filter in the matchmaking/server-browser interfaces based on the value.
    ///
    /// This is usually formatted as a comma or semicolon separated list.
    ///
    /// Don't set this unless it actually changes, its only uploaded to the master once; when
    /// acknowledged.
    pub fn set_game_data(&self, data: &str) {
        let desc = CString::new(data).unwrap();
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetGameData(self.server, desc.as_ptr());
        }
    }

    /// Sets whether this server is dedicated or a listen server.
    pub fn set_dedicated_server(&self, dedicated: bool) {
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetDedicatedServer(self.server, dedicated);
        }
    }

    /// Login to a generic anonymous account
    pub fn log_on_anonymous(&self) {
        unsafe {
            sys::SteamAPI_ISteamGameServer_LogOnAnonymous(self.server);
        }
    }

    /// Login to a generic account by token
    pub fn log_on(&self, token: &str) {
        let token = CString::new(token).unwrap();
        unsafe {
            sys::SteamAPI_ISteamGameServer_LogOn(self.server, token.as_ptr());
        }
    }

    /// If active, updates the master server with this server's presence so players can find it via
    /// the steam matchmaking/server browser interfaces.
    pub fn enable_heartbeats(&self, active: bool) {
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetAdvertiseServerActive(self.server, active);
        }
    }

    /// Indicate whether you wish to be listed on the master server list
    /// and/or respond to server browser / LAN discovery packets.
    /// The server starts with this value set to false.  You should set all
    /// relevant server parameters before enabling advertisement on the server.
    ///
    /// (This function used to be named EnableHeartbeats, so if you are wondering
    /// where that function went, it's right here.  It does the same thing as before,
    /// the old name was just confusing.)
    #[inline(always)]
    pub fn set_advertise_server_active(&self, active: bool) {
        self.enable_heartbeats(active);
    }

    /// If your game is a "mod," pass the string that identifies it.  The default is an empty
    /// string, meaning this application is the original game, not a mod.
    pub fn set_mod_dir(&self, mod_dir: &str) {
        let mod_dir = CString::new(mod_dir).unwrap();
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetModDir(self.server, mod_dir.as_ptr());
        }
    }

    /// Set name of map to report in the server browser
    pub fn set_map_name(&self, map_name: &str) {
        let map_name = CString::new(map_name).unwrap();
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetMapName(self.server, map_name.as_ptr());
        }
    }

    /// Set the name of server as it will appear in the server browser
    pub fn set_server_name(&self, server_name: &str) {
        let server_name = CString::new(server_name).unwrap();
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetServerName(self.server, server_name.as_ptr());
        }
    }

    /// Sets the maximum number of players allowed on the server at once.
    ///
    /// This value may be changed at any time.
    pub fn set_max_players(&self, count: i32) {
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetMaxPlayerCount(self.server, count);
        }
    }

    /// Sets a string defining the "gametags" for this server, this is optional, but if set it
    /// allows users to filter in the matchmaking/server-browser interfaces based on the value.
    ///
    /// This is usually formatted as a comma or semicolon separated list.
    ///
    /// Don't set this unless it actually changes, its only uploaded to the master once;
    /// when acknowledged.
    ///
    /// The new "gametags" value to set. Must not be an empty string ("").
    /// This can not be longer than 127.
    pub fn set_game_tags(&self, tags: &str) {
        assert!(tags.len() != 0, "tags must not be an empty string (\"\").");
        assert!(tags.len() < 128, "tags can not be longer than 127.");

        let tags = CString::new(tags).unwrap();
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetGameTags(self.server, tags.as_ptr());
        }
    }

    /// Add/update a rules key/value pair.
    pub fn set_key_value(&self, key: &str, value: &str) {
        let key = CString::new(key).unwrap();
        let value = CString::new(value).unwrap();

        unsafe {
            sys::SteamAPI_ISteamGameServer_SetKeyValue(self.server, key.as_ptr(), value.as_ptr());
        }
    }

    /// Clears the whole list of key/values that are sent in rules queries.
    pub fn clear_all_key_values(&self) {
        unsafe {
            sys::SteamAPI_ISteamGameServer_ClearAllKeyValues(self.server);
        }
    }

    /// Let people know if your server will require a password
    pub fn set_password_protected(&self, b_password_protected: bool) {
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetPasswordProtected(self.server, b_password_protected);
        }
    }

    /// Number of bots.  Default value is zero
    pub fn set_bot_player_count(&self, c_bot_players: i32) {
        unsafe {
            sys::SteamAPI_ISteamGameServer_SetBotPlayerCount(self.server, c_bot_players);
        }
    }
}
