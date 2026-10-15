//! The filter vocabulary a lobby-list query is described with: keys, string / number /
//! near filters, the distance and comparison enums, and the length rule Steam imposes on a
//! lobby data key.

use super::*;
use std::fmt::{Display, Formatter};

/// Filters for the lobbies to be returned from `request_lobby_list`.
///
/// This struct is designed to be used as part of the filtering process
/// when calling the [`set_lobby_list_filter`](Matchmaking::set_lobby_list_filter) method.
///
/// # Fields
///
/// - `string`: A string comparison filter that matches lobby attributes with specific strings.
/// - `number`: A number comparison filter that matches lobby attributes with specific integer values.
/// - `near_value`: Specifies a value, and the results will be sorted closest to this value (no actual filtering).
/// - `open_slots`: Filters lobbies based on the number of open slots they have.
/// - `distance`: Filters lobbies based on a distance criterion.
/// - `count`: Specifies the maximum number of lobby results to be returned.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LobbyListFilter<'a> {
    /// A string comparison filter that matches lobby attributes with specific strings.
    //#[cfg_attr(feature = "serde", serde(borrow))]
    pub string: Option<StringFilters<'a>>,
    /// A number comparison filter that matches lobby attributes with specific integer values
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub number: Option<NumberFilters<'a>>,
    /// Specifies a value, and the results will be sorted closest to this value (no actual filtering)
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub near_value: Option<NearFilters<'a>>,
    /// Filters lobbies based on the number of open slots they have
    pub open_slots: Option<u8>,
    /// Filters lobbies based on a distance criterion
    pub distance: Option<DistanceFilter>,
    /// Specifies the maximum number of lobby results to be returned
    pub count: Option<u64>,
}

/// A wrapper for a lobby key string.
///
/// This struct provides a wrapper for a lobby key string. It is used to validate
/// constructed keys and to ensure that they do not exceed the maximum allowed length.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LobbyKey<'a>(pub(crate) &'a str);

impl<'a> std::ops::Deref for LobbyKey<'a> {
    type Target = &'a str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Error)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LobbyKeyTooLongError;

impl Display for LobbyKeyTooLongError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Lobby key is greater than {} characters",
            sys::k_nMaxLobbyKeyLength
        )
    }
}

impl<'a> LobbyKey<'a> {
    /// Attempts to create a new `LobbyKey` from a provided string key.
    ///
    /// # Arguments
    ///
    /// * `key`: The string key to create a `LobbyKey` from.
    ///
    /// # Errors
    ///
    /// This function will return an error of type [`LobbyKeyTooLongError`] if the provided key's length
    /// exceeds k_nMaxLobbyKeyLength (255 characters).
    pub fn try_new(key: &'a str) -> Result<Self, LobbyKeyTooLongError> {
        if key.len() > sys::k_nMaxLobbyKeyLength as usize {
            Err(LobbyKeyTooLongError)
        } else {
            Ok(LobbyKey(key))
        }
    }
    /// Creates a new `LobbyKey` from a provided string key.
    ///
    /// # Arguments
    ///
    /// * `key`: The string key to create a `LobbyKey` from.
    ///
    /// # Panics
    ///
    /// This function will panic if the provided key's length exceeds 255 characters.
    pub fn new(key: &'a str) -> Self {
        Self::try_new(key).unwrap()
    }
}

pub type StringFilters<'a> = Vec<StringFilter<'a>>;

pub type NumberFilters<'a> = Vec<NumberFilter<'a>>;

pub type NearFilters<'a> = Vec<NearFilter<'a>>;

/// A filter used for string based key value comparisons.
///
/// # Fields
///
/// * `0`: The attribute key for comparison.
/// * `1`: The target string value for matching.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StringFilter<'a>(
    #[cfg_attr(feature = "serde", serde(borrow))] pub LobbyKey<'a>,
    pub &'a str,
    pub StringFilterKind,
);

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum StringFilterKind {
    #[default]
    EqualToOrLessThan,
    LessThan,
    Equal,
    GreaterThan,
    EqualToOrGreaterThan,
    NotEqual,
}

impl From<StringFilterKind> for sys::ELobbyComparison {
    fn from(filter: StringFilterKind) -> Self {
        match filter {
            StringFilterKind::EqualToOrLessThan => {
                sys::ELobbyComparison::k_ELobbyComparisonEqualToOrLessThan
            }
            StringFilterKind::LessThan => sys::ELobbyComparison::k_ELobbyComparisonLessThan,
            StringFilterKind::Equal => sys::ELobbyComparison::k_ELobbyComparisonEqual,
            StringFilterKind::GreaterThan => sys::ELobbyComparison::k_ELobbyComparisonGreaterThan,
            StringFilterKind::EqualToOrGreaterThan => {
                sys::ELobbyComparison::k_ELobbyComparisonEqualToOrGreaterThan
            }
            StringFilterKind::NotEqual => sys::ELobbyComparison::k_ELobbyComparisonNotEqual,
        }
    }
}

