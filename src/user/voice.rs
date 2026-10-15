//! The voice capture cluster: `Start`/`StopVoiceRecording`, `GetAvailableVoice`, `GetVoice`
//! and `DecompressVoice`.
//!
//! The header is unusually specific about the contract here, and the wrappers follow it rather
//! than a guessed one:
//!
//!   * the "uncompressed" arguments are a deprecated feature that most applications are told to
//!     leave NULL/zero (`isteamuser.h:96-97`), and asking for it is destructive -- "fetching
//!     uncompressed audio will cause GetVoice to discard any leftover compressed audio"
//!     (`isteamuser.h:90-92`). So none of these methods expose those arguments at all; they always
//!     pass `false`/null/0.
//!   * stopping does not end capture immediately ("the system will keep recording for a little bit
//!     after this function is called", `isteamuser.h:74-75`), which is why [`User::stop_voice_recording`]
//!     returns nothing and the reader keeps answering [`VoiceError::NotRecording`].
//!   * only `DecompressVoice` promises to report the size it needed when the buffer is short
//!     (`isteamuser.h:111-112`); `GetVoice` documents no such answer, so the two grow loops differ
//!     and say why.

use super::*;

/// The largest buffer this crate will **allocate on a caller's behalf**.
///
/// Steam's own suggestion for `DecompressVoice` is "start with a 20kb buffer and reallocate as
/// necessary" (`isteamuser.h:113`), so a grow loop has to stop somewhere: past this ceiling the
/// loop gives up and reports [`VoiceError::BufferTooSmall`] rather than spending unbounded memory
/// on an answer we cannot check. The caller-owned `..._into` methods are *not* capped by this
/// constant -- the caller already chose that size and we only clamp the length we report to Steam
/// into a `u32` (`voice_len`) -- so a 4 MiB buffer from a caller is honoured, not refused.
pub const MAX_VOICE_BYTES: usize = 1 << 20;

/// Where [`User::decode_voice`] starts, in bytes -- `isteamuser.h:113` names this figure.
const FIRST_DECODE_BUFFER: usize = 20 * 1024;

/// How many buffer sizes [`User::read_compressed_voice`] will try before it gives up.
const CAPTURE_ATTEMPTS: usize = 4;

/// Steam's answer to one of the voice calls, with the success case removed.
///
/// The codes are `EVoiceResult` (`steamclientpublic.h:151-163`, discriminants 0..9). That
/// declaration carries no comments, so each sentence below is the name read plainly rather than
/// quoted header text; the header's actual prose is cited where it constrains behaviour -- the
/// drain rule on [`User::stop_voice_recording`] and the buffer-size promise on
/// [`User::decode_voice`]. `k_EVoiceResultOK` is not here because it is the `Ok` of every
/// `Result` in this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum VoiceError {
    /// Voice is not initialised yet.
    #[error("voice is not initialised")]
    NotInitialized,
    /// Nothing is recording. After [`User::stop_voice_recording`] this is the signal that the
    /// buffer has finally drained (`isteamuser.h:75-76`).
    #[error("voice is not recording")]
    NotRecording,
    /// Recording, but no audio has been captured yet.
    #[error("no voice data available")]
    NoData,
    /// The buffer given was too small.
    #[error("voice buffer too small")]
    BufferTooSmall,
    /// The captured data is not usable.
    #[error("voice data corrupted")]
    DataCorrupted,
    /// The user is not allowed to use this feature.
    #[error("voice is restricted for this user")]
    Restricted,
    /// The codec in the buffer is not supported by this build.
    #[error("unsupported voice codec")]
    UnsupportedCodec,
    /// The listener's build is older than the voice data's format.
    #[error("voice receiver is out of date")]
    ReceiverOutOfDate,
    /// The listener did not answer.
    #[error("voice receiver did not answer")]
    ReceiverDidNotAnswer,
    /// Steam answered with a code this binding does not name.
    ///
    /// Not reachable with today's headers -- every code `EVoiceResult` declares is named above --
    /// and the wildcard arm cannot be dropped because the sys enum is `#[non_exhaustive]`. There
    /// is no test for it: fabricating a discriminant for the C enum behind `EVoiceResult` (measured:
    /// `#[repr(i32)]` on Windows, `#[repr(u32)]` on the other three platform bindings) is undefined
    /// behaviour either way, not a reproduction. The panic-site ledger is the guard, as it is for
    /// the other
    /// read-direction enums in this module's parent.
    #[error("unrecognised voice result")]
    Unknown,
}

