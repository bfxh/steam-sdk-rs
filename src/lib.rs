#![doc = include_str!("../README.md")]

#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate bitflags;

use screenshots::Screenshots;
#[cfg(feature = "raw-bindings")]
pub use steamworks_sys as sys;
#[cfg(not(feature = "raw-bindings"))]
use steamworks_sys as sys;
use sys::{EServerMode, ESteamAPIInitResult, SteamErrMsg};

use core::ffi::c_void;
use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::ffi::{c_char, CStr, CString};
use std::fmt::{self, Debug, Formatter};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, Weak};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub use crate::app::*;
pub use crate::callback::*;
#[cfg(feature = "futures")]
pub use crate::callback_stream::*;
pub use crate::error::*;
pub use crate::friends::*;
pub use crate::html_surface::*;
pub use crate::http::*;
pub use crate::identity::*;
pub use crate::input::*;
pub use crate::inventory::*;
pub use crate::matchmaking::*;
pub use crate::matchmaking_servers::*;
pub use crate::music::*;
pub use crate::networking::*;
pub use crate::parental::*;
pub use crate::parties::*;
pub use crate::remote_play::*;
pub use crate::remote_storage::*;
pub use crate::safe_ugc::*;
pub use crate::server::*;
pub use crate::server_stats::*;
pub use crate::timeline::*;
pub use crate::tri::*;
pub use crate::ugc::*;
pub use crate::user::*;
pub use crate::user_stats::*;
pub use crate::utils::*;
pub use crate::video::*;

#[macro_use]
mod callback;
mod app;
#[cfg(feature = "futures")]
mod callback_stream;
mod error;
mod friends;
mod html_surface;
mod http;
mod identity;
mod input;
mod inventory;
#[cfg(test)]
mod lib_tests;
mod matchmaking;
mod matchmaking_servers;
mod music;
mod networking;
pub mod networking_messages;
pub mod networking_sockets;
mod networking_sockets_callback;
pub mod networking_types;
pub mod networking_utils;
mod parental;
mod parties;
mod remote_play;
mod remote_storage;
mod safe_ugc;
pub mod screenshots;
mod server;
mod server_stats;
pub mod timeline;
mod tri;
mod ugc;
mod user;
mod user_stats;
mod utils;
mod video;

/// Alias for [`Result<T, SteamError>`](SteamError)
pub type SteamResult<T = ()> = Result<T, SteamError>;

/// Convert a raw result to a Rust [`Result`] type.
pub(crate) fn to_steam_result(result: sys::EResult) -> SteamResult {
    match result {
        // sys::EResult::k_EResultNone => Ok(()), TODO: I'm not sure if this is considered a success
        sys::EResult::k_EResultOK => Ok(()),
        error => Err(error.try_into().unwrap()),
    }
}

// A note about thread-safety:
// The steam api is assumed to be thread safe unless
// the documentation for a method states otherwise,
// however this is never stated anywhere in the docs
// that I could see.

/// The main entry point into the steam client.
///
/// This provides access to all of the steamworks api that
/// clients can use.
pub struct Client {
    inner: Arc<Inner>,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Client {
            inner: self.inner.clone(),
        }
    }
}

struct Inner {
    manager: Manager,
    callbacks: Callbacks,
    networking_sockets_data: Mutex<NetworkingSocketsData>,
}

struct Callbacks {
    callbacks: Mutex<HashMap<i32, Box<dyn FnMut(*mut c_void) + Send + 'static>>>,
    call_results:
        Mutex<HashMap<sys::SteamAPICall_t, Box<dyn FnOnce(*mut c_void, bool) + Send + 'static>>>,
    /// Registrations and removals that arrived while this thread was inside a
    /// dispatch loop, and therefore could not take `callbacks` without
    /// self-deadlock. Drained by [`Callbacks::drain`] once the lock is free.
    deferred: Mutex<VecDeque<Box<dyn FnOnce(&Callbacks) + Send + 'static>>>,
}

impl Callbacks {
    fn new() -> Self {
        Callbacks {
            callbacks: Mutex::new(HashMap::new()),
            call_results: Mutex::new(HashMap::new()),
            deferred: Mutex::new(VecDeque::new()),
        }
    }

    fn set_handler(&self, id: i32, handler: Box<dyn FnMut(*mut c_void) + Send + 'static>) {
        self.callbacks.lock().unwrap().insert(id, handler);
    }

