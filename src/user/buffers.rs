//! The two calls whose whole contract is an out-parameter: the account's local storage folder,
//! which `isteamuser.h:67-69` describes as "the local storage folder for current Steam account to
//! write application data", and the finished encrypted app ticket (`h:173`).
//!
//! Both were held back in `docs/SuperSteam-Design.md` §6.1d for exactly one question -- who
//! allocates, how big, and what does a short buffer answer -- and this module answers that from
//! the header rather than from the signature. What it does not do is guess: where the header is
//! silent about a failure mode, the wrapper detects the condition on its own side and refuses
//! instead of returning something plausible.

use super::*;

/// The buffer [`User::user_data_folder`] reads into.
///
/// `GetUserDataFolder` returns a plain `bool` and the header never says what `false` means
/// (`isteamuser.h:67-69`), so the size is chosen to make a did-not-fit answer unreachable in
/// practice,
/// and the case is still detected (see `terminated_prefix`) rather than assumed away: 32 KiB is
/// more than a hundred times Windows' own 260-character path limit, and the example Steam gives
/// (`"C:\Progam Files\Steam\userdata\<SteamID>\<AppID>\local"`, `isteamuser.h:68`) is a fraction of
/// it.
const USER_DATA_FOLDER_BYTES: usize = 32 * 1024;

/// The largest app ticket this crate will allocate on a caller's behalf.
///
/// A ticket is a small encrypted blob, and the probe round of `GetEncryptedAppTicket` reports the
/// size it needs (`isteamuser.h:170-172`: "you can pass pTicket=NULL and cbMaxTicket=0").
/// Reporting an absurd number is a failure mode the header
/// does not rule out, so the loop refuses above this figure instead of spending unbounded memory
/// on a length nobody can check.
const MAX_TICKET_BYTES: usize = 1 << 20;

/// The path Steam filled into `buffer`, or nothing if it never ended.
///
/// The detection is ours, not Steam's: a buffer with no NUL anywhere in it means the path was cut
/// at the capacity, and handing back those bytes would be a *different existing-looking* path --
/// worse than an error, because a game would write its saves somewhere it did not choose.
fn terminated_prefix(buffer: &[c_char]) -> Option<String> {
    if !buffer.iter().any(|byte| *byte == 0) {
        return None;
    }
    Some(crate::ugc::steam_out_string(buffer))
}

/// `cubBuffer` is an `int`, so a length that does not fit is refused rather than truncated --
/// the same rule as `cloud_chunk_len`: Steam must not be told about room it cannot have.
fn folder_arg_len(len: usize) -> Option<i32> {
    i32::try_from(len).ok()
}

/// How large a buffer the ticket's second round needs, from the size Steam reported.
///
/// Zero means "no ticket is available" (`isteamuser.h:168-172`), which is not a failure of the caller
/// but of timing -- the request callback has not delivered readiness yet.
fn ticket_buffer(reported: u32) -> Result<usize, SteamError> {
    if reported == 0 {
        return Err(SteamError::InvalidState);
    }
    let needed = reported as usize;
    if needed > MAX_TICKET_BYTES {
        return Err(SteamError::InvalidParameter);
    }
    Ok(needed)
}

impl User {
    /// The local storage folder for the current Steam account, for save games and config.
    ///
    /// `GetUserDataFolder` is documented as "the local storage folder for current Steam account to
    /// write application data, e.g. save games, configs etc." and the example is
    /// `"C:\Progam Files\Steam\userdata\<SteamID>\<AppID>\local"` (`isteamuser.h:67-68`). Two
    /// things can go wrong and both come back as errors, not as a quiet string:
    ///
    ///   * Steam returns `false`, whose meaning the header never states -- so it is reported as
    ///     [`SteamError::InvalidState`], and nothing is inferred from it beyond the grant being
    ///     absent.
    ///   * the path fills the whole buffer without terminating, which would be a truncated path.
    ///     That is detected here (`terminated_prefix`) because a cut path still *reads* like a
    ///     path.
    ///
    /// The bytes are decoded lossily, matching every other string this crate reads out of a fixed
    /// Steam buffer: an undecodable byte becomes U+FFFD rather than failing the call. A byte string
    /// that is not valid UTF-8 is therefore not round-trip exact here.
    pub fn user_data_folder(&self) -> Result<String, SteamError> {
        let mut buffer = vec![0 as c_char; USER_DATA_FOLDER_BYTES];
        let len = folder_arg_len(buffer.len()).ok_or(SteamError::InvalidParameter)?;
        let ok = unsafe {
            sys::SteamAPI_ISteamUser_GetUserDataFolder(self.user, buffer.as_mut_ptr(), len)
        };
        if !ok {
            return Err(SteamError::InvalidState);
        }
        terminated_prefix(&buffer).ok_or(SteamError::InvalidParameter)
    }