impl VoiceError {
    /// Turns Steam's answer into a failure. `k_EVoiceResultOK` is *not* handled here: the callers
    /// check it before they build a value, so a success cannot be mistaken for an error.
    fn from_raw(raw: sys::EVoiceResult) -> Self {
        match raw {
            sys::EVoiceResult::k_EVoiceResultNotInitialized => Self::NotInitialized,
            sys::EVoiceResult::k_EVoiceResultNotRecording => Self::NotRecording,
            sys::EVoiceResult::k_EVoiceResultNoData => Self::NoData,
            sys::EVoiceResult::k_EVoiceResultBufferTooSmall => Self::BufferTooSmall,
            sys::EVoiceResult::k_EVoiceResultDataCorrupted => Self::DataCorrupted,
            sys::EVoiceResult::k_EVoiceResultRestricted => Self::Restricted,
            sys::EVoiceResult::k_EVoiceResultUnsupportedCodec => Self::UnsupportedCodec,
            sys::EVoiceResult::k_EVoiceResultReceiverOutOfDate => Self::ReceiverOutOfDate,
            sys::EVoiceResult::k_EVoiceResultReceiverDidNotAnswer => Self::ReceiverDidNotAnswer,
            _ => Self::Unknown,
        }
    }
}

/// The pointer Steam is handed for `bytes`.
///
/// A zero-length slice still must not be null: Steam takes a payload pointer even for an empty
/// chunk, and the same decision is already recorded in `RemoteStorage::write_file`, so an empty
/// input gets an aligned, never-dereferenced address instead.
fn source_pointer(bytes: &[u8]) -> *mut c_void {
    if bytes.is_empty() {
        std::ptr::NonNull::<u8>::dangling().as_ptr() as *mut c_void
    } else {
        bytes.as_ptr() as *mut c_void
    }
}

/// Steam's answer, or the failure it named.
fn check(raw: sys::EVoiceResult) -> Result<(), VoiceError> {
    if raw == sys::EVoiceResult::k_EVoiceResultOK {
        Ok(())
    } else {
        Err(VoiceError::from_raw(raw))
    }
}

/// The length Steam is told its buffer holds.
///
/// Clamped *down*, never up: an over-large `usize` would truncate to a smaller number and Steam
/// would write into less memory than it believed, so this is the one direction that is always safe
/// -- Steam is told no more than the buffer really has.
fn voice_len(requested: usize) -> u32 {
    requested.min(u32::MAX as usize) as u32
}

/// How many bytes Steam reported writing, believed only as far as the buffer allows.
///
/// The buffer's own length is the only bound worth applying here, and it is the tight one: what
/// Steam was told (`voice_len`, always `<= buffer.len()`) can never exceed it, so a count above
/// `capacity` would mean Steam ignored a size argument we already know it was handed. Trusting the
/// number further would hand the caller bytes nobody wrote.
pub(crate) fn written_bytes(written: u32, capacity: usize) -> usize {
    (written as usize).min(capacity)
}

/// The next capture buffer size when Steam says the current one was short.
///
/// `GetVoice` documents no "required size" answer, so this can only double -- and doubling from a
/// size Steam itself reported as available is what makes the loop converge.
fn grow_for_capture(current: usize) -> usize {
    current.saturating_mul(2).min(MAX_VOICE_BYTES)
}

/// The next decode buffer size, using the number Steam promised to write back
/// (`isteamuser.h:111-112`) as a floor, never as the whole answer: a reported size below the
/// current attempt would loop forever, and one above the cap is refused by the caller.
fn grow_for_decode(reported: usize, current: usize) -> usize {
    reported.max(current.saturating_mul(2)).min(MAX_VOICE_BYTES)
}

