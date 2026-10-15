use super::*;
use crate::networking_types::NetworkingIdentity;
#[cfg(test)]
use serial_test::serial;

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

    /// Whether the account's phone number is verified (`isteamuser.h:196`).
    pub fn is_phone_verified(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUser_BIsPhoneVerified(self.user) }
    }

    /// Whether the account has two-factor authentication enabled (`isteamuser.h:199`).
    pub fn is_two_factor_enabled(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUser_BIsTwoFactorEnabled(self.user) }
    }

    /// Whether the account's phone number is currently identifying (`isteamuser.h:202`).
    pub fn is_phone_identifying(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUser_BIsPhoneIdentifying(self.user) }
    }

    /// Whether the account's phone number is awaiting (re)verification (`isteamuser.h:205`).
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
            let c_str = CString::new(identity).unwrap();
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
    /// Both numbers come from `isteamuser.h:175-177`: series is 1 when the game has only one set
    /// of cards, and a user can hold two badges per series -- regular, which tops out at 5, and
    /// foil, which tops out at 1.
    pub fn game_badge_level(&self, series: i32, foil: bool) -> i32 {
        unsafe {
            sys::SteamAPI_ISteamUser_GetGameBadgeLevel(
                self.user,
                series as ::std::os::raw::c_int,
                foil,
            )
        }
    }
}

/// How long the game says the player is playing, for the Steam China duration-control system.
///
/// The four states are `steamclientpublic.h:462-465` and the comments there are the definition;
/// `Invalid` is Steam's own "nil value", listed here because passing it is what clears a state.
/// Input-only: nothing comes back through this type, so it needs no fallback variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationControlOnlineState {
    /// Nil value (`k_EDurationControlOnlineState_Invalid`).
    Invalid,
    /// "currently in offline play - single-player, offline co-op, etc."
    Offline,
    /// "currently in online play"
    Online,
    /// "currently in online play and requests not to be interrupted"
    OnlineHighPriority,
}

impl From<DurationControlOnlineState> for sys::EDurationControlOnlineState {
    fn from(state: DurationControlOnlineState) -> Self {
        match state {
            DurationControlOnlineState::Invalid => {
                sys::EDurationControlOnlineState::k_EDurationControlOnlineState_Invalid
            }
            DurationControlOnlineState::Offline => {
                sys::EDurationControlOnlineState::k_EDurationControlOnlineState_Offline
            }
            DurationControlOnlineState::Online => {
                sys::EDurationControlOnlineState::k_EDurationControlOnlineState_Online
            }
            DurationControlOnlineState::OnlineHighPriority => {
                sys::EDurationControlOnlineState::k_EDurationControlOnlineState_OnlineHighPri
            }
        }
    }
}

/// Errors from `begin_authentication_session`
#[derive(Debug, Error)]
pub enum AuthSessionError {
    /// The ticket is invalid
    #[error("invalid ticket")]
    InvalidTicket,
    /// A ticket has already been submitted for this steam ID
    #[error("duplicate ticket request")]
    DuplicateRequest,
    /// The ticket is from an incompatible interface version
    #[error("incompatible interface version")]
    InvalidVersion,
    /// The ticket is not for this game
    #[error("incorrect game for ticket")]
    GameMismatch,
    /// The ticket has expired
    #[error("ticket has expired")]
    ExpiredTicket,
    /// Steam answered with a result code this binding does not name.
    ///
    /// The sys enum is `#[non_exhaustive]`, so the mapping's wildcard arm cannot be dropped;
    /// it used to be `unreachable!()`, which panicked in the caller's process. Not reachable
    /// with today's headers -- every declared code is named -- which is why there is no test
    /// for this arm: the honest guard is the panic-site ledger, not a constructed lie.
    #[error("unknown authentication session result")]
    Unknown,
}

#[test]
#[serial]
#[ignore = "requires a running Steam client"]
fn test_auth_dll() {
    let client = Client::init().unwrap();
    let user = client.user();

    let _cb = client.register_callback(|v: AuthSessionTicketResponse| {
        println!("Got dll auth response: {:?}", v)
    });
    let _cb = client.register_callback(|v: ValidateAuthTicketResponse| {
        println!("Got validate auth response: {:?}", v)
    });

    let id = user.steam_id();
    let (auth, ticket) = user.authentication_session_ticket_with_steam_id(id);

    println!("{:?}", auth);
    println!("{:?}", ticket);

    println!("{:?}", user.begin_authentication_session(id, &ticket));

    // this might still return NoAuth if the validation has not completed
    let has_space_war = user.user_has_license_for_app(id, AppId(480));
    println!("User has license response: {has_space_war:?}");

    for _ in 0..20 {
        client.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(50));
    }

    println!("END");

    user.cancel_authentication_ticket(auth);

    for _ in 0..20 {
        client.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(50));
    }

    user.end_authentication_session(id);
}

