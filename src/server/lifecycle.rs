//! Starting the server and keeping it fed: the raw `SteamGameServer_InitEx` call behind
//! `Server::init`, the two callback pumps, the registration helper they share, and the
//! server's own Steam id.

use super::*;
use crate::utils::try_c_string;

impl Server {
    fn steam_game_server_init_ex(
        un_ip: std::ffi::c_uint,
        us_game_port: std::ffi::c_ushort,
        us_query_port: std::ffi::c_ushort,
        e_server_mode: EServerMode,
        pch_version_string: *const c_char,
        p_out_err_msg: *mut SteamErrMsg,
    ) -> ESteamAPIInitResult {
        let versions: Vec<&[u8]> = vec![
            sys::STEAMUTILS_INTERFACE_VERSION,
            sys::STEAMNETWORKINGUTILS_INTERFACE_VERSION,
            sys::STEAMGAMESERVER_INTERFACE_VERSION,
            sys::STEAMGAMESERVERSTATS_INTERFACE_VERSION,
            sys::STEAMHTTP_INTERFACE_VERSION,
            sys::STEAMINVENTORY_INTERFACE_VERSION,
            sys::STEAMNETWORKING_INTERFACE_VERSION,
            sys::STEAMNETWORKINGMESSAGES_INTERFACE_VERSION,
            sys::STEAMNETWORKINGSOCKETS_INTERFACE_VERSION,
            sys::STEAMUGC_INTERFACE_VERSION,
            b"\0",
        ];

        let merged_versions: Vec<u8> = versions.into_iter().flatten().cloned().collect();
        let merged_versions_ptr = merged_versions.as_ptr().cast::<::std::os::raw::c_char>();

        return unsafe {
            sys::SteamInternal_GameServer_Init_V2(
                un_ip,
                us_game_port,
                us_query_port,
                e_server_mode,
                pch_version_string,
                merged_versions_ptr,
                p_out_err_msg,
            )
        };
    }

    /// Attempts to initialize the steamworks api and returns
    /// a server to access the rest of the api.
    ///
    /// This should only ever have one instance per a program.
    ///
    /// Currently the steamworks api doesn't support IPv6.
    ///
    /// # Errors
    ///
    /// This can fail if:
    /// * The steam client isn't running
    /// * The app ID of the game couldn't be determined.
    ///
    ///   If the game isn't being run through steam this can be provided by
    ///   placing a `steam_appid.txt` with the ID inside in the current
    ///   working directory
    /// * The game isn't running on the same user/level as the steam client
    /// * The user doesn't own a license for the game.
    /// * The app ID isn't completely set up.
    pub fn init(
        ip: Ipv4Addr,
        game_port: u16,
        query_port: u16,
        server_mode: ServerMode,
        version: &str,
    ) -> Result<(Server, Client), SteamAPIInitError> {
        unsafe {
            let version = match try_c_string(version) {
                Some(version) => version,
                // The version is part of what registers a server, and a value that is not a C string
                // cannot be registered at all -- so this answers through the error the function already
                // returns instead of starting a server under a shortened version.
                None => {
                    return Err(SteamAPIInitError::Generic(
                        "the version string contains an interior nul".to_owned(),
                    ))
                }
            };

            // let internal_check_interface_versions =

            let raw_ip: u32 = ip.into();
            let server_mode = match server_mode {
                ServerMode::NoAuthentication => sys::EServerMode::eServerModeNoAuthentication,
                ServerMode::Authentication => sys::EServerMode::eServerModeAuthentication,
                ServerMode::AuthenticationAndSecure => {
                    sys::EServerMode::eServerModeAuthenticationAndSecure
                }
            };

            let mut err_msg: sys::SteamErrMsg = [0; 1024];
            let result = Self::steam_game_server_init_ex(
                raw_ip,
                game_port,
                query_port,
                server_mode,
                version.as_ptr(),
                &mut err_msg,
            );

            if result != sys::ESteamAPIInitResult::k_ESteamAPIInitResult_OK {
                return Err(SteamAPIInitError::from_result_and_message(result, err_msg));
            }

            sys::SteamAPI_ManualDispatch_Init();
            let server_raw = sys::SteamAPI_SteamGameServer_v015();
            let server = Arc::new(Inner {
                manager: Manager::Server,
                callbacks: Callbacks::new(),
                networking_sockets_data: Mutex::new(NetworkingSocketsData {
                    sockets: Default::default(),
                    independent_connections: Default::default(),
                    fake_udp_ports: Default::default(),
                    connection_callback: Default::default(),
                }),
            });
            Ok((
                Server {
                    inner: server.clone(),
                    server: server_raw,
                },
                Client { inner: server },
            ))
        }
    }

    /// Runs any currently pending callbacks
    ///
    /// This runs all currently pending callbacks on the current
    /// thread.
    ///
    /// This should be called frequently (e.g. once per a frame)
    /// in order to reduce the latency between receiving events.
    pub fn run_callbacks(&self) {
        self.inner.run_callbacks()
    }

    /// Runs any currently pending callbacks.
    ///
    /// This is identical to `run_callbacks` in every way, except that
    /// `callback_handler` is called for every callback invoked.
    ///
    /// This option provides an alternative for handling callbacks that
    /// don't require the handler to be `Send`, and `'static`.
    ///
    /// This should be called frequently (e.g. once per a frame)
    /// in order to reduce the latency between receiving events.
    pub fn process_callbacks(&self, mut callback_handler: impl FnMut(CallbackResult)) {
        self.inner.process_callbacks(&mut callback_handler)
    }

    /// Registers the passed function as a callback for the
    /// given type.
    ///
    /// The callback will be run on the thread that [`run_callbacks`]
    /// is called when the event arrives.
    ///
    /// If the callback handler cannot be made `Send` or `'static`
    /// the call to [`run_callbacks`] should be replaced with a call to
    /// [`process_callbacks`] instead.
    ///
    /// [`run_callbacks`]: Self::run_callbacks
    /// [`process_callbacks`]: Self::process_callbacks
    pub fn register_callback<C, F>(&self, f: F) -> CallbackHandle
    where
        C: Callback,
        F: FnMut(C) + 'static + Send,
    {
        unsafe { register_callback(&self.inner, f) }
    }

    /// Returns the steam id of the current server
    pub fn steam_id(&self) -> SteamId {
        unsafe { SteamId(sys::SteamAPI_ISteamGameServer_GetSteamID(self.server)) }
    }
}