impl User {
    /// Starts capturing microphone data.
    ///
    /// Steam gives nothing back for this call (`void`), and there is no way to ask whether a
    /// microphone exists -- the failure shows up as [`VoiceError::NotInitialized`] or
    /// [`VoiceError::Restricted`] on the first read. Start here, then use
    /// [`User::read_compressed_voice`].
    pub fn start_voice_recording(&self) {
        unsafe { sys::SteamAPI_ISteamUser_StartVoiceRecording(self.user) }
    }

    /// Stops capturing.
    ///
    /// Deliberately not "stopped": "because people often release push-to-talk keys early, the
    /// system will keep recording for a little bit after this function is called", and the reader
    /// "should continue to be called until it returns k_eVoiceResultNotRecording"
    /// (`isteamuser.h:74-76`). So this returns nothing and the drain happens in
    /// [`User::read_compressed_voice`].
    pub fn stop_voice_recording(&self) {
        unsafe { sys::SteamAPI_ISteamUser_StopVoiceRecording(self.user) }
    }

    /// How many bytes of captured compressed audio `read_compressed_voice` could take right now.
    ///
    /// Only the compressed count is asked for: the two other arguments of `GetAvailableVoice` are
    /// named `_Deprecated` and the header tells most applications to leave them NULL/zero
    /// (`isteamuser.h:79-83`, `:96-97`).
    pub fn available_compressed_voice(&self) -> Result<usize, VoiceError> {
        let mut compressed: u32 = 0;
        let raw = unsafe {
            sys::SteamAPI_ISteamUser_GetAvailableVoice(
                self.user,
                &mut compressed as *mut u32,
                std::ptr::null_mut(),
                0,
            )
        };
        check(raw)?;
        Ok(compressed as usize)
    }

    /// Reads captured compressed audio into `buffer`, returning how many bytes Steam wrote.
    ///
    /// A caller-supplied buffer is the point: voice polling happens "at least once per frame, and
    /// preferably every few milliseconds" (`isteamuser.h:100-102`), so a per-call allocation would
    /// put a malloc on the hot path. A buffer that is too short is reported as
    /// [`VoiceError::BufferTooSmall`] and **no byte count is offered with it**: `GetVoice` documents
    /// no required-size answer the way `DecompressVoice` does (`isteamuser.h:111-112`), so
    /// [`User::read_compressed_voice`] can only grow by doubling rather than by the number Steam
    /// says it needed.
    pub fn read_compressed_voice_into(&self, buffer: &mut [u8]) -> Result<usize, VoiceError> {
        let declared = voice_len(buffer.len());
        let mut written: u32 = 0;
        let raw = unsafe {
            sys::SteamAPI_ISteamUser_GetVoice(
                self.user,
                true,
                buffer.as_mut_ptr() as *mut c_void,
                declared,
                &mut written as *mut u32,
                // The deprecated uncompressed half: never asked for, so nothing is discarded
                // (`isteamuser.h:90-92`).
                false,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
            )
        };
        check(raw)?;
        Ok(written_bytes(written, buffer.len()))
    }