/// A filter used for numerical attribute comparison in lobby filtering.
///
/// # Fields
///
/// * `key`: The attribute key for comparison.
/// * `value`: The target numerical value for matching.
/// * `comparison`: The comparison mode indicating how the numerical values should be compared.
///
/// # Example
///
/// ```no_run
/// # use steamworks::*;
/// let elo_filter = NumberFilter(
///     LobbyKey::new("lobby_elo"),
///     1500,
///     ComparisonFilter::GreaterThan,
/// );
/// ```
///
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NumberFilter<'a>(
    #[cfg_attr(feature = "serde", serde(borrow))] pub LobbyKey<'a>,
    pub i32,
    pub ComparisonFilter,
);

/// A filter used for near-value sorting in lobby filtering.
///
/// This struct enables sorting the lobby results based on their closeness to a reference value.
/// It includes two fields: the attribute key to use for sorting and the reference numerical value.
///
/// This filter does not perform actual filtering but rather sorts the results based on proximity.
///
/// # Fields
///
/// * `0`: The attribute key to use for sorting.
/// * `1`: The reference numerical value used for sorting proximity.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NearFilter<'a>(
    #[cfg_attr(feature = "serde", serde(borrow))] pub LobbyKey<'a>,
    pub i32,
);

impl<'a> LobbyListFilter<'a> {
    /// Sets the string comparison filter for the lobby list filter.
    ///
    /// # Arguments
    ///
    /// * `string`: A tuple containing the attribute name and the target string value to match.
    ///
    pub fn set_string(mut self, string: Option<StringFilters<'a>>) -> Self {
        self.string = string;
        self
    }

    /// Sets the number comparison filter for the lobby list filter.
    ///
    /// # Arguments
    ///
    /// * `number`: A tuple containing the attribute name and the target integer value to match.
    ///
    pub fn set_number(mut self, number: Option<NumberFilters<'a>>) -> Self {
        self.number = number;
        self
    }

    /// Sets the near value filter for the lobby list filter.
    ///
    /// # Arguments
    ///
    /// * `near_value`: A tuple containing the attribute name and the reference integer value.
    ///                 Lobby results will be sorted based on their closeness to this value.
    ///
    pub fn set_near_value(mut self, near_value: Option<NearFilters<'a>>) -> Self {
        self.near_value = near_value;
        self
    }

    /// Sets the open slots filter for the lobby list filter.
    ///
    /// # Arguments
    ///
    /// * `open_slots`: The number of open slots to filter lobbies by.
    ///
    pub fn set_open_slots(mut self, open_slots: Option<u8>) -> Self {
        self.open_slots = open_slots;
        self
    }

    /// Sets the distance filter for the lobby list filter.
    ///
    /// # Arguments
    ///
    /// * `distance`: A distance filter that specifies a distance criterion for filtering lobbies.
    ///
    pub fn set_distance(mut self, distance: Option<DistanceFilter>) -> Self {
        self.distance = distance;
        self
    }

    /// Sets the maximum number of lobby results to be returned.
    ///
    /// # Arguments
    ///
    /// * `count`: The maximum number of lobby results to retrieve.
    ///
    pub fn set_count(mut self, count: Option<u64>) -> Self {
        self.count = count;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DistanceFilter {
    Close,
    #[default]
    Default,
    Far,
    Worldwide,
}

impl From<DistanceFilter> for sys::ELobbyDistanceFilter {
    fn from(filter: DistanceFilter) -> Self {
        match filter {
            DistanceFilter::Close => sys::ELobbyDistanceFilter::k_ELobbyDistanceFilterClose,
            DistanceFilter::Default => sys::ELobbyDistanceFilter::k_ELobbyDistanceFilterDefault,
            DistanceFilter::Far => sys::ELobbyDistanceFilter::k_ELobbyDistanceFilterFar,
            DistanceFilter::Worldwide => sys::ELobbyDistanceFilter::k_ELobbyDistanceFilterWorldwide,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ComparisonFilter {
    #[default]
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanEqualTo,
    LessThan,
    LessThanEqualTo,
}

impl From<ComparisonFilter> for sys::ELobbyComparison {
    fn from(filter: ComparisonFilter) -> Self {
        match filter {
            ComparisonFilter::Equal => sys::ELobbyComparison::k_ELobbyComparisonEqual,
            ComparisonFilter::NotEqual => sys::ELobbyComparison::k_ELobbyComparisonNotEqual,
            ComparisonFilter::GreaterThan => sys::ELobbyComparison::k_ELobbyComparisonGreaterThan,
            ComparisonFilter::GreaterThanEqualTo => {
                sys::ELobbyComparison::k_ELobbyComparisonEqualToOrGreaterThan
            }
            ComparisonFilter::LessThan => sys::ELobbyComparison::k_ELobbyComparisonLessThan,
            ComparisonFilter::LessThanEqualTo => {
                sys::ELobbyComparison::k_ELobbyComparisonEqualToOrLessThan
            }
        }
    }
}
