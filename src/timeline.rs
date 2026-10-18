use super::*;
use crate::utils::c_string_prefix;
use std::time::Duration;

pub struct Timeline {
    pub(crate) timeline: *mut sys::ISteamTimeline,
    /// Whether the client's steam API is not recent enough.
    pub(crate) disabled: bool,
    pub(crate) _inner: Arc<Inner>,
}

pub enum TimelineGameMode {
    /// The player is fully loaded into the game and playing.
    Playing,
    /// The player is in a multiplayer lobby.
    Staging,
    /// The player is in the game's main menu or a pause menu.
    Menus,
    /// The player is waiting for a loading screen.
    LoadingScreen,
}

impl From<TimelineGameMode> for sys::ETimelineGameMode {
    fn from(mode: TimelineGameMode) -> Self {
        match mode {
            TimelineGameMode::Playing => sys::ETimelineGameMode::k_ETimelineGameMode_Playing,
            TimelineGameMode::Staging => sys::ETimelineGameMode::k_ETimelineGameMode_Staging,
            TimelineGameMode::Menus => sys::ETimelineGameMode::k_ETimelineGameMode_Menus,
            TimelineGameMode::LoadingScreen => {
                sys::ETimelineGameMode::k_ETimelineGameMode_LoadingScreen
            }
        }
    }
}

pub enum TimelineEventClipPriority {
    /// This event is not appropriate as a clip.
    None,
    /// The user may want to make a clip around this event.
    Standard,
    /// The player will be likely to want a clip around event,
    /// and those clips should be promoted more prominently than clips with the [TimelineEventClipPriority::Standard] priority.
    Featured,
}

impl From<TimelineEventClipPriority> for sys::ETimelineEventClipPriority {
    fn from(priority: TimelineEventClipPriority) -> Self {
        match priority {
            TimelineEventClipPriority::None => {
                sys::ETimelineEventClipPriority::k_ETimelineEventClipPriority_None
            }
            TimelineEventClipPriority::Standard => {
                sys::ETimelineEventClipPriority::k_ETimelineEventClipPriority_Standard
            }
            TimelineEventClipPriority::Featured => {
                sys::ETimelineEventClipPriority::k_ETimelineEventClipPriority_Featured
            }
        }
    }
}

impl Timeline {
    /// Changes the color of the timeline bar.
    pub fn set_timeline_game_mode(&self, mode: TimelineGameMode) {
        if self.disabled {
            return;
        }

        unsafe {
            sys::SteamAPI_ISteamTimeline_SetTimelineGameMode(self.timeline, mode.into());
        }
    }

    /// Sets a description for the current game state in the timeline.
    /// These help the user to find specific moments in the timeline when saving clips.
    /// Setting a new state description replaces any previous description.
    pub fn set_timeline_state_description(&self, description: &str, duration: Duration) {
        if self.disabled {
            return;
        }

        let description = c_string_prefix(description);

        unsafe {
            sys::SteamAPI_ISteamTimeline_SetTimelineTooltip(
                self.timeline,
                description.as_ptr(),
                duration.as_secs_f32(),
            )
        }
    }

    /// Clears the previous set game state in the timeline.
    pub fn clear_timeline_state_description(&self, duration: Duration) {
        if self.disabled {
            return;
        }

        unsafe {
            sys::SteamAPI_ISteamTimeline_ClearTimelineTooltip(self.timeline, duration.as_secs_f32())
        }
    }

    /// Use this to mark an event on the Timeline.
    /// The event can be instantaneous or take some amount of time to complete,
    /// depending on the value passed in `duration`.
    pub fn add_timeline_event(
        &self,
        icon: &str,
        title: &str,
        description: &str,
        priority: u32,
        start_offset_seconds: f32,
        duration: Duration,
        clip_priority: TimelineEventClipPriority,
    ) {
        if self.disabled {
            return;
        }

        let icon = c_string_prefix(icon);
        let title = c_string_prefix(title);
        let description = c_string_prefix(description);
        let duration = duration.as_secs_f32();

        unsafe {
            // Steam's parameter order is title, description, icon.
            let _handle = sys::SteamAPI_ISteamTimeline_AddRangeTimelineEvent(
                self.timeline,
                title.as_ptr(),
                description.as_ptr(),
                icon.as_ptr(),
                priority,
                start_offset_seconds,
                duration,
                clip_priority.into(),
            );
        }
    }

