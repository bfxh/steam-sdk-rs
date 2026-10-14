use super::*;

use std::os::raw::c_char;

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

/// A single browser instance.
///
/// Dropping it removes the browser on the Steam side, matching how
/// [`crate::InventoryResult`] releases its handle.
pub struct Browser {
    html: *mut sys::ISteamHTMLSurface,
    handle: sys::HHTMLBrowser,
}

// Steam serialises these calls internally, and the crate already asserts this
// for handle types holding a raw interface pointer (see UpdateWatchHandle), so
// a Browser can cross threads the same way.
unsafe impl Send for Browser {}
unsafe impl Sync for Browser {}

/// Which mouse button an HTML input event refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HtmlMouseButton {
    Left,
    Right,
    Middle,
}

impl HtmlMouseButton {
    fn as_raw(self) -> sys::ISteamHTMLSurface_EHTMLMouseButton {
        match self {
            HtmlMouseButton::Left => sys::ISteamHTMLSurface_EHTMLMouseButton::eHTMLMouseButton_Left,
            HtmlMouseButton::Right => {
                sys::ISteamHTMLSurface_EHTMLMouseButton::eHTMLMouseButton_Right
            }
            HtmlMouseButton::Middle => {
                sys::ISteamHTMLSurface_EHTMLMouseButton::eHTMLMouseButton_Middle
            }
        }
    }
}

/// A keyboard modifier for HTML input events.
///
/// Deliberately a single choice, not a set: Steam's generated parameter type is
/// a `#[repr(i32)]` enum whose variants happen to carry bit values (1, 2, 4),
/// so a combination such as Shift+Ctrl has no valid discriminant in that type.
/// Building one by transmuting would create an invalid enum value, which is
/// undefined behaviour rather than merely a wrong number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HtmlKeyModifier {
    #[default]
    None,
    Alt,
    Ctrl,
    Shift,
}

impl HtmlKeyModifier {
    fn as_raw(self) -> sys::ISteamHTMLSurface_EHTMLKeyModifiers {
        match self {
            HtmlKeyModifier::None => {
                sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_None
            }
            HtmlKeyModifier::Alt => {
                sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_AltDown
            }
            HtmlKeyModifier::Ctrl => {
                sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_CtrlDown
            }
            HtmlKeyModifier::Shift => {
                sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_ShiftDown
            }
        }
    }
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

impl Browser {
    fn handle(&self) -> sys::HHTMLBrowser {
        self.handle
    }

    /// The Steam-side browser handle, for use through the `raw-bindings` escape hatch.
    pub fn raw(&self) -> sys::HHTMLBrowser {
        self.handle
    }

    /// Navigates to a URL. `post_data` is sent as the request body when present.
    pub fn load_url(&self, url: &str, post_data: Option<&str>) -> Result<(), SteamError> {
        let url = cstr(url)?;
        let post = post_data.map(cstr).transpose()?;
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_LoadURL(
                self.html,
                self.handle(),
                url.as_ptr(),
                post.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
            );
        }
        Ok(())
    }

