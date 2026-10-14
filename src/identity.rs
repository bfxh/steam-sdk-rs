//! The identity types a Steam account is described with: the 64-bit id and the pieces
//! Steam splits it into.
//!
//! Separated from the crate root because these are value types with their own mapping rules
//! (account type, universe, instance), while the rest of `lib.rs` is the client, the interface
//! getters and the callback dispatcher.

use super::*;

/// A user's steam id
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamId(pub(crate) u64);

impl SteamId {
    /// Creates a `SteamId` from a raw 64 bit value.
    ///
    /// May be useful for deserializing steam ids from
    /// a network or save format.
    pub fn from_raw(id: u64) -> SteamId {
        SteamId(id)
    }

    /// Returns the raw 64 bit value of the steam id
    ///
    /// May be useful for serializing steam ids over a
    /// network or to a save format.
    pub fn raw(&self) -> u64 {
        self.0
    }

    /// Returns whether or not this Steam ID is invalid, which is when `account_type` is `k_EAccountTypeInvalid`.
    pub fn is_invalid(&self) -> bool {
        unsafe {
            let bits = sys::CSteamID_SteamID_t {
                m_unAll64Bits: self.0,
            };
            bits.m_comp.m_EAccountType()
                == sys::EAccountType::k_EAccountTypeInvalid as std::os::raw::c_uint
        }
    }

    /// Returns the account id for this steam id
    pub fn account_id(&self) -> AccountId {
        unsafe {
            let bits = sys::CSteamID_SteamID_t {
                m_unAll64Bits: self.0,
            };
            AccountId(bits.m_comp.m_unAccountID())
        }
    }

    /// Returns the Steam universe this Steam ID is part of.
    pub fn universe(&self) -> Universe {
        let bits = sys::CSteamID_SteamID_t {
            m_unAll64Bits: self.0,
        };
        match unsafe { bits.m_comp }.m_EUniverse() {
            sys::EUniverse::k_EUniversePublic => Universe::Public,
            sys::EUniverse::k_EUniverseBeta => Universe::Beta,
            sys::EUniverse::k_EUniverseInternal => Universe::Internal,
            sys::EUniverse::k_EUniverseDev => Universe::Dev,
            _ => Universe::Invalid,
        }
    }

    pub fn account_type(&self) -> AccountType {
        let bits = sys::CSteamID_SteamID_t {
            m_unAll64Bits: self.0,
        };
        match unsafe { bits.m_comp }.m_EAccountType() {
            x if x == sys::EAccountType::k_EAccountTypeIndividual as u32 => AccountType::Individual,
            x if x == sys::EAccountType::k_EAccountTypeMultiseat as u32 => AccountType::Multiseat,
            x if x == sys::EAccountType::k_EAccountTypeGameServer as u32 => AccountType::GameServer,
            x if x == sys::EAccountType::k_EAccountTypeAnonGameServer as u32 => {
                AccountType::AnonGameServer
            }
            x if x == sys::EAccountType::k_EAccountTypePending as u32 => AccountType::Pending,
            x if x == sys::EAccountType::k_EAccountTypeContentServer as u32 => {
                AccountType::ContentServer
            }
            x if x == sys::EAccountType::k_EAccountTypeClan as u32 => AccountType::Clan,
            x if x == sys::EAccountType::k_EAccountTypeChat as u32 => AccountType::Chat,
            x if x == sys::EAccountType::k_EAccountTypeConsoleUser as u32 => {
                AccountType::ConsoleUser
            }
            x if x == sys::EAccountType::k_EAccountTypeAnonUser as u32 => AccountType::AnonUser,
            _ => AccountType::Invalid,
        }
    }

    /// Returns the formatted SteamID32 string for this steam id.
    pub fn steamid32(&self) -> String {
        let account_id = self.account_id().raw();
        let last_bit = account_id & 1;
        format!("STEAM_0:{}:{}", last_bit, (account_id >> 1))
    }
}

/// Steam universes.
///
/// Each universe is a self-contained Steam instance.
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u32)]
pub enum Universe {
    Invalid = 0,
    Public = 1,
    Beta = 2,
    Internal = 3,
    Dev = 4,
}

/// Steam account types.
///
/// [`SteamId`]s are used to identify many different types of entities within Steam.
/// This type represents the type of account associated with a Steam ID.
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u32)]
pub enum AccountType {
    /// Invalid user account type.
    Invalid = 0,
    /// Individual user account.
    Individual = 1,
    /// Multiseat (e.g. cybercafe) account.
    Multiseat = 2,
    /// Game server account with a fixed Steam ID.
    GameServer = 3,
    /// Anonymous game server account.
    AnonGameServer = 4,
    /// Pending account.
    Pending = 5,
    /// Identifies a Steam content server.
    ContentServer = 6,
    /// Identifies a Steam clan.
    Clan = 7,
    /// Identifies a Steam chat.
    Chat = 8,
    /// Fake SteamID for local PSN account on PS3 or Live account on 360, etc.
    ConsoleUser = 9,
    /// Anoynmous user account.
    AnonUser = 10,
}

/// A user's account id
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AccountId(pub(crate) u32);

impl AccountId {
    /// Creates an `AccountId` from a raw 32 bit value.
    ///
    /// May be useful for deserializing account ids from
    /// a network or save format.
    pub fn from_raw(id: u32) -> AccountId {
        AccountId(id)
    }

    /// Returns the raw 32 bit value of the steam id
    ///
    /// May be useful for serializing steam ids over a
    /// network or to a save format.
    pub fn raw(&self) -> u32 {
        self.0
    }
}

/// A game id
///
/// Combines `AppId` and other information
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GameId(#[cfg_attr(feature = "serde", serde(rename = "game_id"))] pub(crate) u64);

impl GameId {
    /// Creates a `GameId` from a raw 64 bit value.
    ///
    /// May be useful for deserializing game ids from
    /// a network or save format.
    pub fn from_raw(id: u64) -> GameId {
        GameId(id)
    }

    /// Returns the raw 64 bit value of the game id
    ///
    /// May be useful for serializing game ids over a
    /// network or to a save format.
    pub fn raw(&self) -> u64 {
        self.0
    }

    /// Returns the app id of this game
    pub fn app_id(&self) -> AppId {
        // TODO: Relies on internal details
        AppId((self.0 & 0xFF_FF_FF) as u32)
    }
}