    fn remove_handler(&self, id: i32) {
        // A poisoned lock here is reported rather than panicked on, because
        // this runs from `CallbackHandle::drop`: panicking inside a Drop would
        // turn a recoverable registry problem into an abort during unwinding.
        match self.callbacks.lock() {
            Ok(mut cb) => {
                cb.remove(&id);
            }
            Err(err) => {
                eprintln!("error while dropping callback: {:?}", err);
            }
        }
    }

    /// Run `op` against this registry, or queue it if `owner` is the context
    /// this thread is currently dispatching callbacks for.
    ///
    /// Dispatch holds the `callbacks` lock while a user callback runs, and
    /// `std::sync::Mutex` is not reentrant: a callback that registers or drops
    /// a callback would otherwise block forever on a lock it already owns.
    /// Deferring the mutation to the dispatch loop's drain point keeps the
    /// ordering (queue is FIFO) without ever re-entering the lock.
    ///
    /// Only the *same* context is deferred. A second context (client plus
    /// dedicated game server in one process) has its own lock, so registering
    /// there must apply immediately rather than wait for a dispatch that may
    /// never come.
    fn run_or_defer(&self, owner: *const (), op: impl FnOnce(&Callbacks) + Send + 'static) {
        if DISPATCHING.with(|flag| flag.get()) == owner {
            self.deferred.lock().unwrap().push_back(Box::new(op));
        } else {
            op(self);
        }
    }

    /// Apply everything queued during the current dispatch pass.
    fn drain(&self) {
        loop {
            let op = self.deferred.lock().unwrap().pop_front();
            match op {
                Some(op) => op(self),
                None => break,
            }
        }
    }
}

thread_local! {
    /// The dispatch-context identity of the `Inner` this thread is inside of,
    /// or null when it is not inside a dispatch loop. Stored type-erased and
    /// only ever compared, never dereferenced.
    static DISPATCHING: Cell<*const ()> = const { Cell::new(std::ptr::null()) };
}

/// Marks the current thread as dispatching for `owner` until dropped.
///
/// `try_new` refuses to enter a second time for the same context, which is how
/// a nested `run_callbacks()` from inside a callback turns into a no-op instead
/// of a deadlock on the callback lock.
struct DispatchGuard {
    previous: *const (),
}

impl DispatchGuard {
    fn new(owner: *const ()) -> Self {
        let previous = DISPATCHING.with(|flag| flag.replace(owner));
        DispatchGuard { previous }
    }

    /// Enter for `owner`, or return `None` if this thread is already
    /// dispatching it.
    fn try_new(owner: *const ()) -> Option<Self> {
        if DISPATCHING.with(|flag| flag.get()) == owner {
            return None;
        }
        Some(Self::new(owner))
    }
}

impl Drop for DispatchGuard {
    fn drop(&mut self) {
        DISPATCHING.with(|flag| flag.set(self.previous));
    }
}

impl Inner {
    /// Runs any currently pending callbacks
    ///
    /// This runs all currently pending callbacks on the current
    /// thread.
    ///
    /// This should be called frequently (e.g. once per a frame)
    /// in order to reduce the latency between receiving events.
    ///
    /// Calling it from inside a callback is a no-op for this context: the
    /// dispatch loop already running on this thread keeps pulling events from
    /// the same queue, so nothing is missed and nothing re-enters the lock.
    pub fn run_callbacks(&self) {
        self.run_callbacks_raw(|cb_discrim, data| {
            let mut callbacks = self.callbacks.callbacks.lock().unwrap();
            if let Some(cb) = callbacks.get_mut(&cb_discrim) {
                cb(data);
            }
        });
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
        self.run_callbacks_raw(|cb_discrim, data| {
            {
                let mut callbacks = self.callbacks.callbacks.lock().unwrap();
                if let Some(cb) = callbacks.get_mut(&cb_discrim) {
                    cb(data);
                }
            }
            let cb_result = unsafe { CallbackResult::from_raw(cb_discrim, data) };
            if let Some(cb_result) = cb_result {
                callback_handler(cb_result);
            }
        });
    }

