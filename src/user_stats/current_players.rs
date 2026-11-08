//! `GetNumberOfCurrentPlayers` and the two numbers it answers with.
//!
//! The interface reports this through `NumberOfCurrentPlayers_t`
//! (`isteamuserstats.h:251`, "GetNumberOfCurrentPlayers"): a success flag and a player count. The flag is what makes a
//! missing answer distinguishable from a real zero, so the mapping is a pure function and
//! is judged below without a client.

use super::*;

impl UserStats {
    /// Asks Steam how many players are currently in the game.
    ///
    /// The callback gets `None` when Steam answered `m_bSuccess == 0`; a negative count is
    /// treated the same way, because it cannot describe a number of players.
    pub fn current_players<F>(&self, cb: F)
    where
        F: FnOnce(Result<Option<u32>, SteamError>) + 'static + Send,
    {
        let api_call =
            unsafe { sys::SteamAPI_ISteamUserStats_GetNumberOfCurrentPlayers(self.user_stats) };
        unsafe {
            register_call_result::<sys::NumberOfCurrentPlayers_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| cb(current_players_result(v.m_bSuccess, v.m_cPlayers, io_error)),
            );
        }
    }
}

/// The `NumberOfCurrentPlayers_t` answer. `m_bSuccess` is `uint8` and `m_cPlayers` is
/// `int32`; only a successful, non-negative count is a value this crate will hand out.
fn current_players_result(
    success: u8,
    players: i32,
    io_error: bool,
) -> Result<Option<u32>, SteamError> {
    if io_error {
        Err(SteamError::IOFailure)
    } else if success != 0 && players >= 0 {
        Ok(Some(players as u32))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_successful_answer_is_the_count() {
        assert_eq!(current_players_result(1, 42, false).unwrap(), Some(42));
    }

    #[test]
    fn a_failed_answer_is_none_not_zero() {
        assert_eq!(current_players_result(0, 42, false).unwrap(), None);
    }

    #[test]
    fn a_negative_count_is_not_handed_out() {
        assert_eq!(current_players_result(1, -1, false).unwrap(), None);
    }

    #[test]
    fn an_io_error_is_an_error() {
        assert!(matches!(
            current_players_result(1, 42, true),
            Err(SteamError::IOFailure)
        ));
    }
}
