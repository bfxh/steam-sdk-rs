//! Duration control: Steam's China playtime system, its input state, and the two
//! separate answer vocabularies (`m_progress` and `m_notification`) that a game must
//! not confuse. The mapping tests live here with the types they pin.

use super::*;

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

/// Steam's answer to [`User::duration_control`]; every field doc below is the comment
/// `isteamuser.h:410-419` puts on the matching member, not a reading of the name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DurationControl {
    /// "appid generating playtime" (`isteamuser.h:410`).
    pub app_id: AppId,
    /// "is duration control applicable to user + game combination" (`isteamuser.h:412`). When it
    /// is false the rest of the answer is still filled in.
    pub applicable: bool,
    /// "playtime since most recent 5 hour gap in playtime, only counting up to regulatory limit of
    /// playtime, in seconds" (`isteamuser.h:413`) -- not a rolling five-hour window.
    pub seconds_last_five_hours: i32,
    /// "recommended progress (either everything is fine, or please exit game)"
    /// (`isteamuser.h:415`).
    pub progress: DurationControlProgress,
    /// "notification to show, if any (always k_EDurationControlNotification_None for API calls)"
    /// (`isteamuser.h:416`). Through [`User::duration_control`] this is therefore Steam's own
    /// `None`; the field only says something on the timer-based callback.
    pub notification: DurationControlNotification,
    /// "playtime on current calendar day" (`isteamuser.h:418`).
    pub seconds_today: i32,
    /// "playtime remaining until the user hits a regulatory limit" (`isteamuser.h:419`). It is a
    /// signed `int32`, so the copy preserves the sign instead of taking a magnitude.
    pub seconds_remaining: i32,
}

impl DurationControl {
    pub(crate) fn from_raw(raw: &sys::DurationControl_t) -> Self {
        Self {
            app_id: AppId(raw.m_appid),
            applicable: raw.m_bApplicable,
            seconds_last_five_hours: raw.m_csecsLast5h,
            progress: DurationControlProgress::from_raw(raw.m_progress),
            notification: DurationControlNotification::from_raw(raw.m_notification),
            seconds_today: raw.m_csecsToday,
            seconds_remaining: raw.m_csecsRemaining,
        }
    }
}

/// What the playtime system recommends, or that the game should exit.
///
/// Read-only: nothing is sent back through this type, so no fallback value can be mistaken for a
/// Steam code. `steamclientpublic.h:425-435` puts progress and exit-soon requests in the *same*
/// enum under "WARNING: DO NOT RENUMBER" -- 0..2 are reward throttles, 3..5 tell the game to exit --
/// which is why this cannot be a plain three-way choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationControlProgress {
    /// "Full progress" (`steamclientpublic.h:428`).
    Full,
    /// "deprecated - XP or persistent rewards should be halved" (`steamclientpublic.h:429`).
    Half,
    /// "deprecated - XP or persistent rewards should be stopped" (`steamclientpublic.h:430`).
    None,
    /// "allowed 3h time since 5h gap/break has elapsed, game should exit - steam will terminate
    /// the game soon" (`steamclientpublic.h:432`).
    ExitSoonThreeHours,
    /// "allowed 5h time in calendar day has elapsed, game should exit - steam will terminate the
    /// game soon" (`steamclientpublic.h:433`).
    ExitSoonFiveHours,
    /// "game running after day period, game should exit - steam will terminate the game soon"
    /// (`steamclientpublic.h:434`).
    ExitSoonNight,
    /// A value this binding does not name.
    ///
    /// Not reachable with today's headers: every code the vendored enum declares is named above,
    /// and the wildcard only exists because the sys enum is `#[non_exhaustive]`. There is no test
    /// for it -- fabricating a discriminant for the C enum behind `EDurationControlProgress`
    /// (measured: `#[repr(i32)]` on Windows, `#[repr(u32)]` on the other three platform bindings) is
    /// undefined behaviour either way, not a reproduction, so the honest guard is the panic-site
    /// ledger.
    Unknown,
}

impl DurationControlProgress {
    fn from_raw(raw: sys::EDurationControlProgress) -> Self {
        match raw {
            sys::EDurationControlProgress::k_EDurationControlProgress_Full => Self::Full,
            sys::EDurationControlProgress::k_EDurationControlProgress_Half => Self::Half,
            sys::EDurationControlProgress::k_EDurationControlProgress_None => Self::None,
            sys::EDurationControlProgress::k_EDurationControl_ExitSoon_3h => {
                Self::ExitSoonThreeHours
            }
            sys::EDurationControlProgress::k_EDurationControl_ExitSoon_5h => {
                Self::ExitSoonFiveHours
            }
            sys::EDurationControlProgress::k_EDurationControl_ExitSoon_Night => Self::ExitSoonNight,
            // `#[non_exhaustive]`: this arm is the answer to "newer than us", not a claim that it
            // cannot happen.
            _ => Self::Unknown,
        }
    }
}

