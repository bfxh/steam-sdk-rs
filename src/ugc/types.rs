//! Value types the workshop surface hands back, and their Steam-side mappings.
//!
//! Split out of the old single `ugc.rs`: these are the enums and small structs whose
//! only job is to say what Steam means (`FileType`, `UGCType`, `ItemState`, ...).
//! Keeping them here is what stops the query/update builders from growing a fourth
//! reason to live in one file.

use crate::*;

pub const RESULTS_PER_PAGE: u32 = sys::kNumUGCResultsPerPage as u32;

pub struct UGC {
    pub(crate) ugc: *mut sys::ISteamUGC,
    pub(crate) inner: Arc<Inner>,
}

// TODO: should come from sys, but I don't think its generated.
/// Workshop item ID
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PublishedFileId(pub u64);
impl From<u64> for PublishedFileId {
    fn from(id: u64) -> Self {
        PublishedFileId(id)
    }
}

/// Workshop item types to search for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UGCType {
    Items,
    ItemsMtx,
    ItemsReadyToUse,
    Collections,
    Artwork,
    Videos,
    Screenshots,
    AllGuides,
    WebGuides,
    IntegratedGuides,
    UsableInGame,
    ControllerBindings,
    GameManagedItems,
    All,
}
impl Into<sys::EUGCMatchingUGCType> for UGCType {
    fn into(self) -> sys::EUGCMatchingUGCType {
        match self {
            UGCType::Items => sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_Items,
            UGCType::ItemsMtx => sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_Items_Mtx,
            UGCType::ItemsReadyToUse => {
                sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_Items_ReadyToUse
            }
            UGCType::Collections => sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_Collections,
            UGCType::Artwork => sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_Artwork,
            UGCType::Videos => sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_Videos,
            UGCType::Screenshots => sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_Screenshots,
            UGCType::AllGuides => sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_AllGuides,
            UGCType::WebGuides => sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_WebGuides,
            UGCType::IntegratedGuides => {
                sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_IntegratedGuides
            }
            UGCType::UsableInGame => sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_UsableInGame,
            UGCType::ControllerBindings => {
                sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_ControllerBindings
            }
            UGCType::GameManagedItems => {
                sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_GameManagedItems
            }
            UGCType::All => sys::EUGCMatchingUGCType::k_EUGCMatchingUGCType_All,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UGCQueryType {
    RankedByVote,
    RankedByPublicationDate,
    AcceptedForGameRankedByAcceptanceDate,
    RankedByTrend,
    FavoritedByFriendsRankedByPublicationDate,
    CreatedByFriendsRankedByPublicationDate,
    RankedByNumTimesReported,
    CreatedByFollowedUsersRankedByPublicationDate,
    NotYetRated,
    RankedByTotalVotesAsc,
    RankedByVotesUp,
    RankedByTextSearch,
    RankedByTotalUniqueSubscriptions,
    RankedByPlaytimeTrend,
    RankedByTotalPlaytime,
    RankedByAveragePlaytimeTrend,
    RankedByLifetimeAveragePlaytime,
    RankedByPlaytimeSessionsTrend,
    RankedByLifetimePlaytimeSessions,
    RankedByLastUpdatedDate,
}
impl Into<sys::EUGCQuery> for UGCQueryType {
    fn into(self) -> sys::EUGCQuery {
        match self {
            UGCQueryType::RankedByVote => sys::EUGCQuery::k_EUGCQuery_RankedByVote,
            UGCQueryType::RankedByPublicationDate => {
                sys::EUGCQuery::k_EUGCQuery_RankedByPublicationDate
            }
            UGCQueryType::AcceptedForGameRankedByAcceptanceDate => {
                sys::EUGCQuery::k_EUGCQuery_AcceptedForGameRankedByAcceptanceDate
            }
            UGCQueryType::RankedByTrend => sys::EUGCQuery::k_EUGCQuery_RankedByTrend,
            UGCQueryType::FavoritedByFriendsRankedByPublicationDate => {
                sys::EUGCQuery::k_EUGCQuery_FavoritedByFriendsRankedByPublicationDate
            }
            UGCQueryType::CreatedByFriendsRankedByPublicationDate => {
                sys::EUGCQuery::k_EUGCQuery_CreatedByFriendsRankedByPublicationDate
            }
            UGCQueryType::RankedByNumTimesReported => {
                sys::EUGCQuery::k_EUGCQuery_RankedByNumTimesReported
            }
            UGCQueryType::CreatedByFollowedUsersRankedByPublicationDate => {
                sys::EUGCQuery::k_EUGCQuery_CreatedByFollowedUsersRankedByPublicationDate
            }
            UGCQueryType::NotYetRated => sys::EUGCQuery::k_EUGCQuery_NotYetRated,
            UGCQueryType::RankedByTotalVotesAsc => {
                sys::EUGCQuery::k_EUGCQuery_RankedByTotalVotesAsc
            }
            UGCQueryType::RankedByVotesUp => sys::EUGCQuery::k_EUGCQuery_RankedByVotesUp,
            UGCQueryType::RankedByTextSearch => sys::EUGCQuery::k_EUGCQuery_RankedByTextSearch,
            UGCQueryType::RankedByTotalUniqueSubscriptions => {
                sys::EUGCQuery::k_EUGCQuery_RankedByTotalUniqueSubscriptions
            }
            UGCQueryType::RankedByPlaytimeTrend => {
                sys::EUGCQuery::k_EUGCQuery_RankedByPlaytimeTrend
            }
            UGCQueryType::RankedByTotalPlaytime => {
                sys::EUGCQuery::k_EUGCQuery_RankedByTotalPlaytime
            }
            UGCQueryType::RankedByAveragePlaytimeTrend => {
                sys::EUGCQuery::k_EUGCQuery_RankedByAveragePlaytimeTrend
            }
            UGCQueryType::RankedByLifetimeAveragePlaytime => {
                sys::EUGCQuery::k_EUGCQuery_RankedByLifetimeAveragePlaytime
            }
            UGCQueryType::RankedByPlaytimeSessionsTrend => {
                sys::EUGCQuery::k_EUGCQuery_RankedByPlaytimeSessionsTrend
            }
            UGCQueryType::RankedByLifetimePlaytimeSessions => {
                sys::EUGCQuery::k_EUGCQuery_RankedByLifetimePlaytimeSessions
            }
            UGCQueryType::RankedByLastUpdatedDate => {
                sys::EUGCQuery::k_EUGCQuery_RankedByLastUpdatedDate
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Community,
    Microtransaction,
    Collection,
    Art,
    Video,
    Screenshot,
    Game,
    Software,
    Concept,
    WebGuide,
    IntegratedGuide,
    Merch,
    ControllerBinding,
    SteamworksAccessInvite,
    SteamVideo,
    GameManagedItem,
    Clip,

    /// A Workshop file type this binding does not name.
    ///
    /// Reachable today, not hypothetically: `k_EWorkshopFileTypeMax` (17) is declared by the
    /// vendored headers and named by neither this enum nor the mapping above, so a Steam answer
    /// using it landed in `unreachable!()` -- a panic inside the caller while reading an item.
    /// `k_EWorkshopFileTypeFirst` is not a separate case: it shares 0 with `Community`.
    ///
    /// Writing it back sends `k_EWorkshopFileTypeFirst`, which Steam does not accept as a real
    /// type, so `create_item` fails with Steam's own error instead of this wrapper inventing one.
    Unknown,
}

impl Into<sys::EWorkshopFileType> for FileType {
    fn into(self) -> sys::EWorkshopFileType {
        match self {
            FileType::Community => sys::EWorkshopFileType::k_EWorkshopFileTypeCommunity,
            FileType::Microtransaction => {
                sys::EWorkshopFileType::k_EWorkshopFileTypeMicrotransaction
            }
            FileType::Collection => sys::EWorkshopFileType::k_EWorkshopFileTypeCollection,
            FileType::Art => sys::EWorkshopFileType::k_EWorkshopFileTypeArt,
            FileType::Video => sys::EWorkshopFileType::k_EWorkshopFileTypeVideo,
            FileType::Screenshot => sys::EWorkshopFileType::k_EWorkshopFileTypeScreenshot,
            FileType::Game => sys::EWorkshopFileType::k_EWorkshopFileTypeGame,
            FileType::Software => sys::EWorkshopFileType::k_EWorkshopFileTypeSoftware,
            FileType::Concept => sys::EWorkshopFileType::k_EWorkshopFileTypeConcept,
            FileType::WebGuide => sys::EWorkshopFileType::k_EWorkshopFileTypeWebGuide,
            FileType::IntegratedGuide => sys::EWorkshopFileType::k_EWorkshopFileTypeIntegratedGuide,
            FileType::Merch => sys::EWorkshopFileType::k_EWorkshopFileTypeMerch,
            FileType::ControllerBinding => {
                sys::EWorkshopFileType::k_EWorkshopFileTypeControllerBinding
            }
            FileType::SteamworksAccessInvite => {
                sys::EWorkshopFileType::k_EWorkshopFileTypeSteamworksAccessInvite
            }
            FileType::SteamVideo => sys::EWorkshopFileType::k_EWorkshopFileTypeSteamVideo,
            FileType::GameManagedItem => sys::EWorkshopFileType::k_EWorkshopFileTypeGameManagedItem,
            FileType::Clip => sys::EWorkshopFileType::k_EWorkshopFileTypeClip,
            // Sending a read-only `Unknown` back is refused by deferring: this passes Steam's
            // own out-of-range sentinel, so `create_item` fails at Steam. The tempting
            // alternative -- `k_EWorkshopFileTypeFirst`, which shares 0 with `Community` --
            // would silently create an item of the wrong type.
            FileType::Unknown => sys::EWorkshopFileType::k_EWorkshopFileTypeMax,
        }
    }
}
impl From<sys::EWorkshopFileType> for FileType {
    fn from(file_type: sys::EWorkshopFileType) -> FileType {
        match file_type {
            sys::EWorkshopFileType::k_EWorkshopFileTypeCommunity => FileType::Community,
            sys::EWorkshopFileType::k_EWorkshopFileTypeMicrotransaction => {
                FileType::Microtransaction
            }
            sys::EWorkshopFileType::k_EWorkshopFileTypeCollection => FileType::Collection,
            sys::EWorkshopFileType::k_EWorkshopFileTypeArt => FileType::Art,
            sys::EWorkshopFileType::k_EWorkshopFileTypeVideo => FileType::Video,
            sys::EWorkshopFileType::k_EWorkshopFileTypeScreenshot => FileType::Screenshot,
            sys::EWorkshopFileType::k_EWorkshopFileTypeGame => FileType::Game,
            sys::EWorkshopFileType::k_EWorkshopFileTypeSoftware => FileType::Software,
            sys::EWorkshopFileType::k_EWorkshopFileTypeConcept => FileType::Concept,
            sys::EWorkshopFileType::k_EWorkshopFileTypeWebGuide => FileType::WebGuide,
            sys::EWorkshopFileType::k_EWorkshopFileTypeIntegratedGuide => FileType::IntegratedGuide,
            sys::EWorkshopFileType::k_EWorkshopFileTypeMerch => FileType::Merch,
            sys::EWorkshopFileType::k_EWorkshopFileTypeControllerBinding => {
                FileType::ControllerBinding
            }
            sys::EWorkshopFileType::k_EWorkshopFileTypeSteamworksAccessInvite => {
                FileType::SteamworksAccessInvite
            }
            sys::EWorkshopFileType::k_EWorkshopFileTypeSteamVideo => FileType::SteamVideo,
            sys::EWorkshopFileType::k_EWorkshopFileTypeGameManagedItem => FileType::GameManagedItem,
            sys::EWorkshopFileType::k_EWorkshopFileTypeClip => FileType::Clip,
            _ => FileType::Unknown,
        }
    }
}

/// AppID filter for queries.
/// The "consumer" app is the app that the content is for.
/// The "creator" app is a separate editor to create the content in, if applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppIDs {
    CreatorAppId(AppId),
    ConsumerAppId(AppId),
    Both { creator: AppId, consumer: AppId },
}

impl AppIDs {
    pub fn creator_app_id(&self) -> Option<AppId> {
        match self {
            AppIDs::CreatorAppId(v) => Some(*v),
            AppIDs::ConsumerAppId(_) => None,
            AppIDs::Both { creator, .. } => Some(*creator),
        }
    }
    pub fn consumer_app_id(&self) -> Option<AppId> {
        match self {
            AppIDs::CreatorAppId(_) => None,
            AppIDs::ConsumerAppId(v) => Some(*v),
            AppIDs::Both { consumer, .. } => Some(*consumer),
        }
    }
}

/// Query result sorting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserListOrder {
    CreationOrderAsc,
    CreationOrderDesc,
    TitleAsc,
    LastUpdatedDesc,
    SubscriptionDateDesc,
    VoteScoreDesc,
    ForModeration,
}

impl Into<sys::EUserUGCListSortOrder> for UserListOrder {
    fn into(self) -> sys::EUserUGCListSortOrder {
        match self {
            UserListOrder::CreationOrderAsc => {
                sys::EUserUGCListSortOrder::k_EUserUGCListSortOrder_CreationOrderAsc
            }
            UserListOrder::CreationOrderDesc => {
                sys::EUserUGCListSortOrder::k_EUserUGCListSortOrder_CreationOrderDesc
            }
            UserListOrder::TitleAsc => sys::EUserUGCListSortOrder::k_EUserUGCListSortOrder_TitleAsc,
            UserListOrder::LastUpdatedDesc => {
                sys::EUserUGCListSortOrder::k_EUserUGCListSortOrder_LastUpdatedDesc
            }
            UserListOrder::SubscriptionDateDesc => {
                sys::EUserUGCListSortOrder::k_EUserUGCListSortOrder_SubscriptionDateDesc
            }
            UserListOrder::VoteScoreDesc => {
                sys::EUserUGCListSortOrder::k_EUserUGCListSortOrder_VoteScoreDesc
            }
            UserListOrder::ForModeration => {
                sys::EUserUGCListSortOrder::k_EUserUGCListSortOrder_ForModeration
            }
        }
    }
}

/// Available user-specific lists.
/// Certain ones are only available to the currently logged in user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserList {
    /// Files user has published
    Published,
    /// Files user has voted on
    VotedOn,
    /// Files user has voted up (current user only)
    VotedUp,
    /// Files user has voted down (current user only)
    VotedDown,
    /// Deprecated
    #[deprecated(note = "Deprecated in Steam API")]
    WillVoteLater,
    /// Files user has favorited
    Favorited,
    /// Files user has subscribed to (current user only)
    Subscribed,
    /// Files user has spent in-game time with
    UsedOrPlayed,
    /// Files user is following updates for
    Followed,
}
impl Into<sys::EUserUGCList> for UserList {
    #[allow(deprecated)]
    fn into(self) -> sys::EUserUGCList {
        match self {
            UserList::Published => sys::EUserUGCList::k_EUserUGCList_Published,
            UserList::VotedOn => sys::EUserUGCList::k_EUserUGCList_VotedOn,
            UserList::VotedUp => sys::EUserUGCList::k_EUserUGCList_VotedUp,
            UserList::VotedDown => sys::EUserUGCList::k_EUserUGCList_VotedDown,
            UserList::WillVoteLater => sys::EUserUGCList::k_EUserUGCList_WillVoteLater,
            UserList::Favorited => sys::EUserUGCList::k_EUserUGCList_Favorited,
            UserList::Subscribed => sys::EUserUGCList::k_EUserUGCList_Subscribed,
            UserList::UsedOrPlayed => sys::EUserUGCList::k_EUserUGCList_UsedOrPlayed,
            UserList::Followed => sys::EUserUGCList::k_EUserUGCList_Followed,
        }
    }
}

/// Available published item statistic types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UGCStatisticType {
    /// The number of subscriptions.
    Subscriptions,
    /// The number of favorites.
    Favorites,
    /// The number of followers.
    Followers,
    /// The number of unique subscriptions.
    UniqueSubscriptions,
    /// The number of unique favorites.
    UniqueFavorites,
    /// The number of unique followers.
    UniqueFollowers,
    /// The number of unique views the item has on its Steam Workshop page.
    UniqueWebsiteViews,
    /// The number of times the item has been reported.
    Reports,
    /// The total number of seconds this item has been used across all players.
    SecondsPlayed,
    /// The total number of play sessions this item has been used in.
    PlaytimeSessions,
    /// The number of comments on the items steam has on its Steam Workshop page.
    Comments,
    /// The number of seconds this item has been used over the given time period.
    SecondsPlayedDuringTimePeriod,
    /// The number of sessions this item has been used in over the given time period.
    PlaytimeSessionsDuringTimePeriod,
}
impl Into<sys::EItemStatistic> for UGCStatisticType {
    fn into(self) -> sys::EItemStatistic {
        match self {
            UGCStatisticType::Subscriptions => {
                sys::EItemStatistic::k_EItemStatistic_NumSubscriptions
            }
            UGCStatisticType::Favorites => sys::EItemStatistic::k_EItemStatistic_NumFavorites,
            UGCStatisticType::Followers => sys::EItemStatistic::k_EItemStatistic_NumFollowers,
            UGCStatisticType::UniqueSubscriptions => {
                sys::EItemStatistic::k_EItemStatistic_NumUniqueSubscriptions
            }
            UGCStatisticType::UniqueFavorites => {
                sys::EItemStatistic::k_EItemStatistic_NumUniqueFavorites
            }
            UGCStatisticType::UniqueFollowers => {
                sys::EItemStatistic::k_EItemStatistic_NumUniqueFollowers
            }
            UGCStatisticType::UniqueWebsiteViews => {
                sys::EItemStatistic::k_EItemStatistic_NumUniqueWebsiteViews
            }
            UGCStatisticType::Reports => sys::EItemStatistic::k_EItemStatistic_ReportScore,
            UGCStatisticType::SecondsPlayed => {
                sys::EItemStatistic::k_EItemStatistic_NumSecondsPlayed
            }
            UGCStatisticType::PlaytimeSessions => {
                sys::EItemStatistic::k_EItemStatistic_NumPlaytimeSessions
            }
            UGCStatisticType::Comments => sys::EItemStatistic::k_EItemStatistic_NumComments,
            UGCStatisticType::SecondsPlayedDuringTimePeriod => {
                sys::EItemStatistic::k_EItemStatistic_NumSecondsPlayedDuringTimePeriod
            }
            UGCStatisticType::PlaytimeSessionsDuringTimePeriod => {
                sys::EItemStatistic::k_EItemStatistic_NumPlaytimeSessionsDuringTimePeriod
            }
        }
    }
}

bitflags! {
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct ItemState: u32 {
        const NONE = 0;
        const SUBSCRIBED = 1;
        const LEGACY_ITEM = 2;
        const INSTALLED = 4;
        const NEEDS_UPDATE = 8;
        const DOWNLOADING = 16;
        const DOWNLOAD_PENDING = 32;
    }
}

/// Users can control what user-generated content they want to see under the Mature Content Filtering section in their preferences.
/// This filtering is done automatically by Steam servers, but first, user-generated content must be tagged appropriately.
/// Developers can use AddContentDescriptor and RemoveContentDescriptor calls to manage content descriptors a piece of UGC has.
/// These can be retrieved from the result of a query via GetQueryUGCContentDescriptors.
pub enum UGCContentDescriptorID {
    /// Some Nudity or Sexual Content: Contains content that has some nudity or sexual themes, but not as the primary focus.
    NudityOrSexualContent = 1,
    /// Frequent Violence or Gore: Contains content that features extreme violence or gore.
    FrequentViolenceOrGore = 2,
    /// Adult Only Sexual Content: Contains content that is sexually explicit or graphic and is intended for adults only. Users must affirm that they are at least eighteen years old before they can view content with this content descriptor.
    AdultOnlySexualContent = 3,
    /// Frequent Nudity or Sexual Content: Contains content that primarily features nudity or sexual themes. Users must affirm that they are at least eighteen years old before they can view content with this content descriptor.
    GratuitousSexualContent = 4,
    /// General Mature Content: Contains mature topics that may not be appropriate for all audiences.
    AnyMatureContent = 5,
}
impl Into<sys::EUGCContentDescriptorID> for UGCContentDescriptorID {
    fn into(self) -> sys::EUGCContentDescriptorID {
        match self {
            UGCContentDescriptorID::NudityOrSexualContent => {
                sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_NudityOrSexualContent
            }
            UGCContentDescriptorID::FrequentViolenceOrGore => {
                sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_FrequentViolenceOrGore
            }
            UGCContentDescriptorID::AdultOnlySexualContent => {
                sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_AdultOnlySexualContent
            }
            UGCContentDescriptorID::GratuitousSexualContent => {
                sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_GratuitousSexualContent
            }
            UGCContentDescriptorID::AnyMatureContent => {
                sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_AnyMatureContent
            }
        }
    }
}
impl From<sys::EUGCContentDescriptorID> for UGCContentDescriptorID {
    fn from(content_descriptor_id: sys::EUGCContentDescriptorID) -> UGCContentDescriptorID {
        match content_descriptor_id {
            sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_NudityOrSexualContent => {
                UGCContentDescriptorID::NudityOrSexualContent
            }
            sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_FrequentViolenceOrGore => {
                UGCContentDescriptorID::FrequentViolenceOrGore
            }
            sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_AdultOnlySexualContent => {
                UGCContentDescriptorID::AdultOnlySexualContent
            }
            sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_GratuitousSexualContent => {
                UGCContentDescriptorID::GratuitousSexualContent
            }
            sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_AnyMatureContent => {
                UGCContentDescriptorID::AnyMatureContent
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DownloadItemResult {
    pub app_id: AppId,
    pub published_file_id: PublishedFileId,
    pub error: Option<SteamError>,
}

impl_callback!(cb: DownloadItemResult_t => DownloadItemResult {
    Self {
        app_id: AppId(cb.m_unAppID),
        published_file_id: PublishedFileId(cb.m_nPublishedFileId),
        error: to_steam_result(cb.m_eResult).err(),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct DeleteItemResult {
    pub published_file_id: PublishedFileId,
    pub error: Option<SteamError>,
}

impl_callback!(cb: DeleteItemResult_t => DeleteItemResult {
    Self {
        published_file_id: PublishedFileId(cb.m_nPublishedFileId),
        error: to_steam_result(cb.m_eResult).err(),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InstallInfo {
    pub folder: String,
    pub size_on_disk: u64,
    pub timestamp: u32,
}
