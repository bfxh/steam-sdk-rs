//! The browser handle itself: how it is sent across threads, what it can be asked to do,
//! and the `Drop` that hands the instance back to Steam.

use super::*;

/// A single browser instance.
///
/// Dropping it removes the browser on the Steam side, matching how
/// [`crate::InventoryResult`] releases its handle.
pub struct Browser {
    // `pub(super)`: `mod.rs` is the only place that ever *builds* a Browser -- `create_browser`
    // plants a handle-0 placeholder before Steam answers, and `browser_from_handle` re-wraps a
    // handle that came from outside -- so the split put the constructor one level up.
    pub(super) html: *mut sys::ISteamHTMLSurface,
    pub(super) handle: sys::HHTMLBrowser,
}

// Steam serialises these calls internally, and the crate already asserts this
// for handle types holding a raw interface pointer (see UpdateWatchHandle), so
// a Browser can cross threads the same way.
unsafe impl Send for Browser {}
unsafe impl Sync for Browser {}

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

/// Whether dropping this wrapper should ask Steam to remove anything.
///
/// Zero is this crate's own not-yet-assigned handle: `create_browser` plants a `Browser` with
/// `handle: 0` before Steam answers, and if the answer never arrives the placeholder is dropped when
/// the callback map is. Steam's `RemoveBrowser` takes "HHTMLBrowser unBrowserHandle"
/// (`isteamhtmlsurface.h:47`) and documents no value that means "none", so 0 is not a handle Steam
/// ever handed out, and asking it to remove one is a call about nothing.
fn removes_on_drop(handle: sys::HHTMLBrowser) -> bool {
    handle != 0
}

impl Drop for Browser {
    fn drop(&mut self) {
        if !removes_on_drop(self.handle) {
            return;
        }
        unsafe {
            sys::SteamAPI_ISteamHTMLSurface_RemoveBrowser(self.html, self.handle);
        }
    }
}

#[cfg(test)]
mod browser_tests {
    //! Pure half of `Drop`: whether a handle is one Steam gave us. Reaching the FFI needs a live
    //! browser, so the decision is judged here and the wiring is stated, not tested.
    use super::removes_on_drop;

    #[test]
    fn the_placeholder_handle_asks_steam_for_nothing() {
        assert!(!removes_on_drop(0));
    }

    #[test]
    fn any_real_handle_is_removed_when_the_wrapper_dies() {
        assert!(removes_on_drop(1));
        assert!(removes_on_drop(u32::MAX));
    }
}
