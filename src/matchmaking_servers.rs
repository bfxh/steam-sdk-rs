use std::net::Ipv4Addr;
use std::ptr;
use std::time::Duration;

use super::*;

/// The query's round-trip time, read out of a field the SDK types as a plain `int`.
///
/// The header documents the value as "current ping time in milliseconds"
/// (`matchmakingtypes.h:177`) and its constructor seeds it with
/// "m_nPing = m_nAppID = m_nPlayers = m_nMaxPlayers = m_nBotPlayers = m_ulTimeLastPlayed = m_nServerVersion = 0"
/// (`matchmakingtypes.h:211`); neither says anything about the sign. The field on
/// `GameServerItem` is a `Duration`, which cannot hold a negative, and the caller here is walking a
/// server list -- so a negative answer is read as no measured delay instead of aborting the caller's
/// frame mid-list. That is this crate's own choice about an undocumented sign, not a claim about what
/// Steam means by it. The networking layer *does* have named negative answers for its own ping field
/// -- `PingEstimate` keeps `-1` and `-2` as `Failed` and `Unknown`, because `steamnetworkingtypes.h:1089-1090`
/// declares them -- and nothing here borrows those constants: a master-server query answer and a route
/// estimate are two different fields, and treating one's sentinels as the other's would be a claim about
/// Steam that no header supports.
fn ping_duration(raw: std::os::raw::c_int) -> Duration {
    Duration::from_millis(u64::try_from(raw).unwrap_or(0))
}

#[cfg(test)]
mod ping_tests {
    //! The pure half of reading `gameserveritem_t`: the ping field. Filling the struct needs a query
    //! against a running master server; deciding what one `int` means needs nothing, so the decision
    //! lives in `ping_duration` and is judged here rather than registered as uncovered.
    use super::ping_duration;
    use std::time::Duration;

    #[test]
    fn a_plain_count_of_milliseconds_becomes_that_duration() {
        assert_eq!(ping_duration(0), Duration::ZERO);
        assert_eq!(ping_duration(1), Duration::from_millis(1));
        assert_eq!(ping_duration(37_000), Duration::from_millis(37_000));
        assert_eq!(
            ping_duration(i32::MAX),
            Duration::from_millis(i32::MAX as u64)
        );
    }

    /// The case the old `raw.m_nPing.try_into().unwrap()` aborted on: an `i32` that is not a valid
    /// `u64` has no `Duration` to become, and a server list must not die for it.
    #[test]
    fn a_negative_ping_reads_as_no_measured_delay() {
        assert_eq!(ping_duration(-1), Duration::ZERO);
        assert_eq!(ping_duration(i32::MIN), Duration::ZERO);
    }

    /// Anti-vacuity: the fallible conversion the function is built on really can fail, which is what
    /// makes the arm above a live branch rather than defensive decoration.
    #[test]
    fn the_conversion_being_made_total_really_is_fallible() {
        assert!(u64::try_from(-1).is_err());
        assert!(u64::try_from(0).is_ok());
    }
}

