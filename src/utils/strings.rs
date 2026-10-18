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

use std::ffi::{c_char, CString};

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

/// The other direction: the `CString` to hand Steam for a string the caller supplied.
///
/// `CString::new(value).unwrap()` -- the shape most of this crate's setters used -- fails on exactly
/// one input, an interior NUL, and the input is the caller's, so safe code outside the crate could
/// panic the crate's frame. Cutting at the first NUL is not a policy invented here either: every one of
/// these parameters is a `const char *` in the header, so the NUL already ended the string as far as
/// Steam was concerned and nothing observable is lost by honouring it in the wrapper.
///
/// Where a setter has a real error channel it should answer `Err(InvalidParameter)` instead -- that is
/// what `cstr()` in `src/http.rs` and `src/html_surface/mod.rs` do. This helper is for the
/// fire-and-forget ones, which have no channel and so must not panic.
pub(crate) fn c_string_prefix(value: &str) -> CString {
    // `split` always yields at least one piece, and the first piece has no NUL in it by construction.
    let head = value.split('\0').next().unwrap_or(value);
    CString::new(head).unwrap_or_default()
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

#[cfg(test)]
mod outgoing_tests {
    //! The write direction only: which bytes Steam is handed for a caller's string. Reaching the FFI
    //! needs a live client, so this judges the rule, not the wiring.
    use super::c_string_prefix;
    use std::ffi::CString;

    #[test]
    fn a_plain_string_goes_through_byte_for_byte() {
        assert_eq!(c_string_prefix("MyGame"), CString::new("MyGame").unwrap());
        assert_eq!(c_string_prefix(""), CString::new("").unwrap());
    }

    /// The panic this replaces: `CString::new("a\0b").unwrap()` aborts the caller's frame over a value
    /// the caller chose. A `const char *` ends at that NUL anyway, so the prefix is all Steam would
    /// have read.
    #[test]
    fn an_interior_nul_ends_the_string_instead_of_panicking() {
        assert_eq!(c_string_prefix("a\0b"), CString::new("a").unwrap());
        assert_eq!(c_string_prefix("\0first"), CString::new("").unwrap());
        assert_eq!(c_string_prefix("clean\0"), CString::new("clean").unwrap());
    }

    #[test]
    fn nul_free_input_keeps_every_byte() {
        let long = "x".repeat(4096);
        assert_eq!(c_string_prefix(&long).to_string_lossy().len(), 4096);
    }
}
