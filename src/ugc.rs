use super::*;

use std::error;
use std::ffi::{CStr, CString};
use std::fmt;
use std::marker;
use std::mem;
use std::os::raw::c_char;
use std::path::Path;

pub const RESULTS_PER_PAGE: u32 = sys::kNumUGCResultsPerPage as u32;

pub struct UGC {
    pub(crate) ugc: *mut sys::ISteamUGC,
    pub(crate) inner: Arc<Inner>,
}

// TODO: should come from sys, but I don't think its generated.
#[allow(non_upper_case_globals)]
const UGCQueryHandleInvalid: u64 = 0xffffffffffffffff;

/// Worshop item ID
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
            _ => unreachable!(),
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

impl UGC {
    /// Suspends or resumes all workshop downloads
    pub fn suspend_downloads(&self, suspend: bool) {
        unsafe {
            sys::SteamAPI_ISteamUGC_SuspendDownloads(self.ugc, suspend);
        }
    }

    /// Creates a workshop item
    pub fn create_item<F>(&self, app_id: AppId, file_type: FileType, cb: F)
    where
        F: FnOnce(Result<(PublishedFileId, bool), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_CreateItem(self.ugc, app_id.0, file_type.into());
            register_call_result::<sys::CreateItemResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        crate::to_steam_result(v.m_eResult).map(|_| {
                            (
                                PublishedFileId(v.m_nPublishedFileId),
                                v.m_bUserNeedsToAcceptWorkshopLegalAgreement,
                            )
                        })
                    })
                },
            );
        }
    }

    /// Starts an item update process
    #[must_use]
    pub fn start_item_update(&self, app_id: AppId, file_id: PublishedFileId) -> UpdateHandle {
        unsafe {
            let handle = sys::SteamAPI_ISteamUGC_StartItemUpdate(self.ugc, app_id.0, file_id.0);
            UpdateHandle {
                ugc: self.ugc,
                inner: self.inner.clone(),

                handle,
            }
        }
    }

    /// Starts an item update process using the panic-free
    /// [`SafeUgcUpdate`] builder.
    ///
    /// Each setter on the returned value validates its input and returns
    /// `Result` instead of panicking on invalid paths or interior nul bytes.
    #[must_use]
    pub fn safe_item_update(&self, app_id: AppId, file_id: PublishedFileId) -> SafeUgcUpdate {
        SafeUgcUpdate::new(self.start_item_update(app_id, file_id))
    }

    /// Subscribes to a workshop item
    pub fn subscribe_item<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_SubscribeItem(self.ugc, published_file_id.0);
            register_call_result::<sys::RemoteStorageSubscribePublishedFileResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        crate::to_steam_result(v.m_eResult)
                    })
                },
            );
        }
    }

    pub fn unsubscribe_item<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_UnsubscribeItem(self.ugc, published_file_id.0);
            register_call_result::<sys::RemoteStorageUnsubscribePublishedFileResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        crate::to_steam_result(v.m_eResult)
                    })
                },
            );
        }
    }

    /// Gets the publisher file IDs of all currently subscribed items.
    ///
    /// Set `include_locally_disabled` to `true` to include items that are
    /// locally disabled.
    pub fn subscribed_items(&self, include_locally_disabled: bool) -> Vec<PublishedFileId> {
        unsafe {
            let count =
                sys::SteamAPI_ISteamUGC_GetNumSubscribedItems(self.ugc, include_locally_disabled);
            let mut data: Vec<sys::PublishedFileId_t> = vec![0; count as usize];
            let gotten_count = sys::SteamAPI_ISteamUGC_GetSubscribedItems(
                self.ugc,
                data.as_mut_ptr(),
                count,
                include_locally_disabled,
            );
            debug_assert!(count == gotten_count);
            data.into_iter().map(|v| PublishedFileId(v)).collect()
        }
    }

    pub fn item_state(&self, item: PublishedFileId) -> ItemState {
        unsafe {
            let state = sys::SteamAPI_ISteamUGC_GetItemState(self.ugc, item.0);
            ItemState::from_bits_truncate(state)
        }
    }

    pub fn item_download_info(&self, item: PublishedFileId) -> Option<(u64, u64)> {
        unsafe {
            let mut current = 0u64;
            let mut total = 0u64;
            if sys::SteamAPI_ISteamUGC_GetItemDownloadInfo(
                self.ugc,
                item.0,
                &mut current,
                &mut total,
            ) {
                Some((current, total))
            } else {
                None
            }
        }
    }

    pub fn item_install_info(&self, item: PublishedFileId) -> Option<InstallInfo> {
        unsafe {
            let mut size_on_disk = 0u64;
            let mut folder = [0 as c_char; 4096];
            let mut timestamp = 0u32;
            if sys::SteamAPI_ISteamUGC_GetItemInstallInfo(
                self.ugc,
                item.0,
                &mut size_on_disk,
                folder.as_mut_ptr(),
                folder.len() as _,
                &mut timestamp,
            ) {
                Some(InstallInfo {
                    folder: CStr::from_ptr(folder.as_ptr())
                        .to_string_lossy()
                        .into_owned(),
                    size_on_disk,
                    timestamp,
                })
            } else {
                None
            }
        }
    }

    pub fn download_item(&self, item: PublishedFileId, high_priority: bool) -> bool {
        unsafe { sys::SteamAPI_ISteamUGC_DownloadItem(self.ugc, item.0, high_priority) }
    }

    /// Queries a paged list of all workshop items.
    pub fn query_all(
        &self,
        query_type: UGCQueryType,
        item_type: UGCType,
        appids: AppIDs,
        page: u32,
    ) -> Result<QueryHandle, CreateQueryError> {
        // Call the external function with the correct parameters
        let handle = unsafe {
            sys::SteamAPI_ISteamUGC_CreateQueryAllUGCRequestPage(
                self.ugc,
                query_type.into(),
                item_type.into(),
                appids.creator_app_id().unwrap_or(AppId(0)).0,
                appids.consumer_app_id().unwrap_or(AppId(0)).0,
                page,
            )
        };

        // Check for an invalid handle
        if handle == UGCQueryHandleInvalid {
            return Err(CreateQueryError);
        }

        // Return a new AllQuery instance
        Ok(QueryHandle {
            ugc: self.ugc,
            inner: Arc::clone(&self.inner),
            handle: Some(handle),
        })
    }

    /// Queries every matching item with Steam's deep paging interface.
    ///
    /// `cursor` continues a previous page; Steam's header says it "should be set
    /// to NULL or `\"*\"` to get the first result set", which is what `None`
    /// sends. The answer carries [`QueryResults::next_cursor`] while more results
    /// remain. A cursor with an interior nul never reaches Steam and is refused as
    /// `CreateQueryError`.
    pub fn query_all_with_cursor(
        &self,
        query_type: UGCQueryType,
        item_type: UGCType,
        appids: AppIDs,
        cursor: Option<&str>,
    ) -> Result<QueryHandle, CreateQueryError> {
        let cursor = match cursor {
            Some(cursor) => Some(CString::new(cursor).map_err(|_| CreateQueryError)?),
            None => None,
        };
        let cursor_ptr = cursor
            .as_ref()
            .map_or(std::ptr::null(), |cursor| cursor.as_ptr());
        let handle = unsafe {
            sys::SteamAPI_ISteamUGC_CreateQueryAllUGCRequestCursor(
                self.ugc,
                query_type.into(),
                item_type.into(),
                appids.creator_app_id().unwrap_or(AppId(0)).0,
                appids.consumer_app_id().unwrap_or(AppId(0)).0,
                cursor_ptr,
            )
        };
        if handle == UGCQueryHandleInvalid {
            return Err(CreateQueryError);
        }
        Ok(QueryHandle {
            ugc: self.ugc,
            inner: Arc::clone(&self.inner),
            handle: Some(handle),
        })
    }

    /// Queries a list of workshop itmes, related to a user in some way (Ex. user's subscriptions, favorites, upvoted, ...)
    pub fn query_user(
        &self,
        account: AccountId,
        list_type: UserList,
        item_type: UGCType,
        sort_order: UserListOrder,
        appids: AppIDs,
        page: u32,
    ) -> Result<QueryHandle, CreateQueryError> {
        let res = unsafe {
            sys::SteamAPI_ISteamUGC_CreateQueryUserUGCRequest(
                self.ugc,
                account.0,
                list_type.into(),
                item_type.into(),
                sort_order.into(),
                appids.creator_app_id().unwrap_or(AppId(0)).0,
                appids.consumer_app_id().unwrap_or(AppId(0)).0,
                page,
            )
        };

        if res == UGCQueryHandleInvalid {
            return Err(CreateQueryError);
        }

        Ok(QueryHandle {
            ugc: self.ugc,
            inner: Arc::clone(&self.inner),
            handle: Some(res),
        })
    }

    pub fn query_items(
        &self,
        mut items: Vec<PublishedFileId>,
    ) -> Result<QueryHandle, CreateQueryError> {
        debug_assert!(items.len() > 0);

        let res = unsafe {
            sys::SteamAPI_ISteamUGC_CreateQueryUGCDetailsRequest(
                self.ugc,
                items.as_mut_ptr() as _,
                items.len() as _,
            )
        };

        if res == UGCQueryHandleInvalid {
            return Err(CreateQueryError);
        }

        Ok(QueryHandle {
            ugc: self.ugc,
            inner: Arc::clone(&self.inner),
            handle: Some(res),
        })
    }

    pub fn query_item(&self, item: PublishedFileId) -> Result<QueryHandle, CreateQueryError> {
        let mut items = vec![item];

        let res = unsafe {
            sys::SteamAPI_ISteamUGC_CreateQueryUGCDetailsRequest(
                self.ugc,
                items.as_mut_ptr() as _,
                1 as _,
            )
        };

        if res == UGCQueryHandleInvalid {
            return Err(CreateQueryError);
        }

        Ok(QueryHandle {
            ugc: self.ugc,
            inner: Arc::clone(&self.inner),
            handle: Some(res),
        })
    }

    /// **DELETES** the item from the Steam Workshop.
    pub fn delete_item<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_DeleteItem(self.ugc, published_file_id.0);
            register_call_result::<sys::DeleteItemResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else if let Err(error) = to_steam_result(v.m_eResult) {
                        Err(error)
                    } else {
                        Ok(())
                    })
                },
            );
        }
    }

    /// Start tracking playtime on a set of workshop items.
    pub fn start_playtime_tracking<F>(&self, published_file_ids: &[PublishedFileId], cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_StartPlaytimeTracking(
                self.ugc,
                published_file_ids.as_ptr() as *mut u64,
                published_file_ids.len() as u32,
            );
            register_call_result::<sys::StartPlaytimeTrackingResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else if let Err(error) = to_steam_result(v.m_eResult) {
                        Err(error)
                    } else {
                        Ok(())
                    })
                },
            );
        }
    }

    /// Stop tracking playtime on a set of workshop items.
    pub fn stop_playtime_tracking<F>(&self, published_file_ids: &[PublishedFileId], cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_StopPlaytimeTracking(
                self.ugc,
                published_file_ids.as_ptr() as *mut u64,
                published_file_ids.len() as u32,
            );
            register_call_result::<sys::StopPlaytimeTrackingResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else if let Err(error) = to_steam_result(v.m_eResult) {
                        Err(error)
                    } else {
                        Ok(())
                    })
                },
            );
        }
    }

    /// Stop tracking playtime of all workshop items.
    pub fn stop_playtime_tracking_for_all_items<F>(&self, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_StopPlaytimeTrackingForAllItems(self.ugc);
            register_call_result::<sys::StopPlaytimeTrackingResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else if let Err(error) = to_steam_result(v.m_eResult) {
                        Err(error)
                    } else {
                        Ok(())
                    })
                },
            );
        }
    }
}