    /// Marks a single moment on the timeline and returns Steam's handle for it.
    ///
    /// Unlike [`Timeline::add_timeline_event`], which discards the handle, this
    /// keeps it so the event can later be removed or opened in the overlay.
    pub fn add_instantaneous_event(
        &self,
        title: &str,
        description: &str,
        icon: &str,
        priority: u32,
        start_offset_seconds: f32,
        clip_priority: TimelineEventClipPriority,
    ) -> Result<TimelineEventHandle, SteamError> {
        self.guarded(|| {
            let title = timeline_cstr(title)?;
            let description = timeline_cstr(description)?;
            let icon = timeline_cstr(icon)?;
            let raw = unsafe {
                sys::SteamAPI_ISteamTimeline_AddInstantaneousTimelineEvent(
                    self.timeline,
                    title.as_ptr(),
                    description.as_ptr(),
                    icon.as_ptr(),
                    priority,
                    start_offset_seconds,
                    clip_priority.into(),
                )
            };
            Ok(TimelineEventHandle(raw))
        })
    }

    /// Opens a timeline event whose end comes later, from
    /// [`Timeline::end_range_event`].
    ///
    /// Steam discards any range event that is still open when the game exits,
    /// so an event started here is not on the timeline until it is ended.
    pub fn start_range_event(
        &self,
        title: &str,
        description: &str,
        icon: &str,
        priority: u32,
        start_offset_seconds: f32,
        clip_priority: TimelineEventClipPriority,
    ) -> Result<TimelineEventHandle, SteamError> {
        self.guarded(|| {
            let title = timeline_cstr(title)?;
            let description = timeline_cstr(description)?;
            let icon = timeline_cstr(icon)?;
            let raw = unsafe {
                sys::SteamAPI_ISteamTimeline_StartRangeTimelineEvent(
                    self.timeline,
                    title.as_ptr(),
                    description.as_ptr(),
                    icon.as_ptr(),
                    priority,
                    start_offset_seconds,
                    clip_priority.into(),
                )
            };
            Ok(TimelineEventHandle(raw))
        })
    }

    /// Updates an open range event started by [`Timeline::start_range_event`].
    pub fn update_range_event(
        &self,
        event: TimelineEventHandle,
        title: &str,
        description: &str,
        icon: &str,
        priority: u32,
        clip_priority: TimelineEventClipPriority,
    ) -> Result<(), SteamError> {
        self.guarded(|| {
            let title = timeline_cstr(title)?;
            let description = timeline_cstr(description)?;
            let icon = timeline_cstr(icon)?;
            unsafe {
                sys::SteamAPI_ISteamTimeline_UpdateRangeTimelineEvent(
                    self.timeline,
                    event.raw(),
                    title.as_ptr(),
                    description.as_ptr(),
                    icon.as_ptr(),
                    priority,
                    clip_priority.into(),
                )
            };
            Ok(())
        })
    }

    /// Ends an open range event and shows it on the timeline.
    pub fn end_range_event(&self, event: TimelineEventHandle, end_offset_seconds: f32) {
        if self.disabled {
            return;
        }
        unsafe {
            sys::SteamAPI_ISteamTimeline_EndRangeTimelineEvent(
                self.timeline,
                event.raw(),
                end_offset_seconds,
            )
        }
    }

    /// Deletes an event from the timeline.
    ///
    /// The handle must come from the current game process, as Steam states.
    pub fn remove_event(&self, event: TimelineEventHandle) {
        if self.disabled {
            return;
        }
        unsafe { sys::SteamAPI_ISteamTimeline_RemoveTimelineEvent(self.timeline, event.raw()) }
    }

    /// Asks Steam whether a recording exists for `event`; the answer arrives as
    /// [`TimelineEventRecordingExists`].
    pub fn does_event_recording_exist<F>(
        &self,
        event: TimelineEventHandle,
        callback: F,
    ) -> Result<(), SteamError>
    where
        F: FnOnce(Result<TimelineEventRecordingExists, SteamError>) + 'static + Send,
    {
        self.guarded(|| {
            let api_call = unsafe {
                sys::SteamAPI_ISteamTimeline_DoesEventRecordingExist(self.timeline, event.raw())
            };
            unsafe {
                register_call_result::<sys::SteamTimelineEventRecordingExists_t, _>(
                    &self._inner,
                    api_call,
                    move |v, io_error| {
                        callback(if io_error {
                            Err(SteamError::IOFailure)
                        } else {
                            Ok(TimelineEventRecordingExists {
                                event: TimelineEventHandle(v.m_ulEventID),
                                recording_exists: v.m_bRecordingExists,
                            })
                        })
                    },
                );
            }
            Ok(())
        })
    }

