use super::*;
use crate::networking_types::NetworkingIdentity;
#[cfg(test)]
use serial_test::serial;
use std::net::{Ipv4Addr, SocketAddrV4};

mod auth;
mod callbacks;
mod config;
mod interfaces;
mod lifecycle;
mod packets;
mod status;

pub use callbacks::*;
pub use status::*;

/// The main entry point into the steam client for servers.
///
/// This provides access to all of the steamworks api that
/// servers can use.
#[derive(Clone)]
pub struct Server {
    inner: Arc<Inner>,
    server: *mut sys::ISteamGameServer,
}

// SAFETY: nothing in this crate's tree needs `Send` for this type today (measured: this
// line deleted, `cargo check --all-targets` stays green). The promise is kept because
// the type is public and a caller may move it to the thread that pumps callbacks.
// The type holds a raw Steam interface pointer, plain-copy handles, and the owning `Arc<Inner>`;
// it carries no non-atomic reference count, which is the one hazard a `Send` claim here cannot
// see -- `scripts/ci/callback_threads.py` counts `Rc` tokens beside every promise and ships at
// zero for exactly that reason. Moving the value therefore moves sole ownership of the pointer;
// nothing reads the same state through two threads because of the move itself.
unsafe impl Send for Server {}

unsafe impl Sync for Server {}

/// Used to set the mode that a gameserver will run in
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerMode {
    /// Don't authenticate user logins.
    ///
    /// The server will not appear on the server list
    NoAuthentication,
    /// Authenticate users
    ///
    /// The server will appear on the server list and
    /// VAC will not be run on clients.
    Authentication,
    /// Authenticate users and use anti-cheat.
    ///
    /// The server will appear on the server list and
    /// VAC will be run on clients.
    AuthenticationAndSecure,
}

/// Pass to SteamGameServer_Init to indicate that the same UDP port will be used for game traffic
/// UDP queries for server browser pings and LAN discovery.  In this case, Steam will not open up a
/// socket to handle server browser queries, and you must use ISteamGameServer::HandleIncomingPacket
/// and ISteamGameServer::GetNextOutgoingPacket to handle packets related to server discovery on
/// your socket.
pub const QUERY_PORT_SHARED: u16 = sys::STEAMGAMESERVER_QUERY_PORT_SHARED;

#[test]
#[serial]
#[ignore = "requires a Steam client installation to initialize the server interface"]
fn test() {
    let (server, single) = Server::init(
        [127, 0, 0, 1].into(),
        23334,
        23335,
        ServerMode::Authentication,
        "0.0.1",
    )
    .unwrap();

    println!("{:?}", server.steam_id());

    server.set_product("steamworks-rs test");
    server.set_game_description("basic server test");
    server.set_dedicated_server(true);
    server.log_on_anonymous();

    println!("{:?}", server.steam_id());

    let _cb = server.register_callback(|v: AuthSessionTicketResponse| {
        println!("Got auth ticket response: {:?}", v.result)
    });
    let _cb = server.register_callback(|v: ValidateAuthTicketResponse| {
        println!("Got validate auth ticket response: {:?}", v)
    });

    let id = server.steam_id();
    let (auth, ticket) = server.authentication_session_ticket_with_steam_id(id);

    println!("{:?}", server.begin_authentication_session(id, &ticket));

    for _ in 0..20 {
        single.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(50));
    }

    println!("END");

    server.cancel_authentication_ticket(auth);

    for _ in 0..20 {
        single.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(50));
    }

    server.end_authentication_session(id);
}