/// A handle for an authentication ticket that can be used to cancel
/// it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthTicket(pub(crate) sys::HAuthTicket);

/// Called when generating a authentication session ticket.
///
/// This can be used to verify the ticket was created successfully.
#[derive(Clone, Debug)]
pub struct AuthSessionTicketResponse {
    /// The ticket in question
    pub ticket: AuthTicket,
    /// The result of generating the ticket
    pub result: SteamResult,
}

impl_callback!(cb: GetAuthSessionTicketResponse_t => AuthSessionTicketResponse {
    Self {
        ticket: AuthTicket(cb.m_hAuthTicket),
        result: to_steam_result(cb.m_eResult),
    }
});

#[test]
#[serial]
#[ignore = "requires a running Steam client"]
fn test_auth_webapi() {
    let client = Client::init().unwrap();
    let user = client.user();

    let _cb = client.register_callback(|v: TicketForWebApiResponse| {
        println!("Got webapi auth response: {:?}", v)
    });

    let auth = user.authentication_session_ticket_for_webapi("myIdentity");

    println!("{:?}", auth);

    for _ in 0..20 {
        client.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(100));
    }

    println!("END");
}

/// Called when generating a authentication session ticket for web api.
///
/// This can be used to verify the ticket was created successfully.
#[derive(Debug)]
pub struct TicketForWebApiResponse {
    pub ticket_handle: AuthTicket,
    pub result: SteamResult,
    pub ticket_len: i32,
    pub ticket: Vec<u8>,
}

impl_callback!(cb: GetTicketForWebApiResponse_t => TicketForWebApiResponse {
    Self {
        ticket_handle: AuthTicket(cb.m_hAuthTicket),
        result: to_steam_result(cb.m_eResult),
        ticket_len: cb.m_cubTicket,
        ticket: cb.m_rgubTicket.to_vec(),
    }
});

/// Called when an authentication ticket has been
/// validated.
#[derive(Clone, Debug)]
pub struct ValidateAuthTicketResponse {
    /// The steam id of the entity that provided the ticket
    pub steam_id: SteamId,
    /// The result of the validation
    pub response: Result<(), AuthSessionValidateError>,
    /// The steam id of the owner of the game. Differs from
    /// `steam_id` if the game is borrowed.
    pub owner_steam_id: SteamId,
}

impl_callback!(cb: ValidateAuthTicketResponse_t => ValidateAuthTicketResponse {
    Self {
        steam_id: SteamId(cb.m_SteamID.m_steamid.m_unAll64Bits),
        owner_steam_id: SteamId(cb.m_OwnerSteamID.m_steamid.m_unAll64Bits),
        response: match cb.m_eAuthSessionResponse {
            sys::EAuthSessionResponse::k_EAuthSessionResponseOK => Ok(()),
            sys::EAuthSessionResponse::k_EAuthSessionResponseUserNotConnectedToSteam => {
                Err(AuthSessionValidateError::UserNotConnectedToSteam)
            }
            sys::EAuthSessionResponse::k_EAuthSessionResponseNoLicenseOrExpired => {
                Err(AuthSessionValidateError::NoLicenseOrExpired)
            }
            sys::EAuthSessionResponse::k_EAuthSessionResponseVACBanned => {
                Err(AuthSessionValidateError::VACBanned)
            }
            sys::EAuthSessionResponse::k_EAuthSessionResponseLoggedInElseWhere => {
                Err(AuthSessionValidateError::LoggedInElseWhere)
            }
            sys::EAuthSessionResponse::k_EAuthSessionResponseVACCheckTimedOut => {
                Err(AuthSessionValidateError::VACCheckTimedOut)
            }
            sys::EAuthSessionResponse::k_EAuthSessionResponseAuthTicketCanceled => {
                Err(AuthSessionValidateError::AuthTicketCancelled)
            }
            sys::EAuthSessionResponse::k_EAuthSessionResponseAuthTicketInvalidAlreadyUsed => {
                Err(AuthSessionValidateError::AuthTicketInvalidAlreadyUsed)
            }
            sys::EAuthSessionResponse::k_EAuthSessionResponseAuthTicketInvalid => {
                Err(AuthSessionValidateError::AuthTicketInvalid)
            }
            sys::EAuthSessionResponse::k_EAuthSessionResponsePublisherIssuedBan => {
                Err(AuthSessionValidateError::PublisherIssuedBan)
            }
            sys::EAuthSessionResponse::k_EAuthSessionResponseAuthTicketNetworkIdentityFailure => {
                Err(AuthSessionValidateError::AuthTicketNetworkIdentityFailure)
            }
            _ => Err(AuthSessionValidateError::Unknown),
        },
    }
});