macro_rules! matchmaking_servers_callback {
    (
        $name:ident;
        $self:ident;
        ($($additional_name:ident : $additional_type:ty where $additional_content:block),*);
        $(
            $fn_name:ident($clear_after_call:tt): ( $( $fn_arg_name:ident: $cpp_fn_arg:ty => $rust_fn_arg:ty where $normalize:tt ),* )
        ),*
    ) => {
        pastey::item! {
            $(
                #[allow(unused_variables)]
                extern "C" fn [<$name:lower _ $fn_name _virtual>]($self: *mut [<$name CallbacksReal>] $(, $fn_arg_name: $cpp_fn_arg)*) {
                    unsafe {
                        $(
                            #[allow(unused_parens)]
                            let [<$fn_arg_name _norm>]: $rust_fn_arg = $normalize;
                        )*
                        // In case of dropping rust_callbacks inside $fn_name
                        let rc_fn = Arc::clone(&(*(*$self).rust_callbacks).$fn_name);
                        (*rc_fn)($([<$fn_arg_name _norm>]),*);
                        $clear_after_call;
                    }
                }
            )*

            pub struct [<$name Callbacks>] {
                $(
                    pub (self) $fn_name: Arc<dyn Fn($($rust_fn_arg),*) + Send + Sync>,
                )*
                $(
                    pub (self) $additional_name: $additional_type,
                )*
            }

            impl [<$name Callbacks>] {
                pub fn new($($fn_name: Box<dyn Fn($($rust_fn_arg),*) + Send + Sync>),*) -> Self {
                    Self {
                        $($fn_name: Arc::new($fn_name),)*
                        $($additional_name: $additional_content,)*
                    }
                }
            }

            #[repr(C)]
            struct [<$name CallbacksReal>] {
                pub vtable: *mut [<$name CallbacksVirtual>],
                pub rust_callbacks: *mut [<$name Callbacks>],
            }

            #[repr(C)]
            struct [<$name CallbacksVirtual>] {
                $(
                    pub $fn_name: extern "C" fn(*mut [<$name CallbacksReal>] $(, $cpp_fn_arg)*)
                ),*
            }

            unsafe fn [<create_ $name:lower>](rust_callbacks: [<$name Callbacks>]) -> *mut [<$name CallbacksReal>] {
                let rust_callbacks = Box::into_raw(Box::new(rust_callbacks));
                let vtable = Box::into_raw(Box::new([<$name CallbacksVirtual>] {
                    $(
                        $fn_name: [<$name:lower _ $fn_name _virtual>]
                    ),*
                }));
                let real = Box::into_raw(Box::new([<$name CallbacksReal>] {
                    vtable,
                    rust_callbacks,
                }));

                real
            }

            unsafe fn [<free_ $name:lower>](real: *mut [<$name CallbacksReal>]) {
                drop(Box::from_raw((*real).rust_callbacks));
                drop(Box::from_raw((*real).vtable));
                drop(Box::from_raw(real));
            }
        }
    };
}

/// The filter-pair length rule, apart from the map walk that applies it.
///
/// Steam's pair of fields is fixed-width -- `char m_szKey[ 256 ];` and `char m_szValue[ 256 ];`
/// (`matchmakingtypes.h:40-41`) -- and its own constructor fills them with
/// "strncpy( m_szKey, pchKey, sizeof(m_szKey) )" (`:35`) and then writes a terminator into the last
/// byte (`:36`), which is to say Steam truncates an over-long filter without saying so. This wrapper
/// refuses instead, so the rule lives here where the boundary can be judged: the bound is `< 256`
/// because the 256th byte has to be the terminator.
fn filter_pair_fits(key: &[u8], value: &[u8]) -> bool {
    key.len() < 256 && value.len() < 256
}

macro_rules! gen_server_list_fn {
    (
        $name:ident, $sys_method:ident
    ) => {
        /// # Usage
        ///
        /// Request must be released at the end of using. For more details see [`ServerListRequest::release`]
        ///
        /// # Arguments
        ///
        /// * app_id: The app to request the server list of.
        /// * filters: An array of filters to only retrieve servers the user cares about.
        /// A list of the keys & values can be found
        /// [here](https://partner.steamgames.com/doc/api/ISteamMatchmakingServers#MatchMakingKeyValuePair_t).
        ///
        /// # Errors
        ///
        /// Every filter's key and value must take 255 bytes or under, otherwise `Err` is returned.
        pub fn $name<ID: Into<AppId>>(
            &self,
            app_id: ID,
            filters: &HashMap<&str, &str>,
            callbacks: ServerListCallbacks,
        ) -> SteamResult<Arc<Mutex<ServerListRequest>>> {
            let app_id = app_id.into().0;
            let mut filters = {
                let mut vec = Vec::with_capacity(filters.len());
                for i in filters {
                    let key_bytes = i.0.as_bytes();
                    let value_bytes = i.1.as_bytes();

                    if !filter_pair_fits(key_bytes, value_bytes) {
                        return Err(SteamError::InvalidParameter);
                    }

                    let mut key = [0 as c_char; 256];
                    let mut value = [0 as c_char; 256];

                    unsafe {
                        key.as_mut_ptr()
                            .copy_from(key_bytes.as_ptr().cast(), key_bytes.len());
                        value
                            .as_mut_ptr()
                            .copy_from(value_bytes.as_ptr().cast(), value_bytes.len());
                    }

                    vec.push(sys::MatchMakingKeyValuePair_t {
                        m_szKey: key,
                        m_szValue: value,
                    });
                }
                vec.shrink_to_fit();

                vec
            };

            unsafe {
                let callbacks = create_serverlist(callbacks);

                let request_arc = ServerListRequest::get(callbacks);
                let mut request = crate::utils::mutex_guard(&request_arc);

                let handle = sys::$sys_method(
                    self.mms,
                    app_id,
                    &mut filters.as_mut_ptr().cast(),
                    filters.len().try_into().unwrap(),
                    callbacks.cast(),
                );

                request.mms = self.mms;
                request.real = callbacks;
                request.h_req = handle;
                request.inner = Some(self._inner.clone());

                drop(request);

                Ok(request_arc)
            }
        }
    };
}