    fn run_callbacks_raw(&self, mut callback_handler: impl FnMut(i32, *mut c_void)) {
        // The whole dispatch loop runs with this thread marked as dispatching
        // for this context, so any registration or handle drop that a user
        // callback triggers gets deferred instead of re-entering the lock.
        // A nested dispatch of the same context returns here instead: the
        // outer loop keeps draining the same pipe, so nothing is lost, whereas
        // re-entering used to block on the callback lock forever.
        let Some(_dispatch) = DispatchGuard::try_new(self as *const Inner as *const ()) else {
            return;
        };
        unsafe {
            let pipe = self.manager.get_pipe();
            sys::SteamAPI_ManualDispatch_RunFrame(pipe);
            let mut callback = std::mem::zeroed();
            let mut apicall_result = Vec::new();
            while sys::SteamAPI_ManualDispatch_GetNextCallback(pipe, &mut callback) {
                if callback.m_iCallback == sys::SteamAPICallCompleted_t_k_iCallback as i32 {
                    let apicall = callback
                        .m_pubParam
                        .cast::<sys::SteamAPICallCompleted_t>()
                        .read_unaligned();
                    apicall_result.resize(apicall.m_cubParam as usize, 0u8);
                    let mut failed = false;
                    if sys::SteamAPI_ManualDispatch_GetAPICallResult(
                        pipe,
                        apicall.m_hAsyncCall,
                        apicall_result.as_mut_ptr().cast(),
                        apicall.m_cubParam as _,
                        apicall.m_iCallback,
                        &mut failed,
                    ) {
                        // Take the handler out under the lock and invoke it only after the
                        // guard is dropped: a call result handler routinely starts another
                        // async call (server-browser join after a list result, subscribe
                        // after create_item), and `register_call_result` needs this very
                        // lock, which `std::sync::Mutex` is not reentrant on.
                        // The &{val} pattern here is to avoid taking a reference to a packed field
                        // Since the value here is Copy, we can just copy it and borrow the copy
                        let handler = {
                            let mut call_results = self.callbacks.call_results.lock().unwrap();
                            call_results.remove(&{ apicall.m_hAsyncCall })
                        };
                        if let Some(cb) = handler {
                            cb(apicall_result.as_mut_ptr().cast(), failed);
                        }
                    }
                } else {
                    callback_handler(callback.m_iCallback, callback.m_pubParam.cast());
                }
                sys::SteamAPI_ManualDispatch_FreeLastCallback(pipe);
                // The handler's lock is released by now, so deferred
                // registrations can take it. Draining per callback (rather
                // than once at the end) means a callback registered from
                // inside a callback can fire in the same dispatch pass it
                // arrived in.
                self.callbacks.drain();
            }
        }
    }
}

struct NetworkingSocketsData {
    sockets: HashMap<
        sys::HSteamListenSocket,
        (
            Weak<networking_sockets::InnerSocket>,
            Sender<networking_types::ListenSocketEvent>,
        ),
    >,
    /// Connections to a remote listening port
    independent_connections:
        HashMap<sys::HSteamNetConnection, Sender<networking_types::NetConnectionEvent>>,
    connection_callback: Weak<CallbackHandle>,
}

/// Returns true if the app wasn't launched through steam and
/// begins relaunching it, the app should exit as soon as possible.
///
/// Returns false if the app was either launched through steam
/// or has a `steam_appid.txt`
pub fn restart_app_if_necessary(app_id: AppId) -> bool {
    unsafe { sys::SteamAPI_RestartAppIfNecessary(app_id.0) }
}

fn static_assert_send<T: Send>() {}
fn static_assert_sync<T>()
where
    T: Sync,
{
}

impl Client {
    /// Call to the native SteamAPI_Init function.
    /// should not be used directly, but through either
    /// init_flat() or init_flat_app()
    unsafe fn steam_api_init_flat(p_out_err_msg: *mut SteamErrMsg) -> ESteamAPIInitResult {
        unsafe { sys::SteamAPI_InitFlat(p_out_err_msg) }
    }

    /// Attempts to initialize the steamworks api without full API integration
    /// through SteamAPI_InitFlat added in SDK 1.59
    /// and returns a client to access the rest of the api.
    ///
    /// This should only ever have one instance per a program.
    ///
    /// # Errors
    ///
    /// This can fail if:
    /// * The steam client isn't running
    /// * The app ID of the game couldn't be determined.
    ///
    ///   If the game isn't being run through steam this can be provided by
    ///   placing a `steam_appid.txt` with the ID inside in the current
    ///   working directory. Alternatively, you can use `Client::init_app(<app_id>)`
    ///   to force a specific app ID.
    /// * The game isn't running on the same user/level as the steam client
    /// * The user doesn't own a license for the game.
    /// * The app ID isn't completely set up.
    pub fn init() -> Result<Client, SteamAPIInitError> {
        static_assert_send::<Client>();
        static_assert_sync::<Client>();
        unsafe {
            let mut err_msg: sys::SteamErrMsg = [0; 1024];
            let result = Self::steam_api_init_flat(&mut err_msg);

            if result != sys::ESteamAPIInitResult::k_ESteamAPIInitResult_OK {
                return Err(SteamAPIInitError::from_result_and_message(result, err_msg));
            }

            sys::SteamAPI_ManualDispatch_Init();
            let client = Arc::new(Inner {
                manager: Manager::Client,
                callbacks: Callbacks::new(),
                networking_sockets_data: Mutex::new(NetworkingSocketsData {
                    sockets: Default::default(),
                    independent_connections: Default::default(),
                    connection_callback: Default::default(),
                }),
            });
            Ok(Client { inner: client })
        }
    }

