mod stat_callback;
pub mod stats;

pub use self::stat_callback::*;
use super::*;
#[cfg(test)]
use serial_test::serial;

mod achievements;
mod global_stats;
mod leaderboard;
mod stats_io;

pub use leaderboard::*;

/// Access to the steam user stats interface
pub struct UserStats {
    pub(crate) user_stats: *mut sys::ISteamUserStats,
    pub(crate) inner: Arc<Inner>,
}
