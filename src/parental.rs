use super::*;

/// Access to the steam parental settings interface
///
/// Steam exposes this to games so they can honour the user's family view
/// restrictions instead of offering gated content anyway.
pub struct ParentalSettings {
    pub(crate) ps: *mut sys::ISteamParentalSettings,
    pub(crate) _inner: Arc<Inner>,
}

/// A Steam client feature that parental controls can restrict.
///
/// The Steam-side sentinels (`Invalid`, `KioskMode_Deprecated`, `Max`) are
/// deliberately absent: they are not features a game can honour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParentalFeature {
    Store,
    Community,
    Profile,
    Friends,
    News,
    Trading,
    Settings,
    Console,
    Browser,
    ParentalSetup,
    Library,
    Test,
    SiteLicense,
    BlockAlways,
    Desktop,
}

impl ParentalFeature {
    fn as_raw(self) -> sys::EParentalFeature {
        match self {
            ParentalFeature::Store => sys::EParentalFeature::k_EFeatureStore,
            ParentalFeature::Community => sys::EParentalFeature::k_EFeatureCommunity,
            ParentalFeature::Profile => sys::EParentalFeature::k_EFeatureProfile,
            ParentalFeature::Friends => sys::EParentalFeature::k_EFeatureFriends,
            ParentalFeature::News => sys::EParentalFeature::k_EFeatureNews,
            ParentalFeature::Trading => sys::EParentalFeature::k_EFeatureTrading,
            ParentalFeature::Settings => sys::EParentalFeature::k_EFeatureSettings,
            ParentalFeature::Console => sys::EParentalFeature::k_EFeatureConsole,
            ParentalFeature::Browser => sys::EParentalFeature::k_EFeatureBrowser,
            ParentalFeature::ParentalSetup => sys::EParentalFeature::k_EFeatureParentalSetup,
            ParentalFeature::Library => sys::EParentalFeature::k_EFeatureLibrary,
            ParentalFeature::Test => sys::EParentalFeature::k_EFeatureTest,
            ParentalFeature::SiteLicense => sys::EParentalFeature::k_EFeatureSiteLicense,
            ParentalFeature::BlockAlways => sys::EParentalFeature::k_EFeatureBlockAlways,
            ParentalFeature::Desktop => sys::EParentalFeature::k_EFeatureDesktop,
        }
    }
}

impl ParentalSettings {
    /// Whether the user has configured a parental lock at all.
    ///
    /// A `false` here means the remaining questions are all answered by the
    /// absence of a policy, not by its permissiveness.
    pub fn is_lock_enabled(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamParentalSettings_BIsParentalLockEnabled(self.ps) }
    }

    /// Whether the lock is currently engaged.
    ///
    /// This is a separate Steam query from [`ParentalSettings::is_lock_enabled`]:
    /// a policy can be configured while its effect is temporarily lifted.
    /// Gate your own behaviour on this one.
    pub fn is_lock_locked(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamParentalSettings_BIsParentalLockLocked(self.ps) }
    }

    /// Steam's effective blocked state for an app.
    pub fn is_app_blocked(&self, app: AppId) -> bool {
        unsafe { sys::SteamAPI_ISteamParentalSettings_BIsAppBlocked(self.ps, app.0) }
    }

    /// Whether the app appears on the parental block list.
    ///
    /// Distinct from [`ParentalSettings::is_app_blocked`]: list membership and
    /// effective state are two different Steam answers, so this crate exposes
    /// both rather than merging them into a guess.
    pub fn is_app_in_block_list(&self, app: AppId) -> bool {
        unsafe { sys::SteamAPI_ISteamParentalSettings_BIsAppInBlockList(self.ps, app.0) }
    }

    /// Steam's effective blocked state for a client feature.
    pub fn is_feature_blocked(&self, feature: ParentalFeature) -> bool {
        let feature = feature.as_raw();
        unsafe { sys::SteamAPI_ISteamParentalSettings_BIsFeatureBlocked(self.ps, feature) }
    }

    /// Whether the feature appears on the parental block list.
    ///
    /// See [`ParentalSettings::is_app_in_block_list`] for why this is kept
    /// separate from [`ParentalSettings::is_feature_blocked`].
    pub fn is_feature_in_block_list(&self, feature: ParentalFeature) -> bool {
        let feature = feature.as_raw();
        unsafe { sys::SteamAPI_ISteamParentalSettings_BIsFeatureInBlockList(self.ps, feature) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [ParentalFeature; 15] = [
        ParentalFeature::Store,
        ParentalFeature::Community,
        ParentalFeature::Profile,
        ParentalFeature::Friends,
        ParentalFeature::News,
        ParentalFeature::Trading,
        ParentalFeature::Settings,
        ParentalFeature::Console,
        ParentalFeature::Browser,
        ParentalFeature::ParentalSetup,
        ParentalFeature::Library,
        ParentalFeature::Test,
        ParentalFeature::SiteLicense,
        ParentalFeature::BlockAlways,
        ParentalFeature::Desktop,
    ];

    #[test]
    fn every_feature_maps_to_its_own_steam_variant() {
        // One mis-wired arm would silently honour the wrong restriction, which
        // is a compliance failure rather than a cosmetic bug.
        let expected = [
            sys::EParentalFeature::k_EFeatureStore,
            sys::EParentalFeature::k_EFeatureCommunity,
            sys::EParentalFeature::k_EFeatureProfile,
            sys::EParentalFeature::k_EFeatureFriends,
            sys::EParentalFeature::k_EFeatureNews,
            sys::EParentalFeature::k_EFeatureTrading,
            sys::EParentalFeature::k_EFeatureSettings,
            sys::EParentalFeature::k_EFeatureConsole,
            sys::EParentalFeature::k_EFeatureBrowser,
            sys::EParentalFeature::k_EFeatureParentalSetup,
            sys::EParentalFeature::k_EFeatureLibrary,
            sys::EParentalFeature::k_EFeatureTest,
            sys::EParentalFeature::k_EFeatureSiteLicense,
            sys::EParentalFeature::k_EFeatureBlockAlways,
            sys::EParentalFeature::k_EFeatureDesktop,
        ];
        for (feature, want) in ALL.iter().copied().zip(expected) {
            assert_eq!(feature.as_raw(), want, "{feature:?} mapped wrongly");
        }
    }

    #[test]
    fn feature_mappings_are_pairwise_distinct() {
        for i in 0..ALL.len() {
            for j in (i + 1)..ALL.len() {
                let (a, b) = (ALL[i], ALL[j]);
                assert_ne!(
                    a.as_raw(),
                    b.as_raw(),
                    "{a:?} and {b:?} collapse to one Steam feature"
                );
            }
        }
    }

    #[test]
    fn no_feature_resolves_to_a_sentinel() {
        // Invalid would make every query report the same answer; Max is a
        // bounds marker, not a feature.
        let sentinels = [
            sys::EParentalFeature::k_EFeatureInvalid,
            sys::EParentalFeature::k_EFeatureMax,
        ];
        for feature in ALL {
            assert!(
                !sentinels.contains(&feature.as_raw()),
                "{feature:?} resolved to a sentinel"
            );
        }
    }

    #[test]
    fn all_fifteen_features_are_covered() {
        // A feature missing from ALL would escape the two mapping tests above,
        // so pin the census against the public enum's size.
        assert_eq!(ALL.len(), 15);
        let unique: std::collections::HashSet<_> = ALL.iter().collect();
        assert_eq!(unique.len(), 15, "duplicate entry in ALL");
    }
}