    /// Resizes the browser viewport in pixels.
    pub fn set_size(&self, width: u32, height: u32) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_SetSize(self.html, self.handle(), width, height);
        }
    }

    /// Aborts the in-flight load.
    pub fn stop_load(&self) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_StopLoad(self.html, self.handle());
        }
    }

    /// Reloads the current page.
    pub fn reload(&self) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_Reload(self.html, self.handle());
        }
    }

    /// Navigates back in history.
    pub fn go_back(&self) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_GoBack(self.html, self.handle());
        }
    }

    /// Navigates forward in history.
    pub fn go_forward(&self) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_GoForward(self.html, self.handle());
        }
    }

    /// Adds a header to subsequent requests from this browser.
    pub fn add_header(&self, key: &str, value: &str) -> Result<(), SteamError> {
        let key = cstr(key)?;
        let value = cstr(value)?;
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_AddHeader(
                self.html,
                self.handle(),
                key.as_ptr(),
                value.as_ptr(),
            );
        }
        Ok(())
    }

    /// Runs JavaScript in the page; results are not returned to the caller.
    pub fn execute_javascript(&self, script: &str) -> Result<(), SteamError> {
        let script = cstr(script)?;
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_ExecuteJavascript(
                self.html,
                self.handle(),
                script.as_ptr(),
            );
        }
        Ok(())
    }

    /// Presses a mouse button at the position established by the last
    /// [`Browser::mouse_move`].
    pub fn mouse_down(&self, button: HtmlMouseButton) {
        let button = button.as_raw();
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_MouseDown(self.html, self.handle(), button);
        }
    }

    /// Releases a mouse button.
    pub fn mouse_up(&self, button: HtmlMouseButton) {
        let button = button.as_raw();
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_MouseUp(self.html, self.handle(), button);
        }
    }

    /// Double-clicks a mouse button.
    pub fn mouse_double_click(&self, button: HtmlMouseButton) {
        let button = button.as_raw();
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_MouseDoubleClick(self.html, self.handle(), button);
        }
    }

    /// Moves the mouse cursor inside the page.
    pub fn mouse_move(&self, x: i32, y: i32) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_MouseMove(self.html, self.handle(), x, y);
        }
    }

    /// Scrolls the mouse wheel by a native delta.
    pub fn mouse_wheel(&self, delta: i32) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_MouseWheel(self.html, self.handle(), delta);
        }
    }

    /// Sends a key press. `system_key` marks OS-level shortcuts, which Steam
    /// handles differently from in-page ones.
    pub fn key_down(&self, native_key_code: u32, modifier: HtmlKeyModifier, system_key: bool) {
        let modifier = modifier.as_raw();
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_KeyDown(
                self.html,
                self.handle(),
                native_key_code,
                modifier,
                system_key,
            );
        }
    }

    /// Sends a key release.
    pub fn key_up(&self, native_key_code: u32, modifier: HtmlKeyModifier) {
        let modifier = modifier.as_raw();
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_KeyUp(
                self.html,
                self.handle(),
                native_key_code,
                modifier,
            );
        }
    }

    /// Sends a typed Unicode character.
    pub fn key_char(&self, unicode_char: u32, modifier: HtmlKeyModifier) {
        let modifier = modifier.as_raw();
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_KeyChar(
                self.html,
                self.handle(),
                unicode_char,
                modifier,
            );
        }
    }

    /// Sets the horizontal scroll offset in absolute pixels.
    pub fn set_horizontal_scroll(&self, pixels: u32) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_SetHorizontalScroll(self.html, self.handle(), pixels);
        }
    }

    /// Sets the vertical scroll offset in absolute pixels.
    pub fn set_vertical_scroll(&self, pixels: u32) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_SetVerticalScroll(self.html, self.handle(), pixels);
        }
    }

    /// Gives or takes away keyboard focus.
    pub fn set_key_focus(&self, focused: bool) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_SetKeyFocus(self.html, self.handle(), focused);
        }
    }

    /// Opens the page source in a browser view.
    pub fn view_source(&self) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_ViewSource(self.html, self.handle());
        }
    }

    /// Copies the current selection to the clipboard.
    pub fn copy_to_clipboard(&self) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_CopyToClipboard(self.html, self.handle());
        }
    }

    /// Pastes from the clipboard into the focused element.
    pub fn paste_from_clipboard(&self) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_PasteFromClipboard(self.html, self.handle());
        }
    }

    /// Searches the page. Repeat calls advance the selection.
    pub fn find(
        &self,
        search: &str,
        currently_in_find: bool,
        reverse: bool,
    ) -> Result<(), SteamError> {
        let search = cstr(search)?;
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_Find(
                self.html,
                self.handle(),
                search.as_ptr(),
                currently_in_find,
                reverse,
            );
        }
        Ok(())
    }

    /// Ends a search.
    pub fn stop_find(&self) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_StopFind(self.html, self.handle());
        }
    }

    /// Asks which link is at a position; the answer arrives as
    /// [`HtmlLinkAtPosition`].
    pub fn get_link_at_position(&self, x: i32, y: i32) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_GetLinkAtPosition(self.html, self.handle(), x, y);
        }
    }

    /// Sets the page zoom, anchored at a point.
    pub fn set_page_scale_factor(&self, zoom: f32, point_x: i32, point_y: i32) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_SetPageScaleFactor(
                self.html,
                self.handle(),
                zoom,
                point_x,
                point_y,
            );
        }
    }

    /// Sets background (non-composited) rendering mode.
    pub fn set_background_mode(&self, enabled: bool) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_SetBackgroundMode(self.html, self.handle(), enabled);
        }
    }

    /// Sets the DPI scaling factor applied to this browser.
    pub fn set_dpi_scaling_factor(&self, scaling: f32) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_SetDPIScalingFactor(self.html, self.handle(), scaling);
        }
    }

    /// Opens the developer tools window.
    pub fn open_developer_tools(&self) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_OpenDeveloperTools(self.html, self.handle());
        }
    }

    /// Answers an [`HtmlStartRequest`]. Steam requires a response: `true` allows
    /// the navigation, `false` cancels it and keeps the current page.
    pub fn allow_start_request(&self, allowed: bool) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_AllowStartRequest(self.html, self.handle(), allowed);
        }
    }

    /// Answers an [`HtmlJsAlert`] or [`HtmlJsConfirm`]. Steam requires a
    /// response; for a confirm, `true` selects OK.
    pub fn js_dialog_response(&self, accepted: bool) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_JSDialogResponse(self.html, self.handle(), accepted);
        }
    }

    /// Answers a [`HtmlFileOpenDialog`] with the chosen paths, or with an empty
    /// slice to cancel it.
    ///
    /// # Errors
    ///
    /// `InvalidParameter` if any path contains an interior nul byte.
    pub fn file_load_dialog_response(&self, files: &[&str]) -> Result<(), SteamError> {
        let cstrings: Vec<CString> = files
            .iter()
            .map(|f| cstr(f))
            .collect::<Result<Vec<_>, _>>()?;
        let mut ptrs: Vec<*const c_char> = cstrings.iter().map(|s| s.as_ptr()).collect();
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_FileLoadDialogResponse(
                self.html,
                self.handle(),
                if ptrs.is_empty() {
                    std::ptr::null_mut()
                } else {
                    ptrs.as_mut_ptr()
                },
            );
        }
        Ok(())
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_RemoveBrowser(self.html, self.handle);
        }
    }
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

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A browser finished loading and is ready for input
pub struct HtmlBrowserReady {
    /// The new browser handle
    pub browser: sys::HHTMLBrowser,
}

