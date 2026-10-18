use super::*;
use crate::networking_types::NetworkingIdentity;
use crate::utils::c_string_prefix;

mod answers;
mod auth;
// Exports nothing of its own: both wrappers in it are methods on `User`, so it is declared rather
// than globbed out.
mod buffers;
mod callbacks;
mod duration;
mod passthrough;
mod voice;

pub use answers::*;
pub use auth::*;
pub use callbacks::*;
pub use duration::*;
pub use voice::*;

/// Access to the steam user interface
pub struct User {
    pub(crate) user: *mut sys::ISteamUser,
    pub(crate) _inner: Arc<Inner>,
}

impl User {
    /// Returns the steam id of the current user
    pub fn steam_id(&self) -> SteamId {
        unsafe { SteamId(sys::SteamAPI_ISteamUser_GetSteamID(self.user)) }
    }

    /// Returns the level of the current user
    pub fn level(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamUser_GetPlayerSteamLevel(self.user) as u32 }
    }

    /// Returns whether the current user's Steam client is connected to the Steam servers.
    pub fn logged_on(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUser_BLoggedOn(self.user) }
    }

    /// Whether this user looks like they are behind a NAT device.
    ///
    /// Two limits come straight from `isteamuser.h:152-153`: it is only valid once the user has
    /// connected to Steam (i.e. after a `SteamServersConnected_t` has been issued), and it "may
    /// not catch all forms of NAT". So `false` is not evidence of a public connection.
    pub fn is_behind_nat(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUser_BIsBehindNAT(self.user) }
    }

    /// Whether the account's phone number is verified; `isteamuser.h:196` reads "gets whether
    /// the users phone number is verified", grammar and all.
    pub fn is_phone_verified(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUser_BIsPhoneVerified(self.user) }
    }

    /// Whether the account has two-factor authentication enabled; `isteamuser.h:199` reads
    /// "gets whether the user has two factor enabled on their account".
    pub fn is_two_factor_enabled(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUser_BIsTwoFactorEnabled(self.user) }
    }

    /// Whether the account's phone number is currently identifying; `isteamuser.h:202` reads
    /// "gets whether the users phone number is identifying".
    pub fn is_phone_identifying(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUser_BIsPhoneIdentifying(self.user) }
    }

    /// Whether the account's phone number is awaiting (re)verification; `isteamuser.h:205`
    /// reads "gets whether the users phone number is awaiting (re)verification".
    pub fn is_phone_requiring_verification(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUser_BIsPhoneRequiringVerification(self.user) }
    }

    /// Retrieve an authentication session ticket that can be sent
    /// to an entity that wishes to verify you.
    ///
    /// This ticket should not be reused.
    ///
    /// When creating ticket for use by the web API you should wait
    /// for the `AuthSessionTicketResponse` event before trying to
    /// use the ticket.
    ///
    /// When the multiplayer session terminates you must call
    /// `cancel_authentication_ticket`
    pub fn authentication_session_ticket_with_steam_id(
        &self,
        steam_id: SteamId,
    ) -> (AuthTicket, Vec<u8>) {
        self.authentication_session_ticket(NetworkingIdentity::new_steam_id(steam_id))
    }
    pub fn authentication_session_ticket(
        &self,
        network_identity: NetworkingIdentity,
    ) -> (AuthTicket, Vec<u8>) {
        unsafe {
            let mut ticket = vec![0; 1024];
            let mut ticket_len = 0;
            let auth_ticket = sys::SteamAPI_ISteamUser_GetAuthSessionTicket(
                self.user,
                ticket.as_mut_ptr().cast(),
                1024,
                &mut ticket_len,
                network_identity.as_ptr(),
            );
            ticket.truncate(ticket_len as usize);
            (AuthTicket(auth_ticket), ticket)
        }
    }

    /// Cancels an authentication session ticket received from
    /// `authentication_session_ticket`.
    ///
    /// This should be called when you are no longer playing with
    /// the specified entity.
    pub fn cancel_authentication_ticket(&self, ticket: AuthTicket) {
        unsafe {
            sys::SteamAPI_ISteamUser_CancelAuthTicket(self.user, ticket.0);
        }
    }

    /// Authenticate the ticket from the steam ID to make sure it is
    /// valid and not reused.
    ///
    /// A `ValidateAuthTicketResponse` callback will be fired if
    /// the entity goes offline or cancels the ticket.
    ///
    /// When the multiplayer session terminates you must call
    /// `end_authentication_session`
    pub fn begin_authentication_session(
        &self,
        user: SteamId,
        ticket: &[u8],
    ) -> Result<(), AuthSessionError> {
        unsafe {
            let res = sys::SteamAPI_ISteamUser_BeginAuthSession(
                self.user,
                ticket.as_ptr().cast(),
                ticket.len() as _,
                user.0,
            );
            Err(match res {
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultOK => return Ok(()),
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultInvalidTicket => {
                    AuthSessionError::InvalidTicket
                }
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultDuplicateRequest => {
                    AuthSessionError::DuplicateRequest
                }
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultInvalidVersion => {
                    AuthSessionError::InvalidVersion
                }
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultGameMismatch => {
                    AuthSessionError::GameMismatch
                }
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultExpiredTicket => {
                    AuthSessionError::ExpiredTicket
                }
                _ => AuthSessionError::Unknown,
            })
        }
    }

    /// Ends an authentication session that was started with
    /// `begin_authentication_session`.
    ///
    /// This should be called when you are no longer playing with
    /// the specified entity.
    pub fn end_authentication_session(&self, user: SteamId) {
        unsafe {
            sys::SteamAPI_ISteamUser_EndAuthSession(self.user, user.0);
        }
    }

    /// Retrieve an authentication ticket to be sent to the entity that
    /// wishes to authenticate you using the
    /// ISteamUserAuth/AuthenticateUserTicket Web API.
    ///
    /// The calling application must wait for the
    /// `TicketForWebApiResponse` callback generated  
    /// by the API call to access the ticket.
    ///  
    /// It is best practice to use an identity string for
    /// each service that will consume tickets.
    ///   
    /// This API can not be used to create a ticket for
    /// use by the BeginAuthSession/ISteamGameServer::BeginAuthSession.
    /// Use the `authentication_session_ticket` API instead
    pub fn authentication_session_ticket_for_webapi(&self, identity: &str) -> AuthTicket {
        unsafe {
            let c_str = c_string_prefix(identity);
            let auth_ticket =
                sys::SteamAPI_ISteamUser_GetAuthTicketForWebApi(self.user, c_str.as_ptr());

            AuthTicket(auth_ticket)
        }
    }

    /// Checks if the user owns a piece of DLC specified by app id.
    ///
    /// This can only be called after authenticating
    /// with the user using `begin_authentication_session`.
    pub fn user_has_license_for_app(&self, user: SteamId, app_id: AppId) -> UserHasLicense {
        unsafe {
            let license_response =
                sys::SteamAPI_ISteamUser_UserHasLicenseForApp(self.user, user.0, app_id.0);

            match license_response {
                sys::EUserHasLicenseForAppResult::k_EUserHasLicenseResultHasLicense => {
                    UserHasLicense::HasLicense
                }
                sys::EUserHasLicenseForAppResult::k_EUserHasLicenseResultDoesNotHaveLicense => {
                    UserHasLicense::DoesNotHaveLicense
                }
                sys::EUserHasLicenseForAppResult::k_EUserHasLicenseResultNoAuth => {
                    UserHasLicense::NoAuth
                }
                _ => UserHasLicense::Unknown,
            }
        }
    }

    /// Tells the Steam China duration-control system what the game's online state is now.
    ///
    /// `isteamuser.h:215-217` states the purpose directly: it "will prevent offline gameplay time
    /// from counting against a user's playtime limits", so a game that never calls it charges the
    /// player for single-player time. Returns whether Steam accepted the change.
    pub fn set_duration_control_online_state(&self, state: DurationControlOnlineState) -> bool {
        unsafe { sys::SteamAPI_ISteamUser_BSetDurationControlOnlineState(self.user, state.into()) }
    }

    /// The sample rate Steam's voice decoder considers optimal, in Hz.
    ///
    /// Read the header before using it (`isteamuser.h:120-122`): Valve writes that you "may find
    /// that you get the best audio output quality when you ignore this function and use the
    /// native sample rate of your audio output device, which is usually 48000 or 44100".
    pub fn voice_optimal_sample_rate(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamUser_GetVoiceOptimalSampleRate(self.user) }
    }

    /// The user's trading-card badge level for one card series of this game.
    ///
    /// Both numbers are Steam's, at `isteamuser.h:175-177`: "if you only have one set of cards,
    /// the series will be 1", and "the user has can have two different badges for a series; the
    /// regular (max level 5) and the foil (max level 1)" -- that third person singularity is
    /// theirs, quoted rather than repaired.
    pub fn game_badge_level(&self, series: i32, foil: bool) -> i32 {
        unsafe {
            sys::SteamAPI_ISteamUser_GetGameBadgeLevel(
                self.user,
                series as ::std::os::raw::c_int,
                foil,
            )
        }
    }

    /// Asks Steam for a store-auth URL for `redirect_url` and delivers it through `cb`.
    ///
    /// The URL goes to Steam as a C string, so an embedded NUL is answered with
    /// `Err(InvalidParameter)` -- `cb` fires before this call returns -- rather than panicking or
    /// sending a silently truncated redirect. Steam's own answer is read out of the 512-byte
    /// inline buffer `StoreAuthURLResponse_t::m_szURL` (`isteamuser.h:378`) by cutting at the first
    /// NUL *inside that buffer*, so a URL that fills it cannot make this read past what Steam owns.
    /// That response carries no result code -- the struct is the callback id and the URL, nothing
    /// else: `isteamuser.h:373-379`, which Steam introduces as "sent to your game in response to
    /// ISteamUser::RequestStoreAuthURL" -- so the only failure this path can report is the transport
    /// one.
    pub fn request_store_auth_url<F>(&self, redirect_url: &str, cb: F)
    where
        F: FnOnce(Result<String, SteamError>) + 'static + Send,
    {
        let url = match CString::new(redirect_url) {
            Ok(url) => url,
            Err(_) => return cb(Err(SteamError::InvalidParameter)),
        };
        let api_call =
            unsafe { sys::SteamAPI_ISteamUser_RequestStoreAuthURL(self.user, url.as_ptr()) };
        unsafe {
            register_call_result::<sys::StoreAuthURLResponse_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    cb(Ok(crate::ugc::steam_out_string(&v.m_szURL)))
                },
            );
        }
    }

    /// Asks for an encrypted app ticket whose body carries `app_data`.
    ///
    /// The answer is the readiness signal only: the ticket bytes themselves come from
    /// [`User::encrypted_app_ticket`], which runs the two-pass out-parameter protocol
    /// ("If no ticket is available, or your buffer is too small, returns false... you can pass
    /// pTicket=NULL and cbMaxTicket=0" to learn the size, then allocate and ask again,
    /// `isteamuser.h:167-173`). The header marks this call itself "asynchronous, you must wait for
    /// the ticket to be completed by the server" (`isteamuser.h:163`), which is what the callback
    /// below is for. An empty `app_data` still has to hand Steam a non-null pointer, so it gets an
    /// aligned, never-dereferenced address, the same way `RemoteStorage::write_file` handles it.
    pub fn request_encrypted_app_ticket<F>(&self, app_data: &[u8], cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        let data_ptr: *const c_void = if app_data.is_empty() {
            std::ptr::NonNull::<u8>::dangling().as_ptr() as *const c_void
        } else {
            app_data.as_ptr() as *const c_void
        };
        let api_call = unsafe {
            sys::SteamAPI_ISteamUser_RequestEncryptedAppTicket(
                self.user,
                data_ptr as *mut c_void,
                app_data.len() as i32,
            )
        };
        unsafe {
            register_call_result::<sys::EncryptedAppTicketResponse_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult)
                    })
                },
            );
        }
    }

    /// Asks whether this account may use the Steam Market, answering through `cb`.
    pub fn market_eligibility<F>(&self, cb: F)
    where
        F: FnOnce(Result<MarketEligibility, SteamError>) + 'static + Send,
    {
        let api_call = unsafe { sys::SteamAPI_ISteamUser_GetMarketEligibility(self.user) };
        unsafe {
            register_call_result::<sys::MarketEligibilityResponse_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        Ok(MarketEligibility::from_raw(&v))
                    })
                },
            );
        }
    }

    /// Asks Steam's China duration-control system what it currently believes about this
    /// player's session, answering through `cb`.
    ///
    /// The answer carries both a progress and a notification, but for this call the header says
    /// what the second one holds: "always k_EDurationControlNotification_None for API calls"
    /// (`isteamuser.h:416`). They stay two mapped fields because `steamclientpublic.h` keeps them
    /// as two enums with two jobs -- the notification only carries meaning on the timer-based
    /// callback this wrapper does not replace.
    pub fn duration_control<F>(&self, cb: F)
    where
        F: FnOnce(Result<DurationControl, SteamError>) + 'static + Send,
    {
        let api_call = unsafe { sys::SteamAPI_ISteamUser_GetDurationControl(self.user) };
        unsafe {
            register_call_result::<sys::DurationControl_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult).map(|_| DurationControl::from_raw(&v))
                    })
                },
            );
        }
    }
}