    /// Reads captured compressed audio into a buffer this call allocates.
    ///
    /// Sized from [`User::available_compressed_voice`], and -- because `GetVoice` documents no
    /// "here is the size I needed" answer -- retried at most `CAPTURE_ATTEMPTS` times with a
    /// doubled buffer on [`VoiceError::BufferTooSmall`], stopping at [`MAX_VOICE_BYTES`]. Use
    /// [`User::read_compressed_voice_into`] if you already own a buffer.
    pub fn read_compressed_voice(&self) -> Result<Vec<u8>, VoiceError> {
        let wanted = self.available_compressed_voice()?.max(1);
        let mut size = wanted.min(MAX_VOICE_BYTES);
        let mut attempt = 0;
        loop {
            let mut buffer = vec![0u8; size];
            match self.read_compressed_voice_into(&mut buffer) {
                Ok(written) => {
                    buffer.truncate(written);
                    return Ok(buffer);
                }
                Err(VoiceError::BufferTooSmall) if attempt + 1 < CAPTURE_ATTEMPTS => {
                    let next = grow_for_capture(size);
                    if next == size {
                        return Err(VoiceError::BufferTooSmall);
                    }
                    size = next;
                    attempt += 1;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Decodes one compressed chunk into raw single-channel 16-bit PCM into `buffer`.
    ///
    /// `sample_rate` is passed to Steam unchanged -- it "supports any sample rate from 11025 to
    /// 48000" (`isteamuser.h:109-110`) and answers `UnsupportedCodec`/`Restricted` itself, so this
    /// side invents no range check it cannot enforce. [`User::voice_optimal_sample_rate`] gives the
    /// cheapest rate, and the header notes the best *quality* usually comes from your output
    /// device's own rate instead (`isteamuser.h:116-122`).
    pub fn decode_voice_into(
        &self,
        compressed: &[u8],
        buffer: &mut [u8],
        sample_rate: u32,
    ) -> Result<usize, VoiceError> {
        let declared = voice_len(buffer.len());
        let mut written: u32 = 0;
        let raw = unsafe {
            sys::SteamAPI_ISteamUser_DecompressVoice(
                self.user,
                source_pointer(compressed),
                voice_len(compressed.len()),
                buffer.as_mut_ptr() as *mut c_void,
                declared,
                &mut written as *mut u32,
                sample_rate,
            )
        };
        check(raw)?;
        Ok(written_bytes(written, buffer.len()))
    }

    /// Decodes a compressed chunk into PCM, allocating and growing the buffer as Steam directs.
    ///
    /// This is the one place the header hands back a number to grow by: "if the output buffer is
    /// not large enough, then *nBytesWritten will be set to the required buffer size, and
    /// k_EVoiceResultBufferTooSmall is returned. It is suggested to start with a 20kb buffer and
    /// reallocate as necessary" (`isteamuser.h:111-113`). The loop starts there, believes that
    /// number as a floor only (`grow_for_decode`), and refuses to exceed [`MAX_VOICE_BYTES`] --
    /// past the cap the refusal is reported, not an unbounded allocation.
    ///
    /// It deliberately re-issues the FFI call instead of sitting on
    /// [`User::decode_voice_into`]: that helper discards the byte count, and on
    /// `BufferTooSmall` the count *is* the required size -- the only input the next attempt has.
    pub fn decode_voice(&self, compressed: &[u8], sample_rate: u32) -> Result<Vec<u8>, VoiceError> {
        let mut size = FIRST_DECODE_BUFFER.min(MAX_VOICE_BYTES);
        loop {
            let mut buffer = vec![0u8; size];
            let mut written: u32 = 0;
            let raw = unsafe {
                sys::SteamAPI_ISteamUser_DecompressVoice(
                    self.user,
                    source_pointer(compressed),
                    voice_len(compressed.len()),
                    buffer.as_mut_ptr() as *mut c_void,
                    voice_len(size),
                    &mut written as *mut u32,
                    sample_rate,
                )
            };
            if raw == sys::EVoiceResult::k_EVoiceResultBufferTooSmall {
                let reported = written as usize;
                let next = grow_for_decode(reported, size);
                if next == size {
                    // Already at the cap: growing is no longer possible, so this is a refusal
                    // rather than an unbounded allocation.
                    return Err(VoiceError::BufferTooSmall);
                }
                size = next;
                continue;
            }
            check(raw)?;
            let kept = written_bytes(written, buffer.len());
            buffer.truncate(kept);
            return Ok(buffer);
        }
    }
}

#[cfg(test)]
mod voice_tests {
    use super::*;

    #[test]
    fn every_declared_voice_code_lands_on_its_own_error() {
        use sys::EVoiceResult as S;
        let named = [
            (S::k_EVoiceResultNotInitialized, VoiceError::NotInitialized),
            (S::k_EVoiceResultNotRecording, VoiceError::NotRecording),
            (S::k_EVoiceResultNoData, VoiceError::NoData),
            (S::k_EVoiceResultBufferTooSmall, VoiceError::BufferTooSmall),
            (S::k_EVoiceResultDataCorrupted, VoiceError::DataCorrupted),
            (S::k_EVoiceResultRestricted, VoiceError::Restricted),
            (
                S::k_EVoiceResultUnsupportedCodec,
                VoiceError::UnsupportedCodec,
            ),
            (
                S::k_EVoiceResultReceiverOutOfDate,
                VoiceError::ReceiverOutOfDate,
            ),
            (
                S::k_EVoiceResultReceiverDidNotAnswer,
                VoiceError::ReceiverDidNotAnswer,
            ),
        ];
        for (raw, want) in named {
            assert_eq!(VoiceError::from_raw(raw), want);
            assert!(check(raw).is_err(), "{raw:?} must not read as success");
        }
        for i in 0..named.len() {
            for j in (i + 1)..named.len() {
                assert_ne!(named[i].1, named[j].1);
            }
        }
        // The one code that is not an error is the one this mapping refuses to carry.
        assert!(check(S::k_EVoiceResultOK).is_ok());
    }

    #[test]
    fn the_length_steam_is_told_is_clamped_down_never_up() {
        assert_eq!(voice_len(0), 0);
        assert_eq!(voice_len(1), 1);
        assert_eq!(voice_len(4096), 4096);
        assert_eq!(voice_len(u32::MAX as usize), u32::MAX);
        // Truncating instead of clamping would tell Steam about more room than the buffer has.
        assert_eq!(voice_len(usize::MAX), u32::MAX);
        if usize::BITS > 32 {
            assert_eq!(voice_len(u32::MAX as usize + 1), u32::MAX);
        }
        for requested in [1usize, 1024, MAX_VOICE_BYTES] {
            assert!((voice_len(requested) as usize) <= requested);
        }
    }

    #[test]
    fn a_write_count_is_believed_only_as_far_as_the_buffer_allows() {
        assert_eq!(written_bytes(5, 10), 5);
        assert_eq!(written_bytes(8, 8), 8);
        assert_eq!(written_bytes(100, 40), 40);
        assert_eq!(written_bytes(u32::MAX, 64), 64);
        assert_eq!(written_bytes(0, 0), 0);
        assert_eq!(written_bytes(4, 2), 2);
        // The bound is the buffer itself, and `voice_len` never tells Steam more than that, so
        // there is no direction in which the two bounds could disagree.
        for capacity in [0usize, 1, 4096, MAX_VOICE_BYTES] {
            assert!(written_bytes(u32::MAX, capacity) <= capacity);
            let declared = voice_len(capacity) as usize;
            assert_eq!(written_bytes(voice_len(capacity), capacity), declared);
        }
    }

    #[test]
    fn both_grow_loops_stop_at_the_registered_cap() {
        assert!(FIRST_DECODE_BUFFER <= MAX_VOICE_BYTES);
        assert!(FIRST_DECODE_BUFFER < MAX_VOICE_BYTES);
        let mut size = FIRST_DECODE_BUFFER;
        for _ in 0..64 {
            size = grow_for_decode(size, size);
        }
        assert_eq!(size, MAX_VOICE_BYTES);
        // Doubling from the smallest possible buffer still lands on the cap, never past it.
        let mut capture = 1usize;
        for _ in 0..64 {
            capture = grow_for_capture(capture);
        }
        assert_eq!(capture, MAX_VOICE_BYTES);
        // Steam's number is a floor, not an order: a smaller report must never shrink the attempt,
        // or the loop would ask for the buffer it just refused forever.
        assert_eq!(grow_for_decode(1, 4096), 8192);
        assert_eq!(grow_for_decode(9000, 4096), 9000);
        assert_eq!(grow_for_capture(MAX_VOICE_BYTES), MAX_VOICE_BYTES);
    }
}