impl UGC {
    /// Initialize this UGC interface for a Steam game server.
    ///
    /// You should pass in the Workshop depot, you can find this on SteamDB. It's usually just the app ID.
    ///
    /// The folder is a path to the directory where you wish for this game server to store UGC content.
    ///
    /// `true` upon success; otherwise, `false` if the calling user is not a game server or if the workshop is currently updating its content.
    pub fn init_for_game_server(&self, workshop_depot: sys::DepotId_t, folder: &str) -> bool {
        unsafe {
            let folder = CString::new(folder).unwrap();
            sys::SteamAPI_ISteamUGC_BInitWorkshopForGameServer(
                self.ugc,
                workshop_depot,
                folder.as_ptr(),
            )
        }
    }
}

/// A handle to update a published item
pub struct UpdateHandle {
    ugc: *mut sys::ISteamUGC,
    inner: Arc<Inner>,

    handle: sys::UGCUpdateHandle_t,
}

impl UpdateHandle {
    #[must_use]
    pub fn title(self, title: &str) -> Self {
        unsafe {
            let title = CString::new(title).unwrap();
            assert!(sys::SteamAPI_ISteamUGC_SetItemTitle(
                self.ugc,
                self.handle,
                title.as_ptr()
            ));
        }
        self
    }

    #[must_use]
    pub fn description(self, description: &str) -> Self {
        unsafe {
            let description = CString::new(description).unwrap();
            assert!(sys::SteamAPI_ISteamUGC_SetItemDescription(
                self.ugc,
                self.handle,
                description.as_ptr()
            ));
        }
        self
    }