/// Which notification timer expired (`steamclientpublic.h:438-454`, also under "WARNING: DO NOT
/// RENUMBER"). Separate from [`DurationControlProgress`] because the header keeps them as two
/// vocabularies answering two questions, and `DurationControl_t` fills both fields -- so the crate
/// forwards both instead of collapsing one into the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationControlNotification {
    /// "just informing you about progress, no notification to show" (`steamclientpublic.h:444`).
    /// Also what the header promises for any answer to a *call*, as opposed to a timer.
    None,
    /// "you've been playing for N hours" (`steamclientpublic.h:445`).
    OneHour,
    /// "deprecated - you've been playing for 3 hours; take a break" (`steamclientpublic.h:447`).
    ThreeHours,
    /// "deprecated - your XP / progress is half normal" (`steamclientpublic.h:448`).
    HalfProgress,
    /// "deprecated - your XP / progress is zero" (`steamclientpublic.h:449`).
    NoProgress,
    /// "allowed 3h time since 5h gap/break has elapsed, game should exit"
    /// (`steamclientpublic.h:451`).
    ExitSoonThreeHours,
    /// "allowed 5h time in calendar day has elapsed, game should exit"
    /// (`steamclientpublic.h:452`).
    ExitSoonFiveHours,
    /// "game running after day period, game should exit" (`steamclientpublic.h:453`).
    ExitSoonNight,
    /// A value this binding does not name; same reason and same absence of a test as
    /// [`DurationControlProgress::Unknown`].
    Unknown,
}

impl DurationControlNotification {
    fn from_raw(raw: sys::EDurationControlNotification) -> Self {
        match raw {
            sys::EDurationControlNotification::k_EDurationControlNotification_None => Self::None,
            sys::EDurationControlNotification::k_EDurationControlNotification_1Hour => {
                Self::OneHour
            }
            sys::EDurationControlNotification::k_EDurationControlNotification_3Hours => {
                Self::ThreeHours
            }
            sys::EDurationControlNotification::k_EDurationControlNotification_HalfProgress => {
                Self::HalfProgress
            }
            sys::EDurationControlNotification::k_EDurationControlNotification_NoProgress => {
                Self::NoProgress
            }
            sys::EDurationControlNotification::k_EDurationControlNotification_ExitSoon_3h => {
                Self::ExitSoonThreeHours
            }
            sys::EDurationControlNotification::k_EDurationControlNotification_ExitSoon_5h => {
                Self::ExitSoonFiveHours
            }
            sys::EDurationControlNotification::k_EDurationControlNotification_ExitSoon_Night => {
                Self::ExitSoonNight
            }
            _ => Self::Unknown,
        }
    }
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

/// The two read-direction mappings and the field copies are what decide whether a game sees
/// Steam's answer or a plausible-looking one, so they are pinned against the vendored enums
/// rather than trusted: every declared code must land on its own variant, no two codes may
/// collide, and every struct field must arrive in the slot named for it.
#[cfg(test)]
mod duration_control_read_tests {
    use super::*;

    #[test]
    fn every_declared_progress_code_lands_on_its_own_variant() {
        use sys::EDurationControlProgress as S;
        let named = [
            (
                S::k_EDurationControlProgress_Full,
                DurationControlProgress::Full,
            ),
            (
                S::k_EDurationControlProgress_Half,
                DurationControlProgress::Half,
            ),
            (
                S::k_EDurationControlProgress_None,
                DurationControlProgress::None,
            ),
            (
                S::k_EDurationControl_ExitSoon_3h,
                DurationControlProgress::ExitSoonThreeHours,
            ),
            (
                S::k_EDurationControl_ExitSoon_5h,
                DurationControlProgress::ExitSoonFiveHours,
            ),
            (
                S::k_EDurationControl_ExitSoon_Night,
                DurationControlProgress::ExitSoonNight,
            ),
        ];
        for (raw, want) in named {
            assert_eq!(DurationControlProgress::from_raw(raw), want);
        }
        for i in 0..named.len() {
            for j in (i + 1)..named.len() {
                assert_ne!(named[i].1, named[j].1);
            }
        }
    }