    /// Begins a game phase: the span of play Steam shows as one row per session.
    ///
    /// A phase runs until it is ended, until a new one starts, or until the game
    /// exits.
    pub fn start_game_phase(&self) {
        if self.disabled {
            return;
        }
        unsafe { sys::SteamAPI_ISteamTimeline_StartGamePhase(self.timeline) }
    }

    /// Ends the running game phase.
    pub fn end_game_phase(&self) {
        if self.disabled {
            return;
        }
        unsafe { sys::SteamAPI_ISteamTimeline_EndGamePhase(self.timeline) }
    }

    /// Gives the running phase a persistent id, which is what
    /// [`Timeline::open_overlay_to_game_phase`] and
    /// [`Timeline::does_game_phase_recording_exist`] later refer to.
    pub fn set_game_phase_id(&self, phase_id: &str) -> Result<(), SteamError> {
        self.guarded(|| {
            let phase_id = timeline_cstr(phase_id)?;
            unsafe {
                sys::SteamAPI_ISteamTimeline_SetGamePhaseID(self.timeline, phase_id.as_ptr())
            };
            Ok(())
        })
    }

    /// Asks Steam whether a recording exists for `phase_id`; the answer arrives
    /// as [`TimelineGamePhaseRecordingExists`].
    pub fn does_game_phase_recording_exist<F>(
        &self,
        phase_id: &str,
        callback: F,
    ) -> Result<(), SteamError>
    where
        F: FnOnce(Result<TimelineGamePhaseRecordingExists, SteamError>) + 'static + Send,
    {
        let phase_id = timeline_cstr(phase_id)?;
        self.guarded(|| {
            let api_call = unsafe {
                sys::SteamAPI_ISteamTimeline_DoesGamePhaseRecordingExist(
                    self.timeline,
                    phase_id.as_ptr(),
                )
            };
            unsafe {
                register_call_result::<sys::SteamTimelineGamePhaseRecordingExists_t, _>(
                    &self._inner,
                    api_call,
                    move |v, io_error| {
                        callback(if io_error {
                            Err(SteamError::IOFailure)
                        } else {
                            Ok(TimelineGamePhaseRecordingExists {
                                phase_id: fixed_phase_id(&v.m_rgchPhaseID),
                                recording: Duration::from_millis(v.m_ulRecordingMS),
                                longest_clip: Duration::from_millis(v.m_ulLongestClipMS),
                                clip_count: v.m_unClipCount,
                                screenshot_count: v.m_unScreenshotCount,
                            })
                        })
                    },
                );
            }
            Ok(())
        })
    }

    /// Adds a tagged, well-known fact about the running phase, with an icon.
    pub fn add_game_phase_tag(
        &self,
        name: &str,
        icon: &str,
        group: &str,
        priority: u32,
    ) -> Result<(), SteamError> {
        self.guarded(|| {
            let name = timeline_cstr(name)?;
            let icon = timeline_cstr(icon)?;
            let group = timeline_cstr(group)?;
            unsafe {
                sys::SteamAPI_ISteamTimeline_AddGamePhaseTag(
                    self.timeline,
                    name.as_ptr(),
                    icon.as_ptr(),
                    group.as_ptr(),
                    priority,
                )
            };
            Ok(())
        })
    }

    /// Sets a free-text attribute on the running phase. Steam keeps only the
    /// last value set for a group, so this is safe to call repeatedly.
    pub fn set_game_phase_attribute(
        &self,
        group: &str,
        value: &str,
        priority: u32,
    ) -> Result<(), SteamError> {
        self.guarded(|| {
            let group = timeline_cstr(group)?;
            let value = timeline_cstr(value)?;
            unsafe {
                sys::SteamAPI_ISteamTimeline_SetGamePhaseAttribute(
                    self.timeline,
                    group.as_ptr(),
                    value.as_ptr(),
                    priority,
                )
            };
            Ok(())
        })
    }

    /// Opens the Steam overlay at the phase with this id.
    pub fn open_overlay_to_game_phase(&self, phase_id: &str) -> Result<(), SteamError> {
        self.guarded(|| {
            let phase_id = timeline_cstr(phase_id)?;
            unsafe {
                sys::SteamAPI_ISteamTimeline_OpenOverlayToGamePhase(
                    self.timeline,
                    phase_id.as_ptr(),
                )
            };
            Ok(())
        })
    }

    /// Opens the Steam overlay at a timeline event.
    pub fn open_overlay_to_event(&self, event: TimelineEventHandle) {
        if self.disabled {
            return;
        }
        unsafe {
            sys::SteamAPI_ISteamTimeline_OpenOverlayToTimelineEvent(self.timeline, event.raw())
        }
    }

