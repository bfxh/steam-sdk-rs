//! The twelve notifications `ISteamHTMLSurface` posts about a browser: readiness, navigation
//! requests and their results, title and URL changes, dialogs, window requests and icon
//! changes. Each struct travels with the `impl_callback!` that registers it.

use super::*;

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