    /// Attempts to initialize the steamworks api with the APP_ID
    /// without full API integration through SteamAPI_InitFlat
    /// and returns a client to access the rest of the api.
    ///
    /// This should only ever have one instance per a program.
    ///
    /// # Errors
    ///
    /// This can fail if:
    /// * The steam client isn't running
    /// * The game isn't running on the same user/level as the steam client
    /// * The user doesn't own a license for the game.
    /// * The app ID isn't completely set up.
    pub fn init_app<ID: Into<AppId>>(app_id: ID) -> Result<Client, SteamAPIInitError> {
        let app_id = app_id.into().0.to_string();
        std::env::set_var("SteamAppId", &app_id);
        std::env::set_var("SteamGameId", app_id);
        Client::init()
    }
}

impl Client {
    /// Runs any currently pending callbacks
    ///
    /// This runs all currently pending callbacks on the current
    /// thread.
    ///
    /// This should be called frequently (e.g. once per a frame)
    /// in order to reduce the latency between recieving events.
    pub fn run_callbacks(&self) {
        self.inner.run_callbacks()
    }

    /// Runs any currently pending callbacks.
    ///
    /// This is identical to `run_callbacks` in every way, except that
    /// `callback_handler` is called for every callback invoked.
    ///
    /// This option provides an alternative for handling callbacks that
    /// can doesn't require the handler to be `Send`, and `'static`.
    ///
    /// This should be called frequently (e.g. once per a frame)
    /// in order to reduce the latency between recieving events.
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