// Called when a microtransaction authorization response is received
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MicroTxnAuthorizationResponse {
    pub app_id: AppId,
    pub order_id: u64,
    pub authorized: bool,
}

impl_callback!(cb: MicroTxnAuthorizationResponse_t => MicroTxnAuthorizationResponse {
    Self {
        app_id: cb.m_unAppID.into(),
        order_id: cb.m_ulOrderID.into(),
        authorized: cb.m_bAuthorized == 1,
    }
});

/// Called when a connection to the Steam servers is made.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamServersConnected;

impl_callback!(_cb: SteamServersConnected_t => SteamServersConnected {
    Self
});

/// Called when the connection to the Steam servers is lost.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamServersDisconnected {
    /// The reason we were disconnected from the Steam servers
    pub reason: SteamResult,
}

impl_callback!(cb: SteamServersDisconnected_t => SteamServersDisconnected {
    Self {
        reason: to_steam_result(cb.m_eResult),
    }
});

/// Called when the connection to the Steam servers fails.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamServerConnectFailure {
    /// The reason we failed to connect to the Steam servers
    pub reason: SteamResult,
    /// Whether we are still retrying the connection.
    pub still_retrying: bool,
}

impl_callback!(cb: SteamServerConnectFailure_t => SteamServerConnectFailure {
    Self {
        reason: to_steam_result(cb.m_eResult),
        still_retrying: cb.m_bStillRetrying,
    }
});

/// Errors from [`ValidateAuthTicketResponse`]
#[derive(Clone, Debug, Error)]
pub enum AuthSessionValidateError {
    /// The user in question is not connected to steam
    #[error("user not connected to steam")]
    UserNotConnectedToSteam,
    /// The license has expired
    #[error("the license has expired")]
    NoLicenseOrExpired,
    /// The user is VAC banned from the game
    #[error("the user is VAC banned from this game")]
    VACBanned,
    /// The user has logged in elsewhere and the session
    /// has been disconnected
    #[error("the user is logged in elsewhere")]
    LoggedInElseWhere,
    /// VAC has been unable to perform anti-cheat checks on this
    /// user
    #[error("VAC check timed out")]
    VACCheckTimedOut,
    /// The ticket has been cancelled by the issuer
    #[error("the authentication ticket has been cancelled")]
    AuthTicketCancelled,
    /// The ticket has already been used
    #[error("the authentication ticket has already been used")]
    AuthTicketInvalidAlreadyUsed,
    /// The ticket is not from a user instance currently connected
    /// to steam
    #[error("the authentication ticket is invalid")]
    AuthTicketInvalid,
    /// The user is banned from the game (not VAC)
    #[error("the user is banned")]
    PublisherIssuedBan,
    /// The [`NetworkingIdentity`] of the client or game server calling
    /// [`User::begin_authentication_session`] does not match the one
    /// in the ticket.
    ///
    /// The ticket's networking identity is the one passed to [`User::authentication_session_ticket`].
    #[error("the authenticator's network identity does not match the one in the ticket")]
    AuthTicketNetworkIdentityFailure,
    /// Steam answered with a code this binding does not name; see
    /// [`AuthSessionError::Unknown`].
    #[error("unknown auth session response")]
    Unknown,
}

/// Results from [`User::user_has_license_for_app`]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserHasLicense {
    /// The user has a license for the specified app.
    HasLicense,
    /// Steam answered with a result this binding does not name; see
    /// [`AuthSessionError::Unknown`] for why the arm exists and why it is not tested.
    Unknown,
    /// The user does not have a license for the specified app.
    DoesNotHaveLicense,
    /// The user has not been authenticated.
    NoAuth,
}

/// The duration-control mapping is what stands between a game's claim about the current session
/// and what the playtime system records, so the four arms are pinned instead of trusted: each
/// name must reach its own Steam code, and no two may reach the same one.
#[cfg(test)]
mod duration_control_tests {
    use super::*;

    #[test]
    fn each_online_state_reaches_its_own_steam_code() {
        use sys::EDurationControlOnlineState as S;
        let named = [
            (
                DurationControlOnlineState::Invalid,
                S::k_EDurationControlOnlineState_Invalid,
            ),
            (
                DurationControlOnlineState::Offline,
                S::k_EDurationControlOnlineState_Offline,
            ),
            (
                DurationControlOnlineState::Online,
                S::k_EDurationControlOnlineState_Online,
            ),
            (
                DurationControlOnlineState::OnlineHighPriority,
                S::k_EDurationControlOnlineState_OnlineHighPri,
            ),
        ];
        for (ours, theirs) in named {
            assert_eq!(S::from(ours), theirs);
        }
        for i in 0..named.len() {
            for j in (i + 1)..named.len() {
                assert_ne!(S::from(named[i].0), S::from(named[j].0));
            }
        }
    }
}
