//! The authentication-ticket vocabulary: the two ticket shapes, the responses that
//! carry them, and the error enums they fail with. Split out because Steam grows this
//! set with new failure codes, not with new `ISteamUser` methods.

use super::*;
#[cfg(test)]
use serial_test::serial;

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
    /// The [`crate::networking_types::NetworkingIdentity`] of the client or game server calling
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