    #[must_use]
    pub fn language(self, language: &str) -> Self {
        unsafe {
            let language = CString::new(language).unwrap();
            assert!(sys::SteamAPI_ISteamUGC_SetItemUpdateLanguage(
                self.ugc,
                self.handle,
                language.as_ptr()
            ));
        }
        self
    }

    #[must_use]
    pub fn preview_path(self, path: &Path) -> Self {
        unsafe {
            let path = path.canonicalize().unwrap();
            let preview_path = CString::new(&*path.to_string_lossy()).unwrap();
            assert!(sys::SteamAPI_ISteamUGC_SetItemPreview(
                self.ugc,
                self.handle,
                preview_path.as_ptr()
            ));
        }
        self
    }

    #[must_use]
    pub fn content_path(self, path: &Path) -> Self {
        unsafe {
            let path = path.canonicalize().unwrap();
            let content_path = CString::new(&*path.to_string_lossy()).unwrap();
            assert!(sys::SteamAPI_ISteamUGC_SetItemContent(
                self.ugc,
                self.handle,
                content_path.as_ptr()
            ));
        }
        self
    }

    #[must_use]
    pub fn metadata(self, metadata: &str) -> Self {
        unsafe {
            let metadata = CString::new(metadata).unwrap();
            assert!(sys::SteamAPI_ISteamUGC_SetItemMetadata(
                self.ugc,
                self.handle,
                metadata.as_ptr()
            ));
        }
        self
    }

    pub fn visibility(self, visibility: remote_storage::PublishedFileVisibility) -> Self {
        unsafe {
            assert!(sys::SteamAPI_ISteamUGC_SetItemVisibility(
                self.ugc,
                self.handle,
                visibility.into()
            ));
        }
        self
    }

    pub fn tags<S: AsRef<str>>(self, tags: Vec<S>, allow_admin_tags: bool) -> Self {
        unsafe {
            let mut tags = SteamParamStringArray::new(&tags);
            assert!(sys::SteamAPI_ISteamUGC_SetItemTags(
                self.ugc,
                self.handle,
                &tags.as_raw(),
                allow_admin_tags
            ));
        }
        self
    }

    pub fn add_key_value_tag(self, key: &str, value: &str) -> Self {
        unsafe {
            let key = CString::new(key).unwrap();
            let value = CString::new(value).unwrap();
            assert!(sys::SteamAPI_ISteamUGC_AddItemKeyValueTag(
                self.ugc,
                self.handle,
                key.as_ptr(),
                value.as_ptr()
            ));
        }
        self
    }

    pub fn remove_key_value_tag(self, key: &str) -> Self {
        unsafe {
            let key = CString::new(key).unwrap();
            assert!(sys::SteamAPI_ISteamUGC_RemoveItemKeyValueTags(
                self.ugc,
                self.handle,
                key.as_ptr()
            ));
        }
        self
    }

    pub fn add_content_descriptor(self, desc_id: UGCContentDescriptorID) -> Self {
        unsafe {
            assert!(sys::SteamAPI_ISteamUGC_AddContentDescriptor(
                self.ugc,
                self.handle,
                desc_id.into(),
            ));
        }
        self
    }

    pub fn remove_content_descriptor(self, desc_id: UGCContentDescriptorID) -> Self {
        unsafe {
            assert!(sys::SteamAPI_ISteamUGC_RemoveContentDescriptor(
                self.ugc,
                self.handle,
                desc_id.into()
            ));
        }
        self
    }

    pub fn remove_all_key_value_tags(self) -> Self {
        unsafe {
            assert!(sys::SteamAPI_ISteamUGC_RemoveAllItemKeyValueTags(
                self.ugc,
                self.handle
            ));
        }
        self
    }

    pub fn submit<F>(self, change_note: Option<&str>, cb: F) -> UpdateWatchHandle
    where
        F: FnOnce(Result<(PublishedFileId, bool), SteamError>) + 'static + Send,
    {
        use std::ptr;
        unsafe {
            let change_note = change_note.and_then(|v| CString::new(v).ok());
            let note = change_note.as_ref().map_or(ptr::null(), |v| v.as_ptr());
            let api_call = sys::SteamAPI_ISteamUGC_SubmitItemUpdate(self.ugc, self.handle, note);
            register_call_result::<sys::SubmitItemUpdateResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult).map(|()| {
                            (
                                PublishedFileId(v.m_nPublishedFileId),
                                v.m_bUserNeedsToAcceptWorkshopLegalAgreement,
                            )
                        })
                    })
                },
            );
        }
        UpdateWatchHandle {
            ugc: self.ugc,
            _inner: self.inner,
            handle: self.handle,
        }
    }
}

/// A handle to watch an update of a published item
pub struct UpdateWatchHandle {
    ugc: *mut sys::ISteamUGC,
    _inner: Arc<Inner>,

    handle: sys::UGCUpdateHandle_t,
}

unsafe impl Send for UpdateWatchHandle {}
unsafe impl Sync for UpdateWatchHandle {}