pub struct GameServerItem {
    pub appid: AppId,
    pub players: i32,
    pub do_not_refresh: bool,
    pub successful_response: bool,
    pub have_password: bool,
    pub secure: bool,
    pub bot_players: i32,
    pub ping: Duration,
    pub max_players: i32,
    pub server_version: i32,
    pub steamid: SteamId,
    pub last_time_played: Duration,
    pub addr: Ipv4Addr,
    pub query_port: u16,
    pub connection_port: u16,
    pub game_description: String,
    pub server_name: String,
    pub game_dir: String,
    pub map: String,
    pub tags: String,
}

impl GameServerItem {
    unsafe fn from_ptr(raw: *const sys::gameserveritem_t) -> Self {
        let raw = *raw;
        Self {
            appid: AppId(raw.m_nAppID),
            players: raw.m_nPlayers,
            bot_players: raw.m_nBotPlayers,
            ping: ping_duration(raw.m_nPing),
            max_players: raw.m_nMaxPlayers,
            server_version: raw.m_nServerVersion,
            steamid: SteamId(raw.m_steamID.m_steamid.m_unAll64Bits),

            do_not_refresh: raw.m_bDoNotRefresh,
            successful_response: raw.m_bHadSuccessfulResponse,
            have_password: raw.m_bPassword,
            secure: raw.m_bSecure,

            addr: Ipv4Addr::from(raw.m_NetAdr.m_unIP),
            query_port: raw.m_NetAdr.m_usQueryPort,
            connection_port: raw.m_NetAdr.m_usConnectionPort,

            game_description: CStr::from_ptr(raw.m_szGameDescription.as_ptr())
                .to_string_lossy()
                .into_owned(),
            server_name: CStr::from_ptr(raw.m_szServerName.as_ptr())
                .to_string_lossy()
                .into_owned(),
            game_dir: CStr::from_ptr(raw.m_szGameDir.as_ptr())
                .to_string_lossy()
                .into_owned(),
            map: CStr::from_ptr(raw.m_szMap.as_ptr())
                .to_string_lossy()
                .into_owned(),
            tags: CStr::from_ptr(raw.m_szGameTags.as_ptr())
                .to_string_lossy()
                .into_owned(),

            last_time_played: Duration::from_secs(raw.m_ulTimeLastPlayed.into()),
        }
    }
}

matchmaking_servers_callback!(
    Ping;
    _self;
    ();
    responded({}): (info: *const sys::gameserveritem_t => GameServerItem where { GameServerItem::from_ptr(info) }),
    failed({ free_ping(_self) }): ()
);

matchmaking_servers_callback!(
    PlayerDetails;
    _self;
    ();
    add_player({}): (
        name: *const std::os::raw::c_char => &CStr where { CStr::from_ptr(name) },
        score: i32 => i32 where {score},
        time_played: f32 => f32 where {time_played}
    ),
    failed({ free_playerdetails(_self) }): (),
    refresh_complete({ free_playerdetails(_self) }): ()
);

matchmaking_servers_callback!(
    ServerRules;
    _self;
    ();
    add_rule({}): (
        rule: *const std::os::raw::c_char => &CStr where { CStr::from_ptr(rule) },
        value: *const std::os::raw::c_char => &CStr where { CStr::from_ptr(value) }
    ),
    failed({ free_serverrules(_self) }): (),
    refresh_complete({ free_serverrules(_self) }): ()
);

