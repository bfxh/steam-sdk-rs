use super::*;

use std::os::raw::c_char;

mod browser;
mod callbacks;
mod keys;

pub use browser::*;
pub use callbacks::*;
pub use keys::*;

/// Access to the steam HTML surface interface
///
/// Unlike the other interfaces here, this one is message-driven: almost every
/// call is fire-and-forget and its result arrives as a callback
/// ([`HtmlBrowserReady`], [`HtmlFinishedRequest`], ...), so a game must keep
/// running the callback loop for the browser to make progress.
pub struct HtmlSurface {
    pub(crate) html: *mut sys::ISteamHTMLSurface,
    pub(crate) _inner: Arc<Inner>,
}

fn cstr(value: &str) -> Result<CString, SteamError> {
    CString::new(value).map_err(|_| SteamError::InvalidParameter)
}

/// Copies a NUL-terminated Steam string into an owned `String`.
///
/// Only valid for the duration of the callback, which is the sole caller: the
/// pointer Steam hands a callback message is not kept.
unsafe fn owned(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

impl HtmlSurface {
    /// Initializes the HTML surface subsystem.
    ///
    /// Required before [`HtmlSurface::create_browser`]; Steam keeps it process
    /// global, so the matching [`HtmlSurface::shutdown`] belongs on teardown.
    pub fn init(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamHTMLSurface_Init(self.html) }
    }

    /// Shuts the HTML surface subsystem down.
    pub fn shutdown(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamHTMLSurface_Shutdown(self.html) }
    }

    /// Creates a browser.
    ///
    /// Steam allocates the instance asynchronously and reports its handle in the
    /// `HTML_BrowserReady` message, so the callback receives the ready-to-use
    /// [`Browser`] rather than a placeholder.
    pub fn create_browser<F>(
        &self,
        user_agent: &str,
        user_css: &str,
        callback: F,
    ) -> Result<(), SteamError>
    where
        F: FnOnce(Result<Browser, SteamError>) + 'static + Send,
    {
        let user_agent = cstr(user_agent)?;
        let user_css = cstr(user_css)?;
        // Handle 0 stands in until Steam reports the real one; it is forgotten on
        // the failure path so Drop never asks Steam to remove a browser 0.
        let placeholder = Browser {
            html: self.html,
            handle: 0,
        };
        unsafe {
            let api_call = sys::SteamAPI_ISteamHTMLSurface_CreateBrowser(
                self.html,
                user_agent.as_ptr(),
                user_css.as_ptr(),
            );
            register_call_result::<sys::HTML_BrowserReady_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        std::mem::forget(placeholder);
                        Err(SteamError::IOFailure)
                    } else {
                        Ok(Browser {
                            handle: v.unBrowserHandle,
                            ..placeholder
                        })
                    });
                },
            );
        }
        Ok(())
    }

    /// Wraps a Steam-reported browser handle so its methods can be called.
    ///
    /// Use this to obtain a [`Browser`] from an [`HtmlBrowserReady`] message you
    /// have already received. The returned value removes the browser when
    /// dropped; call [`HtmlSurface::remove_browser`] instead if you just want to
    /// release a handle you are holding.
    pub fn browser_from_handle(&self, handle: sys::HHTMLBrowser) -> Browser {
        Browser {
            html: self.html,
            handle,
        }
    }

    /// Removes a browser by handle, for callers that do not hold a [`Browser`].
    pub fn remove_browser(&self, handle: sys::HHTMLBrowser) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_RemoveBrowser(self.html, handle);
        }
    }

    /// Sets a cookie for a host. Steam applies it to later requests.
    #[allow(clippy::too_many_arguments)]
    pub fn set_cookie(
        &self,
        hostname: &str,
        key: &str,
        value: &str,
        path: &str,
        expires: u32,
        secure: bool,
        http_only: bool,
    ) -> Result<(), SteamError> {
        let hostname = cstr(hostname)?;
        let key = cstr(key)?;
        let value = cstr(value)?;
        let path = cstr(path)?;
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_SetCookie(
                self.html,
                hostname.as_ptr(),
                key.as_ptr(),
                value.as_ptr(),
                path.as_ptr(),
                expires,
                secure,
                http_only,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_buttons_map_to_distinct_variants() {
        let all = [
            HtmlMouseButton::Left,
            HtmlMouseButton::Right,
            HtmlMouseButton::Middle,
        ];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                let (a, b) = (all[i], all[j]);
                assert_ne!(a.as_raw(), b.as_raw(), "{a:?}/{b:?} collide");
            }
        }
    }

    #[test]
    fn key_modifiers_map_onto_existing_steam_variants() {
        // The whole reason HtmlKeyModifier is single-choice: every mapping must
        // land on a discriminant Steam actually defines.
        assert_eq!(
            HtmlKeyModifier::None.as_raw(),
            sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_None
        );
        assert_eq!(
            HtmlKeyModifier::Alt.as_raw(),
            sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_AltDown
        );
        assert_eq!(
            HtmlKeyModifier::Ctrl.as_raw(),
            sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_CtrlDown
        );
        assert_eq!(
            HtmlKeyModifier::Shift.as_raw(),
            sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_ShiftDown
        );
        assert_eq!(HtmlKeyModifier::default(), HtmlKeyModifier::None);
    }

    #[test]
    fn strings_reject_interior_nul_before_reaching_steam() {
        assert!(cstr("https://example.com").is_ok());
        assert!(matches!(cstr("a\0b"), Err(SteamError::InvalidParameter)));
    }

    #[test]
    fn null_string_pointers_become_empty_not_panicked() {
        assert_eq!(unsafe { owned(std::ptr::null()) }, "");
        let text = CString::new("title").unwrap();
        assert_eq!(unsafe { owned(text.as_ptr()) }, "title");
    }
}