impl UpdateWatchHandle {
    pub fn progress(&self) -> (UpdateStatus, u64, u64) {
        unsafe {
            let mut progress = 0;
            let mut total = 0;
            let status = sys::SteamAPI_ISteamUGC_GetItemUpdateProgress(
                self.ugc,
                self.handle,
                &mut progress,
                &mut total,
            );
            let status = match status {
                sys::EItemUpdateStatus::k_EItemUpdateStatusInvalid => UpdateStatus::Invalid,
                sys::EItemUpdateStatus::k_EItemUpdateStatusPreparingConfig => {
                    UpdateStatus::PreparingConfig
                }
                sys::EItemUpdateStatus::k_EItemUpdateStatusPreparingContent => {
                    UpdateStatus::PreparingContent
                }
                sys::EItemUpdateStatus::k_EItemUpdateStatusUploadingContent => {
                    UpdateStatus::UploadingContent
                }
                sys::EItemUpdateStatus::k_EItemUpdateStatusUploadingPreviewFile => {
                    UpdateStatus::UploadingPreviewFile
                }
                sys::EItemUpdateStatus::k_EItemUpdateStatusCommittingChanges => {
                    UpdateStatus::CommittingChanges
                }
                _ => unreachable!(),
            };
            (status, progress, total)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStatus {
    Invalid,
    PreparingConfig,
    PreparingContent,
    UploadingContent,
    UploadingPreviewFile,
    CommittingChanges,
}

/// Query handle, to allow for more filtering.
pub struct QueryHandle {
    ugc: *mut sys::ISteamUGC,
    inner: Arc<Inner>,

    // Note: this is always filled except in `fetch`, where it must be taken
    // to prevent the handle from being dropped when this query is dropped.
    handle: Option<sys::UGCQueryHandle_t>,
}

impl Drop for QueryHandle {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.as_mut() {
            unsafe {
                sys::SteamAPI_ISteamUGC_ReleaseQueryUGCRequest(self.ugc, *handle);
            }
        }
    }
}

impl QueryHandle {
    /// Excludes items with a specific tag.
    ///
    /// Panics if `tag` could not be converted to a `CString`.
    pub fn exclude_tag(self, tag: &str) -> Self {
        let cstr = CString::new(tag)
            .expect("String passed to exclude_tag could not be converted to a c string");
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddExcludedTag(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Only include items with a specific tag.
    ///
    /// Panics if `tag` could not be converted to a `CString`.
    pub fn require_tag(self, tag: &str) -> Self {
        let cstr = CString::new(tag)
            .expect("String passed to require_tag could not be converted to a c string");
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddRequiredTag(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Sets how to match tags added by `require_tag`. If `true`, then any tag may match. If `false`, all required tags must match.
    pub fn any_required(self, any: bool) -> Self {
        let ok =
            unsafe { sys::SteamAPI_ISteamUGC_SetMatchAnyTag(self.ugc, self.handle.unwrap(), any) };
        debug_assert!(ok);
        self
    }

    /// Sets the language to return the title and description in for the items on a pending UGC Query.
    ///
    /// Defaults to "english"
    pub fn language(self, language: &str) -> Self {
        let cstr = CString::new(language)
            .expect("String passed to language could not be converted to a c string");
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetLanguage(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether results will be returned from the cache for the specific period of time on a pending UGC Query.
    ///
    /// Age is in seconds.
    pub fn allow_cached_response(self, max_age_s: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetAllowCachedResponse(
                self.ugc,
                self.handle.unwrap(),
                max_age_s,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Include the full description in results
    pub fn include_long_desc(self, include: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnLongDescription(
                self.ugc,
                self.handle.unwrap(),
                include,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Include children in results
    pub fn include_children(self, include: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnChildren(self.ugc, self.handle.unwrap(), include)
        };
        debug_assert!(ok);
        self
    }

    /// Include metadata in results
    pub fn include_metadata(self, include: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnMetadata(self.ugc, self.handle.unwrap(), include)
        };
        debug_assert!(ok);
        self
    }

    /// Include additional previews in results
    pub fn include_additional_previews(self, include: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnAdditionalPreviews(
                self.ugc,
                self.handle.unwrap(),
                include,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Include key value tags in results
    pub fn include_key_value_tags(self, include: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnKeyValueTags(self.ugc, self.handle.unwrap(), include)
        };
        debug_assert!(ok);
        self
    }

    /// Adds a tag that must be present on all returned items.
    pub fn add_required_tag(self, tag: &str) -> Self {
        let cstr = CString::new(tag).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddRequiredTag(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Adds a tag that must not be present on any returned items.
    pub fn add_excluded_tag(self, tag: &str) -> Self {
        let cstr = CString::new(tag).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddExcludedTag(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to only return the IDs of the items.
    pub fn set_return_only_ids(self, return_only_ids: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnOnlyIDs(
                self.ugc,
                self.handle.unwrap(),
                return_only_ids,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to return key value tags with the items.
    pub fn set_return_key_value_tags(self, return_kv_tags: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnKeyValueTags(
                self.ugc,
                self.handle.unwrap(),
                return_kv_tags,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to return the full description of the items.
    pub fn set_return_long_description(self, return_long_desc: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnLongDescription(
                self.ugc,
                self.handle.unwrap(),
                return_long_desc,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to return metadata with the items.
    pub fn set_return_metadata(self, return_metadata: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnMetadata(
                self.ugc,
                self.handle.unwrap(),
                return_metadata,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to return children with the items.
    pub fn set_return_children(self, return_children: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnChildren(
                self.ugc,
                self.handle.unwrap(),
                return_children,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to return additional previews with the items.
    pub fn set_return_additional_previews(self, return_additional_previews: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnAdditionalPreviews(
                self.ugc,
                self.handle.unwrap(),
                return_additional_previews,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to only return the total number of items.
    pub fn set_return_total_only(self, return_total_only: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnTotalOnly(
                self.ugc,
                self.handle.unwrap(),
                return_total_only,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets the language to return the title and description in.
    pub fn set_language(self, language: &str) -> Self {
        let cstr = CString::new(language).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetLanguage(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether results will be returned from the cache.
    pub fn set_allow_cached_response(self, max_age_seconds: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetAllowCachedResponse(
                self.ugc,
                self.handle.unwrap(),
                max_age_seconds,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets a filter for the cloud file name.
    pub fn set_cloud_file_name_filter(self, file_name: &str) -> Self {
        let cstr = CString::new(file_name).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetCloudFileNameFilter(
                self.ugc,
                self.handle.unwrap(),
                cstr.as_ptr(),
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether any of the required tags are sufficient for an item to be returned.
    pub fn set_match_any_tag(self, match_any_tag: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetMatchAnyTag(self.ugc, self.handle.unwrap(), match_any_tag)
        };
        debug_assert!(ok);
        self
    }

    /// Sets the full-text search string.
    pub fn set_search_text(self, search_text: &str) -> Self {
        let cstr = CString::new(search_text).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetSearchText(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Sets the number of days to consider for trending items.
    pub fn set_ranked_by_trend_days(self, days: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetRankedByTrendDays(self.ugc, self.handle.unwrap(), days)
        };
        debug_assert!(ok);
        self
    }

    /// Adds a required key-value tag that must be present on all returned items.
    pub fn add_required_key_value_tag(self, key: &str, value: &str) -> Self {
        let key_cstr = CString::new(key).unwrap();
        let value_cstr = CString::new(value).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddRequiredKeyValueTag(
                self.ugc,
                self.handle.unwrap(),
                key_cstr.as_ptr(),
                value_cstr.as_ptr(),
            )
        };
        debug_assert!(ok);
        self
    }

    /// Requires returned items to carry at least one tag from this group: Steam
    /// matches *any* of the tags in the group, where [`QueryHandle::require_tag`]
    /// demands one specific tag.
    ///
    /// A tag with an interior nul is refused as `InvalidParameter` instead of
    /// panicking, and none of the group is applied in that case.
    pub fn add_required_tag_group<S: AsRef<str>>(self, groups: &[S]) -> Result<Self, SteamError> {
        let strings = tag_group_strings(groups)?;
        let mut pointers: Vec<*const c_char> = strings.iter().map(|s| s.as_ptr()).collect();
        let raw = sys::SteamParamStringArray_t {
            m_nNumStrings: pointers.len() as i32,
            m_ppStrings: pointers.as_mut_ptr(),
        };
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddRequiredTagGroup(self.ugc, self.handle.unwrap(), &raw)
        };
        if ok {
            Ok(self)
        } else {
            Err(SteamError::InvalidParameter)
        }
    }

    /// Asks Steam to include playtime statistics for the last `days` days in the
    /// results ([`QueryResults::statistic`] then has values for them).
    #[must_use]
    pub fn set_return_playtime_stats(self, days: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnPlaytimeStats(self.ugc, self.handle.unwrap(), days)
        };
        debug_assert!(ok);
        self
    }

    /// Restricts results to items created between two Unix timestamps.
    ///
    /// The bounds go to Steam as given; a reversed range is Steam's to answer
    /// (with nothing), not something this wrapper silently swaps.
    #[must_use]
    pub fn set_time_created_date_range(self, start: u32, end: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetTimeCreatedDateRange(
                self.ugc,
                self.handle.unwrap(),
                start,
                end,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Restricts results to items updated between two Unix timestamps.
    ///
    /// See [`QueryHandle::set_time_created_date_range`] for why the bounds are
    /// passed through unexamined.
    #[must_use]
    pub fn set_time_updated_date_range(self, start: u32, end: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetTimeUpdatedDateRange(
                self.ugc,
                self.handle.unwrap(),
                start,
                end,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sends the query to Steam and calls the provided callback with the results when completed.
    ///
    /// When the query was created for deep paging, the answer carries
    /// [`QueryResults::next_cursor`], which is what [`UGC::query_all_with_cursor`]
    /// continues from.
    pub fn fetch<F>(mut self, cb: F)
    where
        F: for<'a> FnOnce(Result<QueryResults<'a>, SteamError>) + 'static + Send,
    {
        let ugc = self.ugc;
        let inner = Arc::clone(&self.inner);
        let handle = self.handle.take().unwrap();
        mem::drop(self);

        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_SendQueryUGCRequest(ugc, handle);
            register_call_result::<sys::SteamUGCQueryCompleted_t, _>(
                &inner,
                api_call,
                move |v, io_error| {
                    let ugc = sys::SteamAPI_SteamUGC_v021();
                    if io_error {
                        sys::SteamAPI_ISteamUGC_ReleaseQueryUGCRequest(ugc, handle);
                        cb(Err(SteamError::IOFailure));
                        return;
                    } else if let Err(error) = to_steam_result(v.m_eResult) {
                        sys::SteamAPI_ISteamUGC_ReleaseQueryUGCRequest(ugc, handle);
                        cb(Err(error));
                        return;
                    }

                    let result = QueryResults {
                        ugc,
                        handle,
                        num_results_returned: v.m_unNumResultsReturned,
                        num_results_total: v.m_unTotalMatchingResults,
                        was_cached: v.m_bCachedData,
                        next_cursor: next_cursor_from(&v.m_rgchNextCursor),
                        _phantom: Default::default(),
                    };
                    cb(Ok(result));
                },
            );
        }
    }

    /// Runs the query, only fetching the total number of results.
    pub fn fetch_total<F>(self, cb: F)
    where
        F: Fn(Result<u32, SteamError>) + 'static + Send,
    {
        unsafe {
            let ok =
                sys::SteamAPI_ISteamUGC_SetReturnTotalOnly(self.ugc, self.handle.unwrap(), true);
            debug_assert!(ok);
        }

        self.fetch(move |res| cb(res.map(|qr| qr.total_results())))
    }

    /// Runs the query, only fetching the IDs.
    pub fn fetch_ids<F>(self, cb: F)
    where
        F: Fn(Result<Vec<PublishedFileId>, SteamError>) + 'static + Send,
    {
        unsafe {
            let ok = sys::SteamAPI_ISteamUGC_SetReturnOnlyIDs(self.ugc, self.handle.unwrap(), true);
            debug_assert!(ok);
        }

        self.fetch(move |res| {
            cb(res.map(|qr| {
                qr.iter()
                    .filter_map(|v| v.map(|v| PublishedFileId(v.published_file_id.0)))
                    .collect::<Vec<_>>()
            }))
        })
    }
}

/// Query results
pub struct QueryResults<'a> {
    ugc: *mut sys::ISteamUGC,
    handle: sys::UGCQueryHandle_t,
    num_results_returned: u32,
    num_results_total: u32,
    was_cached: bool,
    next_cursor: Option<String>,
    _phantom: marker::PhantomData<&'a sys::ISteamUGC>,
}
impl<'a> Drop for QueryResults<'a> {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamUGC_ReleaseQueryUGCRequest(self.ugc, self.handle);
        }
    }
}
impl<'a> QueryResults<'a> {
    /// Were these results retreived from a cache?
    pub fn was_cached(&self) -> bool {
        self.was_cached
    }

    /// Gets the total number of results in this query, not just the current page
    pub fn total_results(&self) -> u32 {
        self.num_results_total
    }

    /// Gets the number of results in this page.
    pub fn returned_results(&self) -> u32 {
        self.num_results_returned
    }

    /// Gets the preview URL of the published file at the specified index.
    pub fn preview_url(&self, index: u32) -> Option<String> {
        let mut url = [0 as c_char; 4096];

        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCPreviewURL(
                self.ugc,
                self.handle,
                index,
                url.as_mut_ptr(),
                url.len() as _,
            )
        };

        if ok {
            Some(unsafe { CStr::from_ptr(url.as_ptr()).to_string_lossy().into_owned() })
        } else {
            None
        }
    }

    /// Gets a UGC statistic about the published file at the specified index.
    pub fn statistic(&self, index: u32, stat_type: UGCStatisticType) -> Option<u64> {
        let mut value = 0u64;

        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCStatistic(
                self.ugc,
                self.handle,
                index,
                stat_type.into(),
                &mut value,
            )
        };

        debug_assert!(ok);

        if ok {
            Some(value)
        } else {
            None
        }
    }

    /// Gets UGCContentDescriptors of the published file at the specified index.
    pub fn content_descriptor(&self, index: u32) -> Vec<UGCContentDescriptorID> {
        let mut descriptors: [sys::EUGCContentDescriptorID; 10] = unsafe { std::mem::zeroed() };
        let max_entries = descriptors.len() as std::ffi::c_uint;

        let num_descriptors = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCContentDescriptors(
                self.ugc,
                self.handle,
                index,
                descriptors.as_mut_ptr(),
                max_entries,
            )
        } as usize;

        Vec::from(&descriptors[..num_descriptors])
            .iter()
            .map(|&x| x.into())
            .collect()
    }

    /// Gets a result.
    ///
    /// Returns None if index was out of bounds.
    pub fn get(&self, index: u32) -> Option<QueryResult> {
        if index >= self.num_results_returned {
            return None;
        }

        unsafe {
            let mut raw_details: sys::SteamUGCDetails_t = mem::zeroed();
            let ok = sys::SteamAPI_ISteamUGC_GetQueryUGCResult(
                self.ugc,
                self.handle,
                index,
                &mut raw_details,
            );
            debug_assert!(ok);

            if to_steam_result(raw_details.m_eResult).is_err() {
                return None;
            }

            let tags = CStr::from_ptr(raw_details.m_rgchTags.as_ptr())
                .to_string_lossy()
                .split(',')
                .map(|s| String::from(s))
                .collect::<Vec<_>>();

            Some(QueryResult {
                published_file_id: PublishedFileId(raw_details.m_nPublishedFileId),
                creator_app_id: if raw_details.m_nCreatorAppID != 0 {
                    Some(AppId(raw_details.m_nCreatorAppID))
                } else {
                    None
                },
                consumer_app_id: if raw_details.m_nConsumerAppID != 0 {
                    Some(AppId(raw_details.m_nConsumerAppID))
                } else {
                    None
                },
                title: CStr::from_ptr(raw_details.m_rgchTitle.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                description: CStr::from_ptr(raw_details.m_rgchDescription.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                owner: SteamId(raw_details.m_ulSteamIDOwner),
                time_created: raw_details.m_rtimeCreated,
                time_updated: raw_details.m_rtimeUpdated,
                time_added_to_user_list: raw_details.m_rtimeAddedToUserList,
                visibility: raw_details.m_eVisibility.into(),
                banned: raw_details.m_bBanned,
                accepted_for_use: raw_details.m_bAcceptedForUse,
                url: CStr::from_ptr(raw_details.m_rgchURL.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                num_upvotes: raw_details.m_unVotesUp,
                num_downvotes: raw_details.m_unVotesDown,
                score: raw_details.m_flScore,
                num_children: raw_details.m_unNumChildren,
                tags,
                tags_truncated: raw_details.m_bTagsTruncated,
                file_name: CStr::from_ptr(raw_details.m_pchFileName.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                file_type: raw_details.m_eFileType.into(),
                file_size: raw_details.m_nFileSize.max(0) as u32,
            })
        }
    }

    /// Returns an iterator that runs over all the fetched results
    pub fn iter<'b>(&'b self) -> impl Iterator<Item = Option<QueryResult>> + 'b {
        (0..self.returned_results()).map(move |i| self.get(i))
    }

    /// Returns the given index's children as a list of PublishedFileId.
    ///
    /// You must call `include_children(true)` before fetching the query for this to work.
    ///
    /// Returns None if the index was out of bounds.
    pub fn get_children(&self, index: u32) -> Option<Vec<PublishedFileId>> {
        let num_children = self.get(index)?.num_children;
        let mut children: Vec<sys::PublishedFileId_t> = vec![0; num_children as usize];

        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCChildren(
                self.ugc,
                self.handle,
                index,
                children.as_mut_ptr(),
                num_children,
            )
        };

        if ok {
            Some(children.into_iter().map(Into::into).collect())
        } else {
            None
        }
    }

    /// Returns the number of key value tags associated with the item at the specified index.
    pub fn key_value_tags(&self, index: u32) -> u32 {
        unsafe { sys::SteamAPI_ISteamUGC_GetQueryUGCNumKeyValueTags(self.ugc, self.handle, index) }
    }

    /// Gets the key value pair of a specified key value tag associated with the item at the specified index.
    pub fn get_key_value_tag(&self, index: u32, kv_tag_index: u32) -> Option<(String, String)> {
        let mut key = [0 as c_char; 256];
        let mut value = [0 as c_char; 256];

        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCKeyValueTag(
                self.ugc,
                self.handle,
                index,
                kv_tag_index,
                key.as_mut_ptr(),
                256,
                value.as_mut_ptr(),
                256,
            )
        };

        if ok {
            Some(unsafe {
                (
                    CStr::from_ptr(key.as_ptr()).to_string_lossy().into_owned(),
                    CStr::from_ptr(value.as_ptr())
                        .to_string_lossy()
                        .into_owned(),
                )
            })
        } else {
            None
        }
    }

    /// The tags Steam reports for the item at `index`.
    ///
    /// Steam answers a tag count and then one call per tag, so this reads the
    /// pair itself. A tag Steam refuses (an index past the end, or a query built
    /// without `include_key_value_tags`-style preview data) comes back as `None`
    /// rather than as an empty tag.
    pub fn tag(&self, index: u32, tag_index: u32) -> Option<String> {
        self.read_tag(index, tag_index, sys::SteamAPI_ISteamUGC_GetQueryUGCTag)
    }

    /// The display name of the tag at `tag_index`, which Steam keeps separate
    /// from the tag's machine name ([`QueryResults::tag`]).
    pub fn tag_display_name(&self, index: u32, tag_index: u32) -> Option<String> {
        self.read_tag(
            index,
            tag_index,
            sys::SteamAPI_ISteamUGC_GetQueryUGCTagDisplayName,
        )
    }

    /// How many tags Steam reports for the item at `index`.
    pub fn tag_count(&self, index: u32) -> u32 {
        unsafe { sys::SteamAPI_ISteamUGC_GetQueryUGCNumTags(self.ugc, self.handle, index) }
    }

    fn read_tag(
        &self,
        index: u32,
        tag_index: u32,
        read: unsafe extern "C" fn(
            *mut sys::ISteamUGC,
            sys::UGCQueryHandle_t,
            u32,
            u32,
            *mut c_char,
            u32,
        ) -> bool,
    ) -> Option<String> {
        let mut buffer = vec![0 as c_char; sys::k_cchTagListMax as usize];
        let ok = unsafe {
            read(
                self.ugc,
                self.handle,
                index,
                tag_index,
                buffer.as_mut_ptr(),
                sys::k_cchTagListMax,
            )
        };
        ok.then(|| steam_out_string(&buffer))
    }

    /// How many previews besides the main one Steam reports for `index`.
    pub fn additional_preview_count(&self, index: u32) -> u32 {
        unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCNumAdditionalPreviews(self.ugc, self.handle, index)
        }
    }

    /// One of the item's extra previews: a URL or YouTube id, the original file
    /// name Steam recorded, and what kind of preview it is.
    ///
    /// Requires the query to have been built with
    /// [`QueryHandle::set_return_additional_previews`] (or
    /// [`QueryHandle::include_additional_previews`]); Steam answers `None`
    /// otherwise, which is a missing request, not an empty preview.
    pub fn additional_preview(&self, index: u32, preview_index: u32) -> Option<AdditionalPreview> {
        let mut url = vec![0 as c_char; sys::k_cchPublishedFileURLMax as usize];
        let mut filename = vec![0 as c_char; sys::k_cchFilenameMax as usize];
        // Steam writes a 4-byte EItemPreviewType here. This wrapper deliberately
        // does not read it as `sys::EItemPreviewType`: that enum is `#[repr(i32)]`
        // on Windows and `#[repr(u32)]` on the other platforms, and both are
        // `#[non_exhaustive]`, so a code Steam has not named in this binding must
        // not be turned into one of its variants.
        let mut kind: i32 = 0;
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCAdditionalPreview(
                self.ugc,
                self.handle,
                index,
                preview_index,
                url.as_mut_ptr(),
                sys::k_cchPublishedFileURLMax,
                filename.as_mut_ptr(),
                sys::k_cchFilenameMax,
                &mut kind as *mut i32 as *mut sys::EItemPreviewType,
            )
        };
        ok.then(|| AdditionalPreview {
            url_or_video_id: steam_out_string(&url),
            original_filename: steam_out_string(&filename),
            kind: preview_kind_from(kind),
        })
    }

    /// The first value stored under `key` for the item at `index`.
    ///
    /// Steam's own answer is "false if there was an error or no matching value
    /// was found", so `None` covers both. A key containing an interior nul can
    /// never match a C string, and is answered the same way instead of panicking.
    /// A key may map to several values; [`QueryResults::get_key_value_tag`] walks
    /// them by index.
    pub fn first_key_value_tag(&self, index: u32, key: &str) -> Option<String> {
        let key = CString::new(key).ok()?;
        let mut value = vec![0 as c_char; UGC_KEY_VALUE_TAG_MAX as usize];
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryFirstUGCKeyValueTag(
                self.ugc,
                self.handle,
                index,
                key.as_ptr(),
                value.as_mut_ptr(),
                UGC_KEY_VALUE_TAG_MAX,
            )
        };
        ok.then(|| steam_out_string(&value))
    }

    /// How many game-version (Steam branch) ranges the item at `index` declares.
    pub fn supported_game_version_count(&self, index: u32) -> u32 {
        unsafe { sys::SteamAPI_ISteamUGC_GetNumSupportedGameVersions(self.ugc, self.handle, index) }
    }

    /// One declared game-version range, as the two branch names Steam recorded.
    ///
    /// The ends stay exactly as Steam answers them, including an empty one: this
    /// SDK's header only defines the empty-string meaning for the *setter*
    /// (`SetRequiredGameVersions`), so reading one back as "unbounded" would be
    /// this wrapper's invention.
    pub fn supported_game_version(
        &self,
        index: u32,
        version_index: u32,
    ) -> Option<GameVersionRange> {
        let mut min_branch = vec![0 as c_char; UGC_GAME_BRANCH_MAX as usize];
        let mut max_branch = vec![0 as c_char; UGC_GAME_BRANCH_MAX as usize];
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetSupportedGameVersionData(
                self.ugc,
                self.handle,
                index,
                version_index,
                min_branch.as_mut_ptr(),
                max_branch.as_mut_ptr(),
                UGC_GAME_BRANCH_MAX,
            )
        };
        ok.then(|| GameVersionRange {
            min_branch: steam_out_string(&min_branch),
            max_branch: steam_out_string(&max_branch),
        })
    }

    /// The cursor for the next page of a deep-paged query, or `None` when Steam
    /// gave none (a page-size query, or the last page).
    ///
    /// Feed it to [`UGC::query_all_with_cursor`] to continue.
    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }

    /// Gets the developer-set metadata associated with the item at the specified index.
    ///
    /// This is returned as a vector of raw bytes.
    pub fn get_metadata(&self, index: u32) -> Option<Vec<u8>> {
        let mut metadata = [0 as c_char; sys::k_cchDeveloperMetadataMax as usize];

        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCMetadata(
                self.ugc,
                self.handle,
                index,
                metadata.as_mut_ptr(),
                sys::k_cchDeveloperMetadataMax,
            )
        };

        if ok {
            let metadata = unsafe { CStr::from_ptr(metadata.as_ptr()).to_bytes() };
            if metadata.is_empty() {
                None
            } else {
                Some(metadata.to_vec())
            }
        } else {
            None
        }
    }
}

/// Query result
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub published_file_id: PublishedFileId,
    pub creator_app_id: Option<AppId>,
    pub consumer_app_id: Option<AppId>,
    pub title: String,
    pub description: String,
    pub owner: SteamId,
    /// Time created in unix epoch seconds format
    pub time_created: u32,
    /// Time updated in unix epoch seconds format
    pub time_updated: u32,
    /// Time when the user added the published item to their list (not always applicable), provided in Unix epoch format (time since Jan 1st, 1970).
    pub time_added_to_user_list: u32,
    pub visibility: PublishedFileVisibility,
    pub banned: bool,
    pub accepted_for_use: bool,
    pub tags: Vec<String>,
    pub tags_truncated: bool,
    /// Original file name of the workshop item. Used in old games, like Total War: Shogun 2.
    pub file_name: String,
    pub file_type: FileType,
    pub file_size: u32,

    pub url: String,
    pub num_upvotes: u32,
    pub num_downvotes: u32,
    /// The bayesian average for up votes / total votes, between \[0,1\].
    pub score: f32,
    pub num_children: u32,
    // TODO: Add missing fields as needed
}

#[derive(Debug, Clone, Copy)]
pub struct CreateQueryError;
impl fmt::Display for CreateQueryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Could not create workshop query")
    }
}
impl error::Error for CreateQueryError {}

/// How much this wrapper asks Steam for a key-value tag's value.
///
/// Steam publishes no such limit (`k_cchTagListMax` covers a whole tag list, not
/// one value), so this keeps the 256 the file has always used for KV tags.
const UGC_KEY_VALUE_TAG_MAX: u32 = 256;

/// How much this wrapper asks Steam for one end of a game-version range. Same
/// reason as [`UGC_KEY_VALUE_TAG_MAX`]: no published per-branch limit to size by.
const UGC_GAME_BRANCH_MAX: u32 = 256;

/// Turns a required-tag group into the C strings Steam expects.
///
/// One unusable tag fails the whole group: a partial group would ask Steam for
/// something the caller never wrote.
fn tag_group_strings<S: AsRef<str>>(groups: &[S]) -> Result<Vec<CString>, SteamError> {
    groups
        .iter()
        .map(|tag| CString::new(tag.as_ref()).map_err(|_| SteamError::InvalidParameter))
        .collect()
}

/// Reads a Steam out-string without trusting its terminator: the answer is cut at
/// the first nul inside the buffer this side owns, so an answer that fills the
/// whole buffer cannot make the read run past it.
fn steam_out_string(buffer: &[c_char]) -> String {
    let end = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    let bytes: Vec<u8> = buffer[..end].iter().map(|byte| *byte as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The paging cursor of a completed query. Steam leaves the field empty for a
/// page-number query and for the last cursor page, and both mean "no next page".
fn next_cursor_from(raw: &[c_char]) -> Option<String> {
    let cursor = steam_out_string(raw);
    (!cursor.is_empty()).then_some(cursor)
}

/// One of the extra previews on a workshop item, beyond its main picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdditionalPreview {
    /// A URL, or a YouTube video id for [`PreviewKind::YouTubeVideo`] -- Steam
    /// uses the same field for both.
    pub url_or_video_id: String,
    pub original_filename: String,
    pub kind: PreviewKind,
}

/// One game-version (Steam branch) range an item declares it works with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameVersionRange {
    pub min_branch: String,
    pub max_branch: String,
}

/// What kind of preview Steam recorded for an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    Image,
    YouTubeVideo,
    Sketchfab,
    EnvironmentMapHorizontalCross,
    EnvironmentMapLatLong,
    Clip,
    /// A preview type this binding cannot name, including Steam's `ReservedMax`
    /// sentinel and any type added after these bindings were generated.
    Unknown,
}

/// Maps Steam's `EItemPreviewType` answer.
///
/// The out-parameter is read as a plain 32-bit integer rather than as
/// `sys::EItemPreviewType`: the generated enum is `#[repr(i32)]` on Windows and
/// `#[repr(u32)]` on the other platforms, and both are `#[non_exhaustive]`.
fn preview_kind_from(raw: i32) -> PreviewKind {
    use sys::EItemPreviewType as P;
    match raw {
        x if x == P::k_EItemPreviewType_Image as i32 => PreviewKind::Image,
        x if x == P::k_EItemPreviewType_YouTubeVideo as i32 => PreviewKind::YouTubeVideo,
        x if x == P::k_EItemPreviewType_Sketchfab as i32 => PreviewKind::Sketchfab,
        x if x == P::k_EItemPreviewType_EnvironmentMap_HorizontalCross as i32 => {
            PreviewKind::EnvironmentMapHorizontalCross
        }
        x if x == P::k_EItemPreviewType_EnvironmentMap_LatLong as i32 => {
            PreviewKind::EnvironmentMapLatLong
        }
        x if x == P::k_EItemPreviewType_Clip as i32 => PreviewKind::Clip,
        _ => PreviewKind::Unknown,
    }
}

#[cfg(test)]
mod query_surface_tests {
    use super::*;

    #[test]
    fn steam_out_strings_are_cut_inside_the_buffer_that_owns_them() {
        let mut buffer = [0 as c_char; 8];
        buffer[..5].copy_from_slice(&[72, 105, 0, 33, 33].map(i8_cast));
        assert_eq!(steam_out_string(&buffer), "Hi");

        // No terminator inside the buffer: the whole thing is the answer, and
        // nothing past it is read.
        let full = b"12345678".map(i8_cast);
        assert_eq!(steam_out_string(&full), "12345678");

        let empty = [0 as c_char; 4];
        assert_eq!(steam_out_string(&empty), "");

        // Steam hands back bytes, not promised UTF-8.
        let not_utf8 = [0xffu8, b'a', 0].map(i8_cast);
        assert_eq!(steam_out_string(&not_utf8), "\u{fffd}a");
    }

    /// Keeps the fixtures byte-for-byte without `as` casts at every literal.
    const fn i8_cast(byte: u8) -> i8 {
        byte as i8
    }

    #[test]
    fn an_empty_cursor_is_no_next_page() {
        let none = [0 as c_char; 6];
        assert_eq!(next_cursor_from(&none), None);

        let mut page = [0 as c_char; 6];
        page[..5].copy_from_slice(&b"abc12".map(i8_cast));
        assert_eq!(next_cursor_from(&page).as_deref(), Some("abc12"));
    }

    #[test]
    fn preview_types_map_without_collapsing_or_guessing() {
        let named = [
            (0, PreviewKind::Image),
            (1, PreviewKind::YouTubeVideo),
            (2, PreviewKind::Sketchfab),
            (3, PreviewKind::EnvironmentMapHorizontalCross),
            (4, PreviewKind::EnvironmentMapLatLong),
            (5, PreviewKind::Clip),
        ];
        for (raw, expected) in named {
            assert_eq!(preview_kind_from(raw), expected);
        }
        let mapped: Vec<PreviewKind> = named.iter().map(|(_, kind)| *kind).collect();
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(mapped[i], mapped[j], "Steam codes must not merge");
            }
        }
        // The sentinel and the gaps are answers this binding cannot name.
        for unknown in [-1, 6, 128, 255, i32::MAX] {
            assert_eq!(
                preview_kind_from(unknown),
                PreviewKind::Unknown,
                "{unknown} must not be reported as a named preview type"
            );
        }
    }

    #[test]
    fn the_constants_the_query_readers_size_their_buffers_with_are_usable() {
        // Guards against an edit that makes a buffer smaller than the length
        // handed to Steam: the call would then write past the allocation.
        for len in [UGC_KEY_VALUE_TAG_MAX, UGC_GAME_BRANCH_MAX] {
            assert!(len > 1, "a {len}-byte buffer cannot hold a value and a nul");
            let buffer = vec![0 as c_char; len as usize];
            assert_eq!(buffer.len(), len as usize);
        }
        let tag_buffer = vec![0 as c_char; sys::k_cchTagListMax as usize];
        assert_eq!(tag_buffer.len(), 1025);
    }

    #[test]
    fn a_tag_group_with_an_interior_nul_is_refused_as_a_whole() {
        let good = tag_group_strings(&["env", "pvp"]);
        assert!(good.is_ok());
        assert_eq!(good.unwrap().len(), 2);

        // One unusable tag must fail the group rather than quietly send the part
        // that survived: Steam would then filter on something nobody asked for.
        let bad = tag_group_strings(&["env", "bad\0tag"]);
        assert_eq!(bad.err(), Some(SteamError::InvalidParameter));

        let empty: [&str; 0] = [];
        assert_eq!(tag_group_strings(&empty).map(|v| v.len()), Ok(0));
    }
}