    /// The finished encrypted app ticket, or an error saying it is not here yet.
    ///
    /// This is the second half of [`User::request_encrypted_app_ticket`], and the two-pass
    /// protocol is Steam's own design, not a workaround invented here: "If no ticket is available,
    /// or your buffer is too small, returns false. Upon exit, *pcbTicket will be either the size of
    /// the ticket copied into your buffer (if true was returned), or the size needed (if false was
    /// returned). To determine the proper size of the ticket, you can pass pTicket=NULL and
    /// cbMaxTicket=0; if a ticket is available, *pcbTicket will contain the size needed, otherwise
    /// it will be zero" (`isteamuser.h:167-172`).
    ///
    /// So: probe with NULL/0, allocate exactly what the probe reported, then fetch. A probe that
    /// reports zero is [`SteamError::InvalidState`] (nothing is available yet); a reported size above
    /// `MAX_TICKET_BYTES` is [`SteamError::InvalidParameter`] rather than a huge allocation; and
    /// a second round that still refuses after being given its own number is
    /// [`SteamError::InvalidState`] -- the protocol did not converge, which is Steam's problem to
    /// explain, not a licence to retry forever.
    pub fn encrypted_app_ticket(&self) -> Result<Vec<u8>, SteamError> {
        let mut probed: u32 = 0;
        unsafe {
            sys::SteamAPI_ISteamUser_GetEncryptedAppTicket(
                self.user,
                std::ptr::null_mut(),
                0,
                &mut probed as *mut u32,
            )
        };
        let needed = ticket_buffer(probed)?;
        let mut buffer = vec![0u8; needed];
        let mut written: u32 = 0;
        let ok = unsafe {
            sys::SteamAPI_ISteamUser_GetEncryptedAppTicket(
                self.user,
                buffer.as_mut_ptr() as *mut c_void,
                folder_arg_len(needed).ok_or(SteamError::InvalidParameter)?,
                &mut written as *mut u32,
            )
        };
        if !ok {
            return Err(SteamError::InvalidState);
        }
        // The count is believed only as far as the buffer really is, exactly as in the voice
        // cluster: a larger number would hand back bytes nobody wrote.
        let kept = crate::utils::written_bytes(written, buffer.len());
        buffer.truncate(kept);
        Ok(buffer)
    }
}

#[cfg(test)]
mod buffer_tests {
    use super::*;

    fn chars(text: &[u8]) -> Vec<c_char> {
        text.iter().map(|byte| *byte as c_char).collect()
    }

    #[test]
    fn a_terminated_buffer_is_read_as_the_path_and_a_truncated_one_is_refused() {
        // Terminated: read up to the NUL, not up to the capacity.
        let mut buffer = chars(b"C:\\Steam\\userdata\\1234\\456\\local");
        buffer.push(0);
        buffer.extend(chars(b"garbage past the terminator that must not be read"));
        assert_eq!(
            terminated_prefix(&buffer).unwrap(),
            r"C:\Steam\userdata\1234\456\local"
        );

        // A single byte plus the NUL, and an empty-but-terminated buffer.
        buffer = chars(b"x");
        buffer.push(0);
        assert_eq!(terminated_prefix(&buffer).unwrap(), "x");
        assert_eq!(terminated_prefix(&[0]).unwrap(), "");

        // No NUL anywhere: the path ran out of room. Refuse -- the cut would still look valid.
        let full = chars(&[b'a'; 8]);
        assert_eq!(terminated_prefix(&full), None);
        assert_eq!(terminated_prefix(&[]), None);
    }

    #[test]
    fn an_unencodable_lossy_path_still_survives_instead_of_failing_the_call() {
        let mut buffer = chars(&[0x63, 0x3a, 0x5c, 0xff, 0x00]);
        let read = terminated_prefix(&mut buffer).unwrap();
        assert!(
            read.contains('\u{fffd}'),
            "undecodable byte must be replaced"
        );
    }

    #[test]
    fn the_int_length_argument_refuses_what_it_cannot_carry() {
        assert_eq!(folder_arg_len(0), Some(0));
        assert_eq!(folder_arg_len(USER_DATA_FOLDER_BYTES), Some(32768));
        assert_eq!(folder_arg_len(i32::MAX as usize), Some(i32::MAX));
        // Truncation here would tell Steam about room the buffer does not have.
        if usize::BITS > 32 {
            assert_eq!(folder_arg_len(i32::MAX as usize + 1), None);
        }
        assert_eq!(folder_arg_len(usize::MAX), None);
    }

    #[test]
    fn a_ticket_probe_of_zero_is_not_granted_and_an_absurd_one_is_refused() {
        assert_eq!(ticket_buffer(0), Err(SteamError::InvalidState));
        assert_eq!(ticket_buffer(1), Ok(1));
        assert_eq!(ticket_buffer(4096), Ok(4096));
        assert_eq!(ticket_buffer(MAX_TICKET_BYTES as u32), Ok(MAX_TICKET_BYTES));
        assert_eq!(
            ticket_buffer(MAX_TICKET_BYTES as u32 + 1),
            Err(SteamError::InvalidParameter)
        );
        assert_eq!(ticket_buffer(u32::MAX), Err(SteamError::InvalidParameter));
        assert!(MAX_TICKET_BYTES <= i32::MAX as usize);
    }
}