impl_callback!(cb: HTML_BrowserReady_t => HtmlBrowserReady {
    Self {
        browser: cb.unBrowserHandle,
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A navigation started; call [`HtmlSurface`] then `allow_start_request` to proceed
pub struct HtmlStartRequest {
    /// The browser navigating
    pub browser: sys::HHTMLBrowser,
    /// Target URL
    pub url: String,
    /// Link target attribute, if any
    pub target: String,
    /// POST body, if any
    pub post_data: String,
    /// Whether this navigation is a redirect
    pub is_redirect: bool,
}

impl_callback!(cb: HTML_StartRequest_t => HtmlStartRequest {
    Self {
        browser: cb.unBrowserHandle,
        url: owned(cb.pchURL),
        target: owned(cb.pchTarget),
        post_data: owned(cb.pchPostData),
        is_redirect: cb.bIsRedirect,
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A navigation completed
pub struct HtmlFinishedRequest {
    /// The browser that finished
    pub browser: sys::HHTMLBrowser,
    /// The loaded URL
    pub url: String,
    /// The page title
    pub page_title: String,
}

impl_callback!(cb: HTML_FinishedRequest_t => HtmlFinishedRequest {
    Self {
        browser: cb.unBrowserHandle,
        url: owned(cb.pchURL),
        page_title: owned(cb.pchPageTitle),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// The address bar value changed
pub struct HtmlUrlChanged {
    /// The browser that changed
    pub browser: sys::HHTMLBrowser,
    /// The new URL
    pub url: String,
    /// POST body, if any
    pub post_data: String,
    /// Whether this was a redirect
    pub is_redirect: bool,
    /// The page title
    pub page_title: String,
    /// Whether this started a new navigation rather than updating in place
    pub new_navigation: bool,
}

impl_callback!(cb: HTML_URLChanged_t => HtmlUrlChanged {
    Self {
        browser: cb.unBrowserHandle,
        url: owned(cb.pchURL),
        post_data: owned(cb.pchPostData),
        is_redirect: cb.bIsRedirect,
        page_title: owned(cb.pchPageTitle),
        new_navigation: cb.bNewNavigation,
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// The page title changed
pub struct HtmlTitleChanged {
    /// The browser reporting
    pub browser: sys::HHTMLBrowser,
    /// The new title
    pub title: String,
}

impl_callback!(cb: HTML_ChangedTitle_t => HtmlTitleChanged {
    Self {
        browser: cb.unBrowserHandle,
        title: owned(cb.pchTitle),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// History availability changed
pub struct HtmlCanGoBackAndForward {
    /// The browser reporting
    pub browser: sys::HHTMLBrowser,
    /// Whether a back navigation is possible
    pub can_go_back: bool,
    /// Whether a forward navigation is possible
    pub can_go_forward: bool,
}

impl_callback!(cb: HTML_CanGoBackAndForward_t => HtmlCanGoBackAndForward {
    Self {
        browser: cb.unBrowserHandle,
        can_go_back: cb.bCanGoBack,
        can_go_forward: cb.bCanGoForward,
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A browser asked to be closed
pub struct HtmlCloseBrowser {
    /// The browser wanting to close
    pub browser: sys::HHTMLBrowser,
}

impl_callback!(cb: HTML_CloseBrowser_t => HtmlCloseBrowser {
    Self {
        browser: cb.unBrowserHandle,
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A page tried to open a new window
pub struct HtmlNewWindow {
    /// The originating browser
    pub browser: sys::HHTMLBrowser,
    /// The requested URL
    pub url: String,
    /// Requested window position and size
    pub rect: (u32, u32, u32, u32),
}

impl_callback!(cb: HTML_NewWindow_t => HtmlNewWindow {
    Self {
        browser: cb.unBrowserHandle,
        url: owned(cb.pchURL),
        rect: (cb.unX, cb.unY, cb.unWide, cb.unTall),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// The answer to [`Browser::get_link_at_position`]
pub struct HtmlLinkAtPosition {
    /// The browser asked
    pub browser: sys::HHTMLBrowser,
    /// The queried position
    pub position: (i32, i32),
    /// The link URL, empty when there is no link
    pub url: String,
    /// Whether the position is over an input element
    pub input: bool,
    /// Whether the link is live (resolvable)
    pub live_link: bool,
}

impl_callback!(cb: HTML_LinkAtPosition_t => HtmlLinkAtPosition {
    Self {
        browser: cb.unBrowserHandle,
        position: (cb.x as i32, cb.y as i32),
        url: owned(cb.pchURL),
        input: cb.bInput,
        live_link: cb.bLiveLink,
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A page raised a JavaScript alert; answer it with [`Browser::js_dialog_response`]
pub struct HtmlJsAlert {
    /// The browser that raised it
    pub browser: sys::HHTMLBrowser,
    /// The alert text
    pub message: String,
}

impl_callback!(cb: HTML_JSAlert_t => HtmlJsAlert {
    Self {
        browser: cb.unBrowserHandle,
        message: owned(cb.pchMessage),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A page raised a JavaScript confirm; answer it with
/// [`Browser::js_dialog_response`] passing `true` for OK
pub struct HtmlJsConfirm {
    /// The browser that raised it
    pub browser: sys::HHTMLBrowser,
    /// The confirm text
    pub message: String,
}

impl_callback!(cb: HTML_JSConfirm_t => HtmlJsConfirm {
    Self {
        browser: cb.unBrowserHandle,
        message: owned(cb.pchMessage),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A page asked for a file; answer it with
/// [`Browser::file_load_dialog_response`]
pub struct HtmlFileOpenDialog {
    /// The browser that asked
    pub browser: sys::HHTMLBrowser,
    /// Dialog title supplied by the page
    pub title: String,
    /// Initial file/path supplied by the page
    pub initial_file: String,
}

impl_callback!(cb: HTML_FileOpenDialog_t => HtmlFileOpenDialog {
    Self {
        browser: cb.unBrowserHandle,
        title: owned(cb.pchTitle),
        initial_file: owned(cb.pchInitialFile),
    }
});

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
