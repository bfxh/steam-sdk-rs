//! Reading the fixed-size `char[N]` fields Steam embeds directly in callback structs.
//!
//! These are arrays, not C strings Steam promised to terminate, and not UTF-8 promised either:
//! `m_rgchAchievementName` is `char m_rgchAchievementName[k_cchStatNameMax];		// name of the achievement`
//! (`isteamuserstats.h:358`, with `enum { k_cchStatNameMax = 128 };` at `:17`), and the networking
//! status carries `char m_debugMsg[ 256 ];` under the comment "Non-localized English language
//! status.  For diagnostic/debugging purposes only." (`isteamnetworkingutils.h:417-419`). Neither
//! field is annotated with a length the way `isteammatchmaking.h` annotates its out-strings, so a
//! reader that walks to the first NUL can leave the array entirely, and a reader that demands UTF-8 can
//! abort the callback it is running in.

use std::ffi::c_char;

/// Reads one of those arrays as text: stops at the first NUL, never past the array, and lossy rather
/// than fatal.
///
/// Lossy is a deliberate choice over `to_str().expect(..)`: an achievement name or a debug line is
/// data somebody else wrote (a game's own locale, a translated string, a stray byte), and these reads
/// happen inside `register_callback` dispatch, where a panic takes the host process down with it. The
/// array bound is the load-bearing part -- `CStr::from_ptr(array.as_ptr())` trusts a terminator exists,
/// and the headers above do not promise one.
pub(crate) fn fixed_string(bytes: &[c_char]) -> String {
    let len = bytes.iter().take_while(|c| **c != 0).count();
    // SAFETY: `len <= bytes.len()`, and every byte read lives in the same array `bytes` borrows, so
    // the slice is in bounds and aligned by construction; reading it cannot trap.
    let raw = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), len) };
    String::from_utf8_lossy(raw).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{c_char, fixed_string};

    #[test]
    fn stops_at_the_first_terminator() {
        let mut buf = [0 as c_char; 8];
        buf[0] = b'a' as c_char;
        buf[1] = b'b' as c_char;
        assert_eq!(fixed_string(&buf), "ab");
        let empty = [0 as c_char; 8];
        assert_eq!(fixed_string(&empty), "");
    }

    /// The case `CStr::from_ptr` gets wrong: with no terminator inside the array, that reader walks
    /// into whatever follows in the struct. This one cannot leave the slice it was handed.
    #[test]
    fn an_unterminated_array_is_bounded_by_the_array() {
        let full = [b'x' as c_char; 128];
        assert_eq!(fixed_string(&full).len(), 128);
        assert!(fixed_string(&full).chars().all(|c| c == 'x'));
    }

    /// Non-UTF-8 in a game-authored name must not abort the callback that received it.
    #[test]
    fn invalid_utf8_is_replaced_not_fatal() {
        let mut buf = [0 as c_char; 4];
        buf[0] = 0xffu8 as c_char;
        buf[1] = b'a' as c_char;
        assert_eq!(fixed_string(&buf), "\u{fffd}a");
    }
}
