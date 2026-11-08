//! The crate-internal bridge for `SteamParamStringArray_t`: it owns the `into_raw` allocations
//! Steam reads by pointer, hands the array back on a halfway failure instead of leaking it,
//! and frees the whole set on drop. Named by `src/ugc/update.rs`, so the parent re-exports it
//! with `pub(crate)` rather than making callers reach into this module.

use super::*;

pub(crate) struct SteamParamStringArray(Vec<*mut c_char>);

impl Drop for SteamParamStringArray {
    fn drop(&mut self) {
        for c_string in &self.0 {
            unsafe { drop(CString::from_raw(*c_string)) };
        }
    }
}

impl SteamParamStringArray {
    /// Builds the array, or `None` when an entry cannot become a C string.
    ///
    /// The entries are `into_raw` allocations that Steam reads by pointer, so a failure halfway
    /// through must hand back what was already taken. The old version `expect`ed instead, which
    /// did two bad things at once: it panicked inside the caller's process, and the strings
    /// converted before the bad one had no owner left to free them.
    pub(crate) fn new<S: AsRef<str>>(vec: &[S]) -> Option<SteamParamStringArray> {
        let mut raw = Vec::with_capacity(vec.len());
        for s in vec {
            match CString::new(s.as_ref()) {
                Ok(value) => raw.push(value.into_raw()),
                Err(_) => {
                    for taken in raw {
                        drop(unsafe { CString::from_raw(taken) });
                    }
                    return None;
                }
            }
        }
        Some(SteamParamStringArray(raw))
    }

    pub(crate) fn as_raw(&mut self) -> sys::SteamParamStringArray_t {
        sys::SteamParamStringArray_t {
            m_nNumStrings: self.0.len() as i32,
            m_ppStrings: self.0.as_mut_ptr().cast(),
        }
    }
}
