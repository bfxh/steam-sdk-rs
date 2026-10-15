//! Types that are used by both [`networking_sockets`](../networking_sockets) and [`networking_messages`](../networking_messages).
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::networking_sockets::{InnerSocket, NetConnection};
use crate::networking_types::NetConnectionError::UnhandledType;
use crate::{Callback, Inner, SteamId, SteamResult};
use std::convert::{TryFrom, TryInto};
use std::ffi::{c_void, CString};
use std::fmt::{Debug, Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::panic::catch_unwind;
use std::sync::Arc;
use steamworks_sys as sys;
use steamworks_sys::ESteamNetConnectionEnd;

mod config;
mod connection_info;
mod end;

pub use config::*;
pub use connection_info::*;
pub use end::*;

mod addresses;
mod events;
mod identity;
mod message;
mod state;

// `addresses` holds `SteamIpAddr`, which is crate-internal, so this re-export is no broader.
pub(crate) use addresses::*;
pub use events::*;
pub use identity::*;
pub use message::*;
pub use state::*;