    /// Returns an accessor to the steam utils interface
    pub fn utils(&self) -> Utils {
        unsafe {
            let utils = sys::SteamAPI_SteamUtils_v010();
            debug_assert!(!utils.is_null());
            Utils {
                utils: utils,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam matchmaking interface
    pub fn matchmaking(&self) -> Matchmaking {
        unsafe {
            let mm = sys::SteamAPI_SteamMatchmaking_v009();
            debug_assert!(!mm.is_null());
            Matchmaking {
                mm: mm,
                inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam matchmaking_servers interface
    pub fn matchmaking_servers(&self) -> MatchmakingServers {
        unsafe {
            let mm = sys::SteamAPI_SteamMatchmakingServers_v002();
            debug_assert!(!mm.is_null());
            MatchmakingServers {
                mms: mm,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam networking interface
    pub fn networking(&self) -> Networking {
        unsafe {
            let net = sys::SteamAPI_SteamNetworking_v006();
            debug_assert!(!net.is_null());
            Networking {
                net: net,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam apps interface
    pub fn apps(&self) -> Apps {
        unsafe {
            let apps = sys::SteamAPI_SteamApps_v009();
            debug_assert!(!apps.is_null());
            Apps {
                apps: apps,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam friends interface
    pub fn friends(&self) -> Friends {
        unsafe {
            let friends = sys::SteamAPI_SteamFriends_v018();
            debug_assert!(!friends.is_null());
            Friends {
                friends: friends,
                inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam input interface
    pub fn input(&self) -> Input {
        unsafe {
            let input = sys::SteamAPI_SteamInput_v006();
            debug_assert!(!input.is_null());
            Input {
                input,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam user interface
    pub fn user(&self) -> User {
        unsafe {
            let user = sys::SteamAPI_SteamUser_v023();
            debug_assert!(!user.is_null());
            User {
                user,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam user stats interface
    pub fn user_stats(&self) -> UserStats {
        unsafe {
            let us = sys::SteamAPI_SteamUserStats_v013();
            debug_assert!(!us.is_null());
            UserStats {
                user_stats: us,
                inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam remote play interface
    pub fn remote_play(&self) -> RemotePlay {
        unsafe {
            let rp = sys::SteamAPI_SteamRemotePlay_v004();
            debug_assert!(!rp.is_null());
            RemotePlay {
                rp,
                inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam remote storage interface
    pub fn remote_storage(&self) -> RemoteStorage {
        unsafe {
            let rs = sys::SteamAPI_SteamRemoteStorage_v016();
            debug_assert!(!rs.is_null());
            let util = sys::SteamAPI_SteamUtils_v010();
            debug_assert!(!util.is_null());
            RemoteStorage {
                rs,
                util,
                inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam screenshots interface
    pub fn screenshots(&self) -> Screenshots {
        unsafe {
            let screenshots = sys::SteamAPI_SteamScreenshots_v003();
            debug_assert!(!screenshots.is_null());
            Screenshots {
                screenshots,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam UGC interface (steam workshop)
    pub fn ugc(&self) -> UGC {
        unsafe {
            let ugc = sys::SteamAPI_SteamUGC_v021();
            debug_assert!(!ugc.is_null());
            UGC {
                ugc,
                inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam inventory interface
    pub fn inventory(&self) -> Inventory {
        unsafe {
            let inv = sys::SteamAPI_SteamInventory_v003();
            debug_assert!(!inv.is_null());
            Inventory {
                inv,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam HTTP interface
    pub fn http(&self) -> Http {
        unsafe {
            let http = sys::SteamAPI_SteamHTTP_v003();
            debug_assert!(!http.is_null());
            Http {
                http,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam parental settings interface
    pub fn parental_settings(&self) -> ParentalSettings {
        unsafe {
            let ps = sys::SteamAPI_SteamParentalSettings_v001();
            debug_assert!(!ps.is_null());
            ParentalSettings {
                ps,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam music interface
    pub fn music(&self) -> Music {
        unsafe {
            let music = sys::SteamAPI_SteamMusic_v001();
            debug_assert!(!music.is_null());
            Music {
                music,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam video interface
    pub fn video(&self) -> Video {
        unsafe {
            let video = sys::SteamAPI_SteamVideo_v007();
            debug_assert!(!video.is_null());
            Video {
                video,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam parties interface
    pub fn parties(&self) -> Parties {
        unsafe {
            let parties = sys::SteamAPI_SteamParties_v002();
            debug_assert!(!parties.is_null());
            Parties {
                parties,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam HTML surface interface
    ///
    /// Call [`HtmlSurface::init`] before creating any browser.
    pub fn html_surface(&self) -> HtmlSurface {
        unsafe {
            let html = sys::SteamAPI_SteamHTMLSurface_v005();
            debug_assert!(!html.is_null());
            HtmlSurface {
                html,
                _inner: self.inner.clone(),
            }
        }
    }

    /// Returns an accessor to the steam timeline interface
    pub fn timeline(&self) -> Timeline {
        unsafe {
            let timeline = sys::SteamAPI_SteamTimeline_v004();

            Timeline {
                timeline,
                disabled: timeline.is_null(),
                _inner: self.inner.clone(),
            }
        }
    }

    pub fn networking_messages(&self) -> networking_messages::NetworkingMessages {
        unsafe {
            let net = sys::SteamAPI_SteamNetworkingMessages_SteamAPI_v002();
            debug_assert!(!net.is_null());
            networking_messages::NetworkingMessages {
                net,
                inner: self.inner.clone(),
            }
        }
    }

    pub fn networking_sockets(&self) -> networking_sockets::NetworkingSockets {
        unsafe {
            let sockets = sys::SteamAPI_SteamNetworkingSockets_SteamAPI_v012();
            debug_assert!(!sockets.is_null());
            networking_sockets::NetworkingSockets {
                sockets,
                inner: self.inner.clone(),
            }
        }
    }

    pub fn networking_utils(&self) -> networking_utils::NetworkingUtils {
        unsafe {
            let utils = sys::SteamAPI_SteamNetworkingUtils_SteamAPI_v004();
            debug_assert!(!utils.is_null());
            networking_utils::NetworkingUtils {
                utils,
                inner: self.inner.clone(),
            }
        }
    }
}

/// Used to separate client and game server modes
enum Manager {
    Client,
    Server,
}

impl Manager {
    /// Returns the pipe handle for the steam api
    fn get_pipe(&self) -> sys::HSteamPipe {
        match self {
            Manager::Client => unsafe { sys::SteamAPI_GetHSteamPipe() },
            Manager::Server => unsafe { sys::SteamGameServer_GetHSteamPipe() },
        }
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        // SAFETY: This is considered unsafe only because of FFI, the function is otherwise
        // always safe to call from any thread.
        match self {
            Manager::Client => unsafe { sys::SteamAPI_Shutdown() },
            Manager::Server => unsafe { sys::SteamGameServer_Shutdown() },
        }
    }
}
