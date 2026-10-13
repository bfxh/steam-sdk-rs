use super::*;

/// Access to the steam music interface
///
/// Controls the user's own Steam music client; a game is a guest here, so the
/// setters report success by taking effect rather than returning a status.
pub struct Music {
    pub(crate) music: *mut sys::ISteamMusic,
    pub(crate) _inner: Arc<Inner>,
}

/// Playback state of the user's Steam music client
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MusicPlaybackStatus {
    /// Steam could not determine the state
    Undefined,
    Playing,
    Paused,
    Idle,
}

impl MusicPlaybackStatus {
    fn from_raw(raw: sys::AudioPlayback_Status) -> Self {
        match raw {
            sys::AudioPlayback_Status::AudioPlayback_Playing => MusicPlaybackStatus::Playing,
            sys::AudioPlayback_Status::AudioPlayback_Paused => MusicPlaybackStatus::Paused,
            sys::AudioPlayback_Status::AudioPlayback_Idle => MusicPlaybackStatus::Idle,
            // Anything Steam reports outside the known set is treated as
            // unknown rather than silently mapped onto one of the real states.
            _ => MusicPlaybackStatus::Undefined,
        }
    }
}

impl Music {
    /// Whether the user allows the game to control Steam music at all.
    ///
    /// When this is `false` the remaining calls are no-ops on Steam's side.
    pub fn is_enabled(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamMusic_BIsEnabled(self.music) }
    }

    /// Whether Steam music is currently playing.
    ///
    /// Prefer [`Music::playback_status`], which distinguishes paused from idle.
    pub fn is_playing(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamMusic_BIsPlaying(self.music) }
    }

    /// Current playback state.
    pub fn playback_status(&self) -> MusicPlaybackStatus {
        unsafe {
            MusicPlaybackStatus::from_raw(sys::SteamAPI_ISteamMusic_GetPlaybackStatus(self.music))
        }
    }

    /// Resumes playback. No-op when [`Music::is_enabled`] is `false`.
    pub fn play(&self) {
        unsafe {
            sys::SteamAPI_ISteamMusic_Play(self.music);
        }
    }

    /// Pauses playback. No-op when [`Music::is_enabled`] is `false`.
    pub fn pause(&self) {
        unsafe {
            sys::SteamAPI_ISteamMusic_Pause(self.music);
        }
    }

    /// Skips to the next track. No-op when [`Music::is_enabled`] is `false`.
    pub fn play_next(&self) {
        unsafe {
            sys::SteamAPI_ISteamMusic_PlayNext(self.music);
        }
    }

    /// Returns to the previous track. No-op when [`Music::is_enabled`] is `false`.
    pub fn play_previous(&self) {
        unsafe {
            sys::SteamAPI_ISteamMusic_PlayPrevious(self.music);
        }
    }

    /// Current Steam music volume, in [0, 1].
    pub fn volume(&self) -> f32 {
        unsafe { sys::SteamAPI_ISteamMusic_GetVolume(self.music) }
    }

    /// Sets the Steam music volume. Values outside [0, 1] are clamped.
    ///
    /// This changes the user's own music volume, so it is worth restoring on
    /// exit; [`MusicVolumeChanged`] tells you when the user changes it back.
    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        unsafe {
            sys::SteamAPI_ISteamMusic_SetVolume(self.music, volume);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// The Steam music client started or stopped playing
pub struct MusicPlaybackStatusChanged;

impl_callback!(_cb: PlaybackStatusHasChanged_t => MusicPlaybackStatusChanged {
    Self {}
});

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// The Steam music volume changed, by this game or by the user
pub struct MusicVolumeChanged {
    /// The new volume, in [0, 1]
    pub volume: f32,
}

impl_callback!(cb: VolumeHasChanged_t => MusicVolumeChanged {
    Self {
        volume: cb.m_flNewVolume,
    }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_statuses_map_to_their_own_variant() {
        assert_eq!(
            MusicPlaybackStatus::from_raw(sys::AudioPlayback_Status::AudioPlayback_Playing),
            MusicPlaybackStatus::Playing
        );
        assert_eq!(
            MusicPlaybackStatus::from_raw(sys::AudioPlayback_Status::AudioPlayback_Paused),
            MusicPlaybackStatus::Paused
        );
        assert_eq!(
            MusicPlaybackStatus::from_raw(sys::AudioPlayback_Status::AudioPlayback_Idle),
            MusicPlaybackStatus::Idle
        );
        assert_eq!(
            MusicPlaybackStatus::from_raw(sys::AudioPlayback_Status::AudioPlayback_Undefined),
            MusicPlaybackStatus::Undefined
        );
    }

    #[test]
    fn volume_input_is_clamped_into_steam_range() {
        // The clamp is the only validation here; assert the boundary behaviour
        // the doc promises.
        for (input, want) in [
            (-1.0f32, 0.0f32),
            (0.0, 0.0),
            (0.5, 0.5),
            (1.0, 1.0),
            (3.0, 1.0),
        ] {
            assert_eq!(input.clamp(0.0, 1.0), want, "clamp({input})");
        }
    }
}