    #[test]
    fn every_declared_notification_code_lands_on_its_own_variant() {
        use sys::EDurationControlNotification as S;
        let named = [
            (
                S::k_EDurationControlNotification_None,
                DurationControlNotification::None,
            ),
            (
                S::k_EDurationControlNotification_1Hour,
                DurationControlNotification::OneHour,
            ),
            (
                S::k_EDurationControlNotification_3Hours,
                DurationControlNotification::ThreeHours,
            ),
            (
                S::k_EDurationControlNotification_HalfProgress,
                DurationControlNotification::HalfProgress,
            ),
            (
                S::k_EDurationControlNotification_NoProgress,
                DurationControlNotification::NoProgress,
            ),
            (
                S::k_EDurationControlNotification_ExitSoon_3h,
                DurationControlNotification::ExitSoonThreeHours,
            ),
            (
                S::k_EDurationControlNotification_ExitSoon_5h,
                DurationControlNotification::ExitSoonFiveHours,
            ),
            (
                S::k_EDurationControlNotification_ExitSoon_Night,
                DurationControlNotification::ExitSoonNight,
            ),
        ];
        for (raw, want) in named {
            assert_eq!(DurationControlNotification::from_raw(raw), want);
        }
        for i in 0..named.len() {
            for j in (i + 1)..named.len() {
                assert_ne!(named[i].1, named[j].1);
            }
        }
        // Progress and notification are different Steam vocabularies whose exit-soon arms carry the
        // same *names*; the mapping must not confuse the two tables.
        assert_eq!(
            DurationControlNotification::from_raw(S::k_EDurationControlNotification_1Hour),
            DurationControlNotification::OneHour
        );
    }

    #[test]
    fn a_duration_control_answer_copies_every_field_to_its_own_slot() {
        let raw = sys::DurationControl_t {
            m_eResult: sys::EResult::k_EResultOK,
            m_appid: 42,
            m_bApplicable: true,
            m_csecsLast5h: -7,
            m_progress: sys::EDurationControlProgress::k_EDurationControl_ExitSoon_5h,
            m_notification: sys::EDurationControlNotification::k_EDurationControlNotification_1Hour,
            m_csecsToday: 1234,
            m_csecsRemaining: -56,
        };
        let answer = DurationControl::from_raw(&raw);
        assert_eq!(answer.app_id, AppId(42));
        assert!(answer.applicable);
        assert_eq!(answer.seconds_last_five_hours, -7);
        assert_eq!(answer.progress, DurationControlProgress::ExitSoonFiveHours);
        assert_eq!(answer.notification, DurationControlNotification::OneHour);
        assert_eq!(answer.seconds_today, 1234);
        // Both counters are signed `int32` in the header, so a negative answer must survive the
        // copy rather than come back as a magnitude.
        assert_eq!(answer.seconds_remaining, -56);
    }

    #[test]
    fn a_market_answer_forwards_the_reason_bitfield_without_rounding_it() {
        let raw = sys::MarketEligibilityResponse_t {
            m_bAllowed: false,
            m_eNotAllowedReason:
                sys::EMarketNotAllowedReasonFlags::k_EMarketNotAllowedReason_AccountDisabled,
            m_rtAllowedAtTime: 7,
            m_cdaySteamGuardRequiredDays: 3,
            m_cdayNewDeviceCooldown: 5,
        };
        let answer = MarketEligibility::from_raw(&raw);
        assert!(!answer.allowed);
        assert_eq!(answer.allowed_at, 7);
        assert_eq!(answer.steam_guard_required_days, 3);
        assert_eq!(answer.new_device_cooldown_days, 5);
        assert_eq!(
            answer.not_allowed_reason,
            sys::EMarketNotAllowedReasonFlags::k_EMarketNotAllowedReason_AccountDisabled
        );
        // Two reasons combined stay two reasons: the mirror would have to invent a variant for any
        // other behaviour, and forwarding raw is the whole point of not mirroring it.
        let combined = sys::EMarketNotAllowedReasonFlags::k_EMarketNotAllowedReason_AccountDisabled
            | sys::EMarketNotAllowedReasonFlags::k_EMarketNotAllowedReason_AccountLockedDown;
        let raw = sys::MarketEligibilityResponse_t {
            m_eNotAllowedReason: combined,
            ..raw
        };
        assert_eq!(
            MarketEligibility::from_raw(&raw).not_allowed_reason,
            combined
        );
        assert_eq!(combined.0, 6);
    }
}