    /// Runs `body` only when the client's timeline API is usable.
    ///
    /// The infallible setters on this type answer `()` and silently skip when
    /// `disabled`, because that is what upstream's shape already allowed; the
    /// ones that must produce a value or a string instead report
    /// [`SteamError::Disabled`], so a caller never reads a successful-looking
    /// handle out of an API that was never called.
    fn guarded<T>(&self, body: impl FnOnce() -> Result<T, SteamError>) -> Result<T, SteamError> {
        if self.disabled {
            return Err(SteamError::Disabled);
        }
        body()
    }
}

/// A string reaching Steam as a C string: an embedded nul is a bad argument,
/// not a panic.
fn timeline_cstr(value: &str) -> Result<CString, SteamError> {
    CString::new(value).map_err(|_| SteamError::InvalidParameter)
}

/// Steam returns the phase id as a fixed char array; the name ends at the
/// first nul.
fn fixed_phase_id(raw: &[std::os::raw::c_char]) -> String {
    let bytes: Vec<u8> = raw
        .iter()
        .map(|b| *b as u8)
        .take_while(|b| *b != 0)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Handle Steam gives back for a timeline event. Valid for the process that
/// created it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimelineEventHandle(u64);

impl TimelineEventHandle {
    /// Rebuilds a handle from the raw value Steam reports.
    pub fn from_raw(raw: u64) -> Self {
        TimelineEventHandle(raw)
    }

    /// The raw event id, as passed to and returned by Steam.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

/// Answers [`Timeline::does_event_recording_exist`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimelineEventRecordingExists {
    pub event: TimelineEventHandle,
    pub recording_exists: bool,
}

/// Answers [`Timeline::does_game_phase_recording_exist`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineGamePhaseRecordingExists {
    /// The phase this answer is about, as Steam reported it back.
    pub phase_id: String,
    /// How much of the phase was recorded.
    pub recording: Duration,
    /// Length of the longest clip available for the phase.
    pub longest_clip: Duration,
    pub clip_count: u32,
    pub screenshot_count: u32,
}

impl_callback!(cb: SteamTimelineEventRecordingExists_t => TimelineEventRecordingExists {
    Self {
        event: TimelineEventHandle(cb.m_ulEventID),
        recording_exists: cb.m_bRecordingExists,
    }
});

impl_callback!(cb: SteamTimelineGamePhaseRecordingExists_t => TimelineGamePhaseRecordingExists {
    Self {
        phase_id: fixed_phase_id(&cb.m_rgchPhaseID),
        recording: Duration::from_millis(cb.m_ulRecordingMS),
        longest_clip: Duration::from_millis(cb.m_ulLongestClipMS),
        clip_count: cb.m_unClipCount,
        screenshot_count: cb.m_unScreenshotCount,
    }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_strings_reject_interior_nul_instead_of_panicking() {
        assert!(timeline_cstr("match-42").is_ok());
        assert!(matches!(
            timeline_cstr("phase\0other"),
            Err(SteamError::InvalidParameter)
        ));
    }

    #[test]
    fn phase_id_stops_at_the_first_nul_and_never_reads_past_the_array() {
        // A Steam fixed buffer: content, then nuls to the end of the array.
        let mut raw = [0 as std::os::raw::c_char; 64];
        for (slot, byte) in raw.iter_mut().zip(b"round-3".iter()) {
            *slot = *byte as std::os::raw::c_char;
        }
        assert_eq!(fixed_phase_id(&raw), "round-3");

        let empty = [0 as std::os::raw::c_char; 64];
        assert_eq!(fixed_phase_id(&empty), "");
    }

    #[test]
    fn event_handle_round_trips_through_the_raw_value_steam_uses() {
        for raw in [0u64, 1, u64::from(u32::MAX), u64::MAX] {
            assert_eq!(TimelineEventHandle::from_raw(raw).raw(), raw);
        }
        assert_ne!(
            TimelineEventHandle::from_raw(1),
            TimelineEventHandle::from_raw(2),
            "two events must not compare equal"
        );
    }

    #[test]
    fn clip_priority_maps_to_distinct_steam_variants() {
        use sys::ETimelineEventClipPriority as S;
        let mapped: Vec<S> = [
            TimelineEventClipPriority::None,
            TimelineEventClipPriority::Standard,
            TimelineEventClipPriority::Featured,
        ]
        .into_iter()
        .map(S::from)
        .collect();
        // Mapping must not collapse two levels onto the same Steam variant.
        assert_ne!(mapped[0], mapped[1]);
        assert_ne!(mapped[1], mapped[2]);
        assert_ne!(mapped[0], mapped[2]);
    }
}