matchmaking_servers_callback!(
    ServerList;
    _self;
    (
        req: Arc<Mutex<ServerListRequest>> where {
            Arc::new(Mutex::new(ServerListRequest {
                h_req: ptr::null_mut(),
                released: false,
                mms: ptr::null_mut(),
                real: ptr::null_mut(),
                inner: None,
            }))
        }
    );
    responded({}): (
        request: sys::HServerListRequest => Arc<Mutex<ServerListRequest>> where { ServerListRequest::get(_self) },
        server: i32 => i32 where {server}
    ),
    failed({}): (
        request: sys::HServerListRequest => Arc<Mutex<ServerListRequest>> where { ServerListRequest::get(_self) },
        server: i32 => i32 where {server}
    ),
    refresh_complete({}): (
        request: sys::HServerListRequest => Arc<Mutex<ServerListRequest>> where { ServerListRequest::get(_self) },
        response: ServerResponse => ServerResponse where {response}
    )
);

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum ServerResponse {
    ServerResponded = 0,
    ServerFailedToRespond = 1,
    NoServersListedOnMasterServer = 2,
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum ReleaseError {
    /// Further using methods on this request after `release`
    /// called will always result in `Err(ReleseError::Released)`.
    Released,
    /// Due to wrapper limitations releasing request while query
    /// is still refreshing (`is_refreshing()`) is impossible.
    /// `Err(ReleaseError::Refreshing)` will be returned.
    Refreshing,
}

pub struct ServerListRequest {
    pub(self) h_req: sys::HServerListRequest,
    pub(self) released: bool,
    pub(self) mms: *mut sys::ISteamMatchmakingServers,
    pub(self) real: *mut ServerListCallbacksReal,
    // Filled by the request method that issues the query, because the callback object -- and with it
    // this request -- is built by the caller before any Steam interface is in hand. `Option` is that
    // ordering, not a maybe: once a request exists it is `Some`, and it is what keeps the client
    // alive while `release()` and the callbacks still call through `mms`.
    pub(self) inner: Option<Arc<Inner>>,
}

/// A request is reached from two threads by Steam's own design, and the ownership graph it sits in
/// is therefore refcounted atomically -- `Arc<dyn Fn(..) + Send>`, never `Rc`.
///
/// The thread that pumps callbacks runs the generated C-ABI trampolines, which clone a
/// callback to survive the callee dropping it (`Arc::clone` in the macro above); the thread that
/// owns the `ServerListRequest` releases it, and releasing is what drops the closure that clone was
/// taken from. With a non-atomic counter those two are unsynchronised mutation of one value -- the
/// kind of defect `unsafe impl Send` papers over rather than fixes. Steam itself warns about the
/// window this describes: cancelling a query before destructing a callback object avoids
/// "crashing when callbacks occur" (`isteammatchmaking.h:514-515`).
// SAFETY: a use site in this crate needs `Send` for this type -- measured by deleting
// this line, which makes `cargo check --all-targets` fail with
// rustc reports it as not sendable, naming *mut ISteamMatchmakingServers and the callback object it points at.
// The type holds a raw Steam interface pointer, plain-copy handles, and the owning `Arc<Inner>`;
// it carries no non-atomic reference count, which is the one hazard a `Send` claim here cannot
// see -- `scripts/ci/callback_threads.py` counts `Rc` tokens beside every promise and ships at
// zero for exactly that reason. Moving the value therefore moves sole ownership of the pointer;
// nothing reads the same state through two threads because of the move itself.
unsafe impl Send for ServerListRequest {}

impl ServerListRequest {
    pub(self) unsafe fn get(_self: *mut ServerListCallbacksReal) -> Arc<Mutex<Self>> {
        let rust_callbacks = &*(*_self).rust_callbacks;
        Arc::clone(&rust_callbacks.req)
    }

    /// # Usage
    ///
    /// Cancels any pending query on it if there's a pending
    /// query in progress. Releasing all heap allocated
    /// structures used for callbacks. The `refresh_complete`
    /// callback will not be posted when request is released.
    ///
    /// Further using methods on this request after `release`
    /// called will always result in `Err(ReleseError::Released)`.
    ///
    /// Due to wrapper limitations releasing request while query
    /// is still refreshing (`is_refreshing()`) is impossible.
    /// `Err(ReleaseError::Refreshing)` will be returned.
    pub fn release(&mut self) -> Result<(), ReleaseError> {
        unsafe {
            if self.released {
                return Err(ReleaseError::Released);
            }
            if sys::SteamAPI_ISteamMatchmakingServers_IsRefreshing(self.mms, self.h_req) {
                return Err(ReleaseError::Refreshing);
            }

            self.released = true;
            sys::SteamAPI_ISteamMatchmakingServers_ReleaseRequest(self.mms, self.h_req);

            free_serverlist(self.real);

            Ok(())
        }
    }

    fn released(&self) -> SteamResult<()> {
        if self.released {
            Err(SteamError::InvalidState)
        } else {
            Ok(())
        }
    }

    /// # Errors
    ///
    /// Err if called on the released request
    pub fn get_server_count(&self) -> SteamResult<i32> {
        unsafe {
            self.released()?;

            Ok(sys::SteamAPI_ISteamMatchmakingServers_GetServerCount(
                self.mms, self.h_req,
            ))
        }
    }

    /// # Errors
    ///
    /// Err if called on the released request
    pub fn get_server_details(&self, server: i32) -> SteamResult<GameServerItem> {
        unsafe {
            self.released()?;

            // Should we then free this pointer?
            let server_item = sys::SteamAPI_ISteamMatchmakingServers_GetServerDetails(
                self.mms, self.h_req, server,
            );

            Ok(GameServerItem::from_ptr(server_item))
        }
    }

    /// # Errors
    ///
    /// Err if called on the released request
    pub fn refresh_query(&self) -> SteamResult<()> {
        unsafe {
            self.released()?;

            sys::SteamAPI_ISteamMatchmakingServers_RefreshQuery(self.mms, self.h_req);

            Ok(())
        }
    }

    /// Stops the query behind this request handle without releasing it.
    ///
    /// Steam's own reason for the call is on the declaration: it is what you should do "before
    /// destructing a callback object that may have been passed" to one of the list requests, because
    /// "Not doing so may result in a crash when a callback" arrives on the object that is already gone
    /// (`isteammatchmaking.h:476-477`).
    ///
    /// It is *not* a release. The same block says "Canceling a query does not release the allocated
    /// request handle." and that handle "must be released using ReleaseRequest( hRequest )"
    /// (`:479-480`), so [`ServerListRequest::release`] still has to run afterwards, and this order is
    /// the one the header asks for: cancel first, then release, so no callback can reach a destructed
    /// object in between.
    ///
    /// # Errors
    ///
    /// Err if called on the released request -- after `release` there is no handle left to cancel, and
    /// Steam would be handed a request id it has already forgotten.
    pub fn cancel_query(&self) -> SteamResult<()> {
        unsafe {
            self.released()?;

            sys::SteamAPI_ISteamMatchmakingServers_CancelQuery(self.mms, self.h_req);

            Ok(())
        }
    }

    /// # Errors
    ///
    /// Err if called on the released request
    pub fn refresh_server(&self, server: i32) -> SteamResult<()> {
        unsafe {
            self.released()?;

            sys::SteamAPI_ISteamMatchmakingServers_RefreshServer(self.mms, self.h_req, server);

            Ok(())
        }
    }

    /// # Errors
    ///
    /// Err if called on the released request
    pub fn is_refreshing(&self) -> SteamResult<bool> {
        unsafe {
            self.released()?;

            Ok(sys::SteamAPI_ISteamMatchmakingServers_IsRefreshing(
                self.mms, self.h_req,
            ))
        }
    }
}

/// Access to the steam MatchmakingServers interface
pub struct MatchmakingServers {
    pub(crate) mms: *mut sys::ISteamMatchmakingServers,
    pub(crate) _inner: Arc<Inner>,
}

/// A query to one server by address, which Steam answers through the callbacks given to it.
///
/// This is held as a handle instead of being forgotten on the floor because the callback objects
/// outlive the call: Steam's instruction is to cancel "before destructing a callback object that may
/// have been passed" to the query, "to avoid crashing when callbacks occur"
/// (`isteammatchmaking.h:514-515`).
pub struct ServerQuery {
    q: sys::HServerQuery,
    cancelled: bool,
    mms: *mut sys::ISteamMatchmakingServers,
    _inner: Arc<Inner>,
}

/// Steam's own no-query marker (`HSERVERQUERY_INVALID`, which the generated layer types as `c_int`),
/// refused at construction instead of handed out as something to cancel: `CancelServerQuery`
/// aimed at that id would be aimed at whatever query Steam has since given the number to.
fn query_id(raw: sys::HServerQuery) -> SteamResult<sys::HServerQuery> {
    if raw == sys::HSERVERQUERY_INVALID {
        return Err(SteamError::InvalidParameter);
    }
    Ok(raw)
}

/// Whether this handle still owns a query. The rule is the one `ServerListRequest` already enforces
/// for `release`: the second call is a caller bug, and this wrapper's own state is what it failed on.
fn cancel_guard(cancelled: bool) -> SteamResult<()> {
    if cancelled {
        return Err(SteamError::InvalidState);
    }
    Ok(())
}

impl ServerQuery {
    unsafe fn new(
        raw: sys::HServerQuery,
        mms: *mut sys::ISteamMatchmakingServers,
        inner: &Arc<Inner>,
    ) -> SteamResult<Self> {
        Ok(Self {
            q: query_id(raw)?,
            cancelled: false,
            mms,
            _inner: Arc::clone(inner),
        })
    }

    /// Stops the query, so no callback reaches the objects that were handed to it.
    ///
    /// Cancelling is not optional hygiene at the end of a game's life: it is the difference between
    /// Steam writing into memory this crate still owns and writing into memory it no longer does.
    ///
    /// # Errors
    ///
    /// `InvalidState` if this handle has already been cancelled.
    pub fn cancel(&mut self) -> SteamResult<()> {
        cancel_guard(self.cancelled)?;
        unsafe {
            sys::SteamAPI_ISteamMatchmakingServers_CancelServerQuery(self.mms, self.q);
        }
        self.cancelled = true;
        Ok(())
    }
}

impl MatchmakingServers {
    /// Asks one server for its ping and details, and hands back the handle to cancel it with.
    ///
    /// Steam's line for the call is "Request updated ping time and other details from a single server"
    /// (`isteammatchmaking.h:504`). The handle used to be dropped on the floor here, which left
    /// [`ServerQuery::cancel`] unwrappable: Steam's own instruction for the callback object is to cancel
    /// "before destructing" it, "to avoid crashing when callbacks occur" (`:513-515`).
    pub fn ping_server(
        &self,
        ip: std::net::Ipv4Addr,
        port: u16,
        callbacks: PingCallbacks,
    ) -> SteamResult<ServerQuery> {
        unsafe {
            let callbacks = create_ping(callbacks);

            let raw = sys::SteamAPI_ISteamMatchmakingServers_PingServer(
                self.mms,
                ip.into(),
                port,
                callbacks.cast(),
            );
            ServerQuery::new(raw, self.mms, &self._inner)
        }
    }

    /// Asks one server who is on it, and hands back the handle to cancel the query with.
    ///
    /// "Request the list of players currently playing on a server" (`isteammatchmaking.h:507`); see
    /// [`MatchmakingServers::ping_server`] for why the handle is returned rather than discarded.
    pub fn player_details(
        &self,
        ip: std::net::Ipv4Addr,
        port: u16,
        callbacks: PlayerDetailsCallbacks,
    ) -> SteamResult<ServerQuery> {
        unsafe {
            let callbacks = create_playerdetails(callbacks);

            let raw = sys::SteamAPI_ISteamMatchmakingServers_PlayerDetails(
                self.mms,
                ip.into(),
                port,
                callbacks.cast(),
            );
            ServerQuery::new(raw, self.mms, &self._inner)
        }
    }

    /// Asks one server which rules it runs, and hands back the handle to cancel the query with.
    ///
    /// Steam's line is "Request the list of rules that the server is running"
    /// (`isteammatchmaking.h:510`, where the rest of the sentence points at
    /// `ISteamGameServer::SetKeyValue()` as the way those values get set); see
    /// [`MatchmakingServers::ping_server`] for the handle's purpose.
    pub fn server_rules(
        &self,
        ip: std::net::Ipv4Addr,
        port: u16,
        callbacks: ServerRulesCallbacks,
    ) -> SteamResult<ServerQuery> {
        unsafe {
            let callbacks = create_serverrules(callbacks);

            let raw = sys::SteamAPI_ISteamMatchmakingServers_ServerRules(
                self.mms,
                ip.into(),
                port,
                callbacks.cast(),
            );
            ServerQuery::new(raw, self.mms, &self._inner)
        }
    }

    /// # Usage
    ///
    /// Request must be released at the end of using. For more details see [`ServerListRequest::release`]
    ///
    /// # Arguments
    ///
    /// * app_id: The app to request the server list of.
    pub fn lan_server_list<ID: Into<AppId>>(
        &self,
        app_id: ID,
        callbacks: ServerListCallbacks,
    ) -> Arc<Mutex<ServerListRequest>> {
        unsafe {
            let app_id = app_id.into().0;

            let callbacks = create_serverlist(callbacks);

            let request_arc = ServerListRequest::get(callbacks);
            let mut request = crate::utils::mutex_guard(&request_arc);

            let handle = sys::SteamAPI_ISteamMatchmakingServers_RequestLANServerList(
                self.mms,
                app_id,
                callbacks.cast(),
            );

            request.mms = self.mms;
            request.real = callbacks;
            request.h_req = handle;

            drop(request);

            request_arc
        }
    }

    gen_server_list_fn!(
        internet_server_list,
        SteamAPI_ISteamMatchmakingServers_RequestInternetServerList
    );
    gen_server_list_fn!(
        favorites_server_list,
        SteamAPI_ISteamMatchmakingServers_RequestFavoritesServerList
    );
    gen_server_list_fn!(
        history_server_list,
        SteamAPI_ISteamMatchmakingServers_RequestHistoryServerList
    );
    gen_server_list_fn!(
        friends_server_list,
        SteamAPI_ISteamMatchmakingServers_RequestFriendsServerList
    );
    // The sixth row of the list Steam introduces as "Request a new list of servers of a particular
    // type." (`isteammatchmaking.h:386`): like the five beside it it "allocates a new asynchronous
    // request object" (`:387`) which "must be released by calling ReleaseRequest( hServerListRequest )"
    // (`:388`). The header names no filter-free variant here, so this one takes the same map the others
    // do, and the file says nothing more about what a spectator list contains.
    gen_server_list_fn!(
        spectator_server_list,
        SteamAPI_ISteamMatchmakingServers_RequestSpectatorServerList
    );
}

#[test]
#[serial_test::serial]
#[ignore = "requires a running Steam client"]
fn test_internet_servers() {
    let client = Client::init_app(304930).unwrap();

    let data = Arc::new(Mutex::new(0));
    let data2 = Arc::clone(&data);
    let data3 = Arc::clone(&data);
    let callbacks = ServerListCallbacks::new(
        Box::new(move |list, server| {
            let details = list.lock().unwrap().get_server_details(server).unwrap();
            println!("{} : {}", details.server_name, details.map);
            *data.lock().unwrap() += 1;
        }),
        Box::new(move |_list, _server| {
            *data2.lock().unwrap() += 1;
        }),
        Box::new(move |list, _response| {
            list.lock().unwrap().release().unwrap();
            println!("{}", data3.lock().unwrap());
        }),
    );

    let mut map = HashMap::new();
    map.insert("map", "PEI");
    let _ = client
        .matchmaking_servers()
        .internet_server_list(304930, &map, callbacks)
        .unwrap();

    for _ in 0..2000 {
        client.run_callbacks();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(test)]
mod query_tests {
    //! Which returned handle is a query, and which state may still be cancelled. The FFI calls
    //! themselves need a running client and are registered as unverified.

    use super::{cancel_guard, query_id, SteamError};
    use steamworks_sys as sys;

    /// Steam's own "no query" marker is refused; every other number is a query Steam said it started.
    /// The mutation this exists for is the comparison dropping out, which would hand back a handle
    /// whose `cancel` points at an id Steam never gave this caller.
    #[test]
    fn the_no_query_marker_is_not_turned_into_a_cancellable_handle() {
        assert_eq!(
            query_id(sys::HSERVERQUERY_INVALID),
            Err(SteamError::InvalidParameter)
        );
        assert_eq!(query_id(0), Ok(0));
        assert_eq!(query_id(1), Ok(1));
        assert_eq!(query_id(i32::MAX), Ok(i32::MAX));
        assert_eq!(query_id(-2), Ok(-2));
    }

    /// A premise pin: the header writes `0xffffffff` into a `const int`, so the marker is `-1` once the
    /// generated layer's `c_int` has taken it. If a regeneration ever moves that number, this goes red
    /// and `query_id` has to be re-read rather than trusted.
    #[test]
    fn steam_s_no_query_marker_is_the_number_the_header_says_it_is() {
        assert_eq!(
            sys::HSERVERQUERY_INVALID,
            -1,
            "the header's `0xffffffff` stored as `int`"
        );
    }

    /// Cancelling twice fails on the second one, and the failure names the reason: this wrapper's own
    /// state, not something Steam declined.
    #[test]
    fn a_second_cancel_is_refused_as_a_wrong_state_not_a_wrong_argument() {
        assert_eq!(cancel_guard(false), Ok(()));
        assert_eq!(cancel_guard(true), Err(SteamError::InvalidState));
    }
}

#[cfg(test)]
mod filter_tests {
    //! The length rule on its own, plus the layout it counts against. Which servers a spectator list
    //! actually returns is registered as unverified: every request here needs a live client.

    use super::{c_char, filter_pair_fits};
    use steamworks_sys as sys;

    /// The bound is the byte reserved for the terminator, so 255 is the last length that still fits
    /// and 256 is the first that must be refused. A mutation moving either side lands here: `< 255`
    /// refuses a filter Steam can hold, `<= 256` copies a filter whose terminator has nowhere to go.
    #[test]
    fn a_filter_is_refused_when_it_cannot_keep_its_terminator() {
        assert!(filter_pair_fits(&[], &[]));
        assert!(filter_pair_fits(&[b'k'; 255], &[b'v'; 255]));
        assert!(!filter_pair_fits(&[b'k'; 256], &[b'v'; 0]));
        assert!(!filter_pair_fits(&[b'k'; 0], &[b'v'; 256]));
        assert!(!filter_pair_fits(&[b'k'; 256], &[b'v'; 256]));
        // One long half is enough to refuse the pair, because both are copied into the same call.
        assert!(!filter_pair_fits(&[b'k'; 300], &[b'v'; 10]));
        assert!(!filter_pair_fits(&[b'k'; 10], &[b'v'; 300]));
    }

    /// A premise pin, not a lock on this crate's code: the bound above counts a field Steam declares,
    /// and if a regeneration ever changes that layout the pin has to be re-read rather than trusted.
    /// Reddened by shrinking one field in the generated layer -- no change of mine can move it.
    #[test]
    fn the_pair_is_exactly_the_two_arrays_the_bound_counts() {
        assert_eq!(
            std::mem::size_of::<sys::MatchMakingKeyValuePair_t>(),
            256 * 2 * std::mem::size_of::<c_char>(),
            "the filter pair is two 256-byte fields and nothing else"
        );
    }
}

#[cfg(test)]
mod thread_claims {
    //! Each `unsafe impl Send` here is an assertion, not a structure: a raw pointer is `Send` only
    //! because the code says so. So the claim is checked at compile time -- if the callback objects
    //! ever go back to a non-atomic refcount, or drop the `Send + Sync` the closures now carry,
    //! `assert_send` stops compiling and this module is what says so out loud.

    use super::{
        PingCallbacks, PlayerDetailsCallbacks, ServerListCallbacks, ServerListRequest,
        ServerRulesCallbacks,
    };

    fn assert_send<T: Send>() {}

    #[test]
    fn every_object_the_send_claim_covers_is_actually_send() {
        assert_send::<PingCallbacks>();
        assert_send::<PlayerDetailsCallbacks>();
        assert_send::<ServerRulesCallbacks>();
        assert_send::<ServerListCallbacks>();
        assert_send::<ServerListRequest>();
    }
}
