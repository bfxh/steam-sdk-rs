//! Kleene three-valued truth: [`False`](Ternary::False), [`Unknown`](Ternary::Unknown),
//! [`True`](Ternary::True).
//!
//! Steam answers a number of queries with "I cannot tell": the user is not authenticated,
//! the request is still in flight, the runtime returned a code this binding does not name.
//! The crate already keeps those answers in named variants (`UserHasLicense::NoAuth`,
//! `SteamError::Pending`, `FriendState::Unknown`), but each of them is a dead end: there was
//! no value that *composes*, so `owned && installed` could only be written by collapsing both
//! sides to `bool` and losing the unknown. `Ternary` is that composable value.
//!
//! The algebra is Kleene (K3), and the choice is load-bearing:
//! `False & Unknown == False`. Bochvar's reading of the same three values makes `Unknown`
//! absorb every operation, so `False & Unknown` would be `Unknown` -- it would turn a
//! definite "does not own it" into "don't know", which is the answer a store check must
//! never lose. The two cells where the algebras disagree are pinned by
//! `bochvar_would_lose_the_definite_answer` below.
//!
//! This module is pure data: no FFI, no `unsafe`, no allocation. It is one byte per value
//! (`#[repr(u8)]`), so `Vec<Ternary>` costs the same as `Vec<Option<bool>>`.
//!
//! ```
//! use steamworks::Ternary;
//!
//! // A definite "no" survives an unknown on the other side: this is the whole point.
//! assert_eq!(Ternary::False.and(Ternary::Unknown), Ternary::False);
//! // A conclusive "yes" does not: the answer is "I can't tell", not "yes".
//! assert_eq!(Ternary::True.and(Ternary::Unknown), Ternary::Unknown);
//! ```

use std::ops::Not;

use crate::UserHasLicense;

/// A Kleene truth value: what a query answered, including "it did not answer".
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[must_use = "a truth value that is dropped is a decision that was never made"]
pub enum Ternary {
    /// Answered no.
    False,
    /// Did not answer, or the answer is not conclusive. Not the same as [`False`](Self::False).
    Unknown,
    /// Answered yes.
    True,
}

impl Ternary {
    /// The two-valued answer, with [`Unknown`](Self::Unknown) becoming `None`.
    ///
    /// This is the only correct direction of the bridge: a `bool` cannot carry an unknown,
    /// so turning one into a `Ternary` is lossless and turning a `Ternary` back is not.
    pub const fn to_bool(self) -> Option<bool> {
        match self {
            Self::False => Some(false),
            Self::True => Some(true),
            Self::Unknown => None,
        }
    }

    /// A `bool` as a [`Ternary`]. Always conclusive; never produces [`Unknown`](Self::Unknown).
    pub const fn from_bool(value: bool) -> Self {
        if value {
            Self::True
        } else {
            Self::False
        }
    }

    /// Whether the answer was [`Unknown`](Self::Unknown).
    pub const fn is_unknown(self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// Kleene conjunction: `False` wins over `Unknown`, `Unknown` wins over `True`.
    ///
    /// This is the operation that makes the type worth having: a definite no on either side
    /// is a definite no on both.
    pub const fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::True, Self::True) => Self::True,
            _ => Self::Unknown,
        }
    }

    /// Kleene disjunction: `True` wins over `Unknown`, `Unknown` wins over `False`.
    pub const fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::False, Self::False) => Self::False,
            _ => Self::Unknown,
        }
    }

    /// Kleene implication, `not self or other`.
    ///
    /// Not tautological in the way a two-valued implication is: `Unknown.implies(Unknown)`
    /// is [`Unknown`](Self::Unknown), not `True`. A policy that must hold for every
    /// assignment therefore belongs in a premise ("assume the conclusion holds, then check
    /// the reason"), not in an `implies` chain.
    pub const fn implies(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::True) => Self::True,
            (Self::True, Self::False) => Self::False,
            _ => Self::Unknown,
        }
    }
}

impl Not for Ternary {
    type Output = Self;

    fn not(self) -> Self {
        match self {
            Self::False => Self::True,
            Self::True => Self::False,
            Self::Unknown => Self::Unknown,
        }
    }
}

impl From<bool> for Ternary {
    fn from(value: bool) -> Self {
        Self::from_bool(value)
    }
}

/// `NoAuth` and `Unknown` both mean Steam did not answer, so both become
/// [`Ternary::Unknown`]; the distinction between them stays in the original enum, which is
/// why this conversion exists instead of the wrapper returning `bool` in the first place.
impl From<UserHasLicense> for Ternary {
    fn from(license: UserHasLicense) -> Self {
        match license {
            UserHasLicense::HasLicense => Self::True,
            UserHasLicense::DoesNotHaveLicense => Self::False,
            UserHasLicense::NoAuth | UserHasLicense::Unknown => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALUES: [Ternary; 3] = [Ternary::False, Ternary::Unknown, Ternary::True];

    #[test]
    fn and_is_the_kleene_table() {
        use Ternary::{False as F, True as T, Unknown as X};
        let expected = [
            (F, F, F),
            (F, X, F),
            (F, T, F),
            (X, F, F),
            (X, X, X),
            (X, T, X),
            (T, F, F),
            (T, X, X),
            (T, T, T),
        ];
        for (a, b, want) in expected {
            assert_eq!(a.and(b), want, "{a:?} & {b:?}");
        }
    }

    #[test]
    fn bochvar_would_lose_the_definite_answer() {
        // These are the two cells where Bochvar's "X absorbs" reading differs from K3.
        // If somebody swaps the algebra, this is the test that says why that is a bug:
        // "the user definitively does not own it" would become "we don't know".
        assert_eq!(Ternary::False.and(Ternary::Unknown), Ternary::False);
        assert_eq!(Ternary::Unknown.and(Ternary::False), Ternary::False);
    }

    #[test]
    fn or_is_the_kleene_table() {
        use Ternary::{False as F, True as T, Unknown as X};
        let expected = [
            (F, F, F),
            (F, X, X),
            (F, T, T),
            (X, F, X),
            (X, X, X),
            (X, T, T),
            (T, F, T),
            (T, X, T),
            (T, T, T),
        ];
        for (a, b, want) in expected {
            assert_eq!(a.or(b), want, "{a:?} | {b:?}");
        }
    }

    #[test]
    fn implies_is_not_or() {
        use Ternary::{False as F, True as T, Unknown as X};
        let expected = [
            (F, F, T),
            (F, X, T),
            (F, T, T),
            (X, F, X),
            (X, X, X),
            (X, T, T),
            (T, F, F),
            (T, X, X),
            (T, T, T),
        ];
        for (a, b, want) in expected {
            assert_eq!(a.implies(b), want, "{a:?} => {b:?}");
        }
        // The reason policies must not be written as an `implies` chain: this is not True.
        assert_eq!(X.implies(X), X);
    }

    #[test]
    fn not_is_involutive_and_commutes_with_de_morgan() {
        for a in VALUES {
            assert_eq!(!(!a), a, "double negation of {a:?}");
        }
        for a in VALUES {
            for b in VALUES {
                assert_eq!(!(a.and(b)), (!a).or(!b), "De Morgan over ({a:?}, {b:?})");
                assert_eq!(!(a.or(b)), (!a).and(!b), "De Morgan over ({a:?}, {b:?})");
            }
        }
    }

    #[test]
    fn bool_bridge_is_lossless_in_one_direction_only() {
        for value in [false, true] {
            assert_eq!(Ternary::from_bool(value).to_bool(), Some(value));
            assert_eq!(Ternary::from(value), Ternary::from_bool(value));
        }
        assert_eq!(Ternary::Unknown.to_bool(), None);
        assert!(Ternary::Unknown.is_unknown());
        assert!(!Ternary::False.is_unknown());
        assert!(!Ternary::True.is_unknown());
    }

    #[test]
    fn one_byte_per_value() {
        // The storage and bulk-path statements in docs/ternary-integration.md assume this.
        assert_eq!(std::mem::size_of::<Ternary>(), 1);
    }

    #[test]
    fn license_projection_keeps_no_auth_as_unknown() {
        let mapped = [
            (UserHasLicense::HasLicense, Ternary::True),
            (UserHasLicense::DoesNotHaveLicense, Ternary::False),
            // Two different reasons, one composable answer. The distinction is not lost --
            // callers who need it still have the enum; this conversion is the opt-in.
            (UserHasLicense::NoAuth, Ternary::Unknown),
            (UserHasLicense::Unknown, Ternary::Unknown),
        ];
        for (license, want) in mapped {
            assert_eq!(Ternary::from(license), want);
        }
    }

    // An admission rule of the shape a game actually writes, exercised over every one of the
    // 3^5 assignments. This is the in-crate replacement for proving the policy with an
    // external engine: 243 cases are cheap enough to check exhaustively, which is a proof,
    // not a sample.
    fn can_play(
        owned: Ternary,
        installed: Ternary,
        license: Ternary,
        vac_banned: Ternary,
        parental_blocked: Ternary,
    ) -> Ternary {
        owned
            .and(installed)
            .and(license)
            .and(!vac_banned)
            .and(!parental_blocked)
    }

    #[test]
    fn unknown_never_becomes_yes() {
        let mut yes = 0;
        let mut no = 0;
        let mut unknown = 0;

        for owned in VALUES {
            for installed in VALUES {
                for license in VALUES {
                    for vac_banned in VALUES {
                        for parental_blocked in VALUES {
                            let verdict =
                                can_play(owned, installed, license, vac_banned, parental_blocked);

                            // Soundness: a Yes requires every input to be definite and
                            // favourable. No `Unknown` can be laundered into permission.
                            if verdict == Ternary::True {
                                assert_eq!(owned, Ternary::True);
                                assert_eq!(installed, Ternary::True);
                                assert_eq!(license, Ternary::True);
                                assert_eq!(vac_banned, Ternary::False);
                                assert_eq!(parental_blocked, Ternary::False);
                                yes += 1;
                            }

                            // A definite no anywhere is a definite no overall.
                            for blocker in [owned, installed, license] {
                                if blocker == Ternary::False {
                                    assert_eq!(verdict, Ternary::False);
                                }
                            }
                            if vac_banned == Ternary::True || parental_blocked == Ternary::True {
                                assert_eq!(verdict, Ternary::False);
                            }

                            // The case this whole module exists for: a favourable but
                            // unconfirmed entitlement may not resolve to Yes.
                            if license == Ternary::Unknown
                                && owned == Ternary::True
                                && installed == Ternary::True
                                && vac_banned == Ternary::False
                                && parental_blocked == Ternary::False
                            {
                                assert_eq!(verdict, Ternary::Unknown);
                            }

                            match verdict {
                                Ternary::True => {}
                                Ternary::False => no += 1,
                                Ternary::Unknown => unknown += 1,
                            }
                        }
                    }
                }
            }
        }

        // Exactly one of the 243 assignments may run the game, and both other answers are
        // reachable -- otherwise the enumeration above proved nothing.
        assert_eq!(yes, 1);
        assert!(no > 0);
        assert!(unknown > 0);
    }
}
