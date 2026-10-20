use super::*;

/// Access to the steam HTTP interface
pub struct Http {
    pub(crate) http: *mut sys::ISteamHTTP,
    pub(crate) _inner: Arc<Inner>,
}

/// HTTP verbs supported by Steam
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Options,
    Patch,
}

impl HttpMethod {
    fn as_raw(self) -> sys::EHTTPMethod {
        match self {
            HttpMethod::Get => sys::EHTTPMethod::k_EHTTPMethodGET,
            HttpMethod::Head => sys::EHTTPMethod::k_EHTTPMethodHEAD,
            HttpMethod::Post => sys::EHTTPMethod::k_EHTTPMethodPOST,
            HttpMethod::Put => sys::EHTTPMethod::k_EHTTPMethodPUT,
            HttpMethod::Delete => sys::EHTTPMethod::k_EHTTPMethodDELETE,
            HttpMethod::Options => sys::EHTTPMethod::k_EHTTPMethodOPTIONS,
            HttpMethod::Patch => sys::EHTTPMethod::k_EHTTPMethodPATCH,
        }
    }
}

/// Rejects strings containing an interior nul byte instead of panicking on
/// `CString::new(...).unwrap()`, matching the crate's no-panic direction.
fn cstr(value: &str) -> Result<CString, SteamError> {
    CString::new(value).map_err(|_| SteamError::InvalidParameter)
}

/// The header value Steam wrote into the buffer, cut where the value actually ends.
///
/// `GetHTTPResponseHeaderSize` says only that it "returns the size of the header value if present"
/// (`isteamhttp.h:75`) and says nothing about a terminator, and `GetHTTPResponseHeaderValue` (`:82`)
/// is the same. So the buffer may hold exactly the value, or the value plus a nul, and nothing on this
/// side observes which. This is the one rule true under both readings: stop at the first nul, keep
/// every byte when there is none. A header value cannot legally contain a nul, so cutting at one loses
/// nothing that was meant to be read.
///
/// The code this replaces dropped the last byte unconditionally, which silently truncates the value in
/// the reading where the size is exact -- and the asymmetry is visible in this same file: `body()`
/// asks for the reported size and returns all of it, with no byte removed.
fn header_value_bytes(buffer: &[u8]) -> &[u8] {
    let end = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    &buffer[..end]
}

/// A cookie container shared by several requests.
///
/// Dropping it releases the underlying Steam handle.
pub struct CookieContainer {
    http: *mut sys::ISteamHTTP,
    handle: sys::HTTPCookieContainerHandle,
    // Declared last so it is released last: this wrapper's own `Drop` calls Steam, and the
    // client's `SteamAPI_Shutdown` runs when the last `Arc<Inner>` goes (`src/lib.rs`).
    _inner: Arc<Inner>,
}

impl CookieContainer {
    /// Binds a cookie to a host + URL path.
    pub fn set_cookie(&self, host: &str, url: &str, cookie: &str) -> Result<(), SteamError> {
        let (host, url, cookie) = (cstr(host)?, cstr(url)?, cstr(cookie)?);
        unsafe {
            if !sys::SteamAPI_ISteamHTTP_SetCookie(
                self.http,
                self.handle,
                host.as_ptr(),
                url.as_ptr(),
                cookie.as_ptr(),
            ) {
                return Err(SteamError::IOFailure);
            }
        }
        Ok(())
    }

    /// Returns the raw Steam handle, e.g. for use through the `raw-bindings` escape hatch.
    pub fn raw(&self) -> sys::HTTPCookieContainerHandle {
        self.handle
    }
}

impl Drop for CookieContainer {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamHTTP_ReleaseCookieContainer(self.http, self.handle);
        }
    }
}

/// A completed (or failed) HTTP request, used to read the response body.
///
/// Steam owns the response buffer, so the request stays alive until this value
/// is dropped; read the body before it goes out of scope.
///
/// # Safety
///
/// The raw interface pointer is `Send`/`Sync` only because Steam serializes
/// these calls internally, matching the existing `UpdateWatchHandle` precedent
/// in this crate.
pub struct HttpResponse {
    http: *mut sys::ISteamHTTP,
    handle: sys::HTTPRequestHandle,
    status: u16,
    // Declared last so it is released last: this wrapper's own `Drop` calls Steam, and the
    // client's `SteamAPI_Shutdown` runs when the last `Arc<Inner>` goes (`src/lib.rs`).
    _inner: Arc<Inner>,
}

// SAFETY: a use site in this crate needs `Send` for this type -- measured by deleting
// this line, which makes `cargo check --all-targets` fail with
// `*mut ISteamHTTP` cannot be sent between threads safely.
// The type holds a raw Steam interface pointer, plain-copy handles, and the owning `Arc<Inner>`;
// it carries no non-atomic reference count, which is the one hazard a `Send` claim here cannot
// see -- `scripts/ci/callback_threads.py` counts `Rc` tokens beside every promise and ships at
// zero for exactly that reason. Moving the value therefore moves sole ownership of the pointer;
// nothing reads the same state through two threads because of the move itself.
unsafe impl Send for HttpResponse {}
unsafe impl Sync for HttpResponse {}

impl HttpResponse {
    /// The HTTP status code Steam received.
    ///
    /// Steam only reports this on the completion callback, so it is captured
    /// then rather than queried later.
    pub fn status_code(&self) -> u16 {
        self.status
    }

    /// Whether the status code is in 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Download progress in [0, 100] for a deferred or streaming request.
    ///
    /// Returns `Err` if Steam has no progress to report for this request.
    pub fn progress_pct(&self) -> Result<f32, SteamError> {
        let mut pct = 0f32;
        unsafe {
            if !sys::SteamAPI_ISteamHTTP_GetHTTPDownloadProgressPct(
                self.http,
                self.handle,
                &mut pct,
            ) {
                return Err(SteamError::IOFailure);
            }
        }
        Ok(pct)
    }

    /// Size of the response body in bytes.
    pub fn body_size(&self) -> Result<u32, SteamError> {
        let mut size = 0u32;
        unsafe {
            if !sys::SteamAPI_ISteamHTTP_GetHTTPResponseBodySize(self.http, self.handle, &mut size)
            {
                return Err(SteamError::IOFailure);
            }
        }
        Ok(size)
    }

    /// The full response body.
    ///
    /// Sizes the buffer from Steam's own reported length, so it cannot
    /// under-allocate relative to the response.
    pub fn body(&self) -> Result<Vec<u8>, SteamError> {
        let size = self.body_size()?;
        let mut buf = vec![0u8; size as usize];
        unsafe {
            if !sys::SteamAPI_ISteamHTTP_GetHTTPResponseBodyData(
                self.http,
                self.handle,
                buf.as_mut_ptr(),
                size,
            ) {
                return Err(SteamError::IOFailure);
            }
        }
        Ok(buf)
    }

    /// A single response header value.
    ///
    /// Steam reports the needed buffer size first, so the value is read in two
    /// passes rather than copied out of a caller-guesses-the-size buffer.
    pub fn header(&self, name: &str) -> Result<Option<String>, SteamError> {
        let name = cstr(name)?;
        unsafe {
            let mut size = 0u32;
            if !sys::SteamAPI_ISteamHTTP_GetHTTPResponseHeaderSize(
                self.http,
                self.handle,
                name.as_ptr(),
                &mut size,
            ) {
                return Err(SteamError::IOFailure);
            }
            if size == 0 {
                return Ok(None);
            }
            let mut buf = vec![0u8; size as usize];
            if !sys::SteamAPI_ISteamHTTP_GetHTTPResponseHeaderValue(
                self.http,
                self.handle,
                name.as_ptr(),
                buf.as_mut_ptr(),
                size,
            ) {
                return Err(SteamError::IOFailure);
            }
            let value = String::from_utf8_lossy(header_value_bytes(&buf)).into_owned();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        }
    }

    /// Whether Steam gave up on the request because it timed out.
    pub fn was_timed_out(&self) -> Result<bool, SteamError> {
        let mut timed_out = false;
        unsafe {
            if !sys::SteamAPI_ISteamHTTP_GetHTTPRequestWasTimedOut(
                self.http,
                self.handle,
                &mut timed_out,
            ) {
                return Err(SteamError::IOFailure);
            }
        }
        Ok(timed_out)
    }

    /// Returns the raw Steam request handle, e.g. for streaming reads.
    pub fn raw(&self) -> sys::HTTPRequestHandle {
        self.handle
    }
}

impl Drop for HttpResponse {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamHTTP_ReleaseHTTPRequest(self.http, self.handle);
        }
    }
}

/// A request under construction.
///
/// Every setter is fallible and returns `Result<Self, SteamError>` so a bad
/// header or URL surfaces as an error rather than a panic. Dropping an
/// un-sent request releases its Steam handle.
pub struct HttpRequest {
    http: *mut sys::ISteamHTTP,
    handle: sys::HTTPRequestHandle,
    _inner: Arc<Inner>,
}

impl HttpRequest {
    fn ok(&self, success: bool) -> Result<(), SteamError> {
        if success {
            Ok(())
        } else {
            Err(SteamError::IOFailure)
        }
    }

    /// Adds a request header.
    pub fn header(self, name: &str, value: &str) -> Result<Self, SteamError> {
        let (name, value) = (cstr(name)?, cstr(value)?);
        let ok = unsafe {
            sys::SteamAPI_ISteamHTTP_SetHTTPRequestHeaderValue(
                self.http,
                self.handle,
                name.as_ptr(),
                value.as_ptr(),
            )
        };
        self.ok(ok)?;
        Ok(self)
    }

    /// Adds a query (GET) or form (POST) parameter.
    pub fn query_param(self, name: &str, value: &str) -> Result<Self, SteamError> {
        let (name, value) = (cstr(name)?, cstr(value)?);
        let ok = unsafe {
            sys::SteamAPI_ISteamHTTP_SetHTTPRequestGetOrPostParameter(
                self.http,
                self.handle,
                name.as_ptr(),
                value.as_ptr(),
            )
        };
        self.ok(ok)?;
        Ok(self)
    }

    /// Sets a raw request body with an explicit content type.
    pub fn raw_body(self, content_type: &str, body: &[u8]) -> Result<Self, SteamError> {
        let content_type = cstr(content_type)?;
        // Steam takes a non-const pointer even though it only reads the body.
        let mut body = body.to_vec();
        let ok = unsafe {
            sys::SteamAPI_ISteamHTTP_SetHTTPRequestRawPostBody(
                self.http,
                self.handle,
                content_type.as_ptr(),
                body.as_mut_ptr(),
                body.len() as u32,
            )
        };
        self.ok(ok)?;
        Ok(self)
    }

    /// Attaches a cookie container to this request.
    pub fn cookies(self, cookies: &CookieContainer) -> Result<Self, SteamError> {
        let handle = cookies.raw();
        let ok = unsafe {
            sys::SteamAPI_ISteamHTTP_SetHTTPRequestCookieContainer(self.http, self.handle, handle)
        };
        self.ok(ok)?;
        Ok(self)
    }

    /// Sets the `User-Agent` value Steam sends.
    pub fn user_agent(self, agent: &str) -> Result<Self, SteamError> {
        let agent = cstr(agent)?;
        let ok = unsafe {
            sys::SteamAPI_ISteamHTTP_SetHTTPRequestUserAgentInfo(
                self.http,
                self.handle,
                agent.as_ptr(),
            )
        };
        self.ok(ok)?;
        Ok(self)
    }

    /// Requires the server certificate to be verified.
    pub fn requires_verified_certificate(self, required: bool) -> Result<Self, SteamError> {
        let ok = unsafe {
            sys::SteamAPI_ISteamHTTP_SetHTTPRequestRequiresVerifiedCertificate(
                self.http,
                self.handle,
                required,
            )
        };
        self.ok(ok)?;
        Ok(self)
    }

    /// Sets the inactivity timeout Steam tolerates between bytes.
    pub fn activity_timeout(self, seconds: u32) -> Result<Self, SteamError> {
        let ok = unsafe {
            sys::SteamAPI_ISteamHTTP_SetHTTPRequestNetworkActivityTimeout(
                self.http,
                self.handle,
                seconds,
            )
        };
        self.ok(ok)?;
        Ok(self)
    }

    /// Sets a hard upper bound on total request duration.
    pub fn absolute_timeout(self, millis: u32) -> Result<Self, SteamError> {
        let ok = unsafe {
            sys::SteamAPI_ISteamHTTP_SetHTTPRequestAbsoluteTimeoutMS(self.http, self.handle, millis)
        };
        self.ok(ok)?;
        Ok(self)
    }

    /// Marks the request for background (deferred) download.
    pub fn defer_download(self) -> Result<Self, SteamError> {
        let ok = unsafe { sys::SteamAPI_ISteamHTTP_DeferHTTPRequest(self.http, self.handle) };
        self.ok(ok)?;
        Ok(self)
    }

    /// Sends the request and delivers [`HttpResponse`] when Steam finishes.
    ///
    /// Download progress is readable through [`HttpResponse::progress_pct`];
    /// the callback fires once, with the status Steam reported.
    pub fn send<F>(self, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<HttpResponse, SteamError>) + 'static + Send,
    {
        self.send_impl(false, callback)
    }

    /// Like [`HttpRequest::send`], but lets Steam deliver the body incrementally
    /// instead of buffering it whole.
    ///
    /// The callback still receives an [`HttpResponse`]; read the body with
    /// [`HttpResponse::body`]. Chunked access through
    /// `GetHTTPStreamingResponseBodyData` is not exposed here, because this
    /// crate's generated binding takes the buffer size by value and reports no
    /// byte count back, so the length of the final chunk is not recoverable.
    pub fn send_and_stream<F>(self, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<HttpResponse, SteamError>) + 'static + Send,
    {
        self.send_impl(true, callback)
    }

    fn send_impl<F>(self, stream: bool, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<HttpResponse, SteamError>) + 'static + Send,
    {
        let http = self.http;
        let handle = self.handle;
        let inner = self._inner.clone();
        // Ownership of the Steam handle transfers to HttpResponse, which
        // releases it; forgetting here avoids a double release.
        std::mem::forget(self);
        let mut response = HttpResponse {
            http,
            handle,
            status: 0,
            _inner: inner.clone(),
        };
        unsafe {
            let mut call: sys::SteamAPICall_t = 0;
            let started = if stream {
                sys::SteamAPI_ISteamHTTP_SendHTTPRequestAndStreamResponse(http, handle, &mut call)
            } else {
                sys::SteamAPI_ISteamHTTP_SendHTTPRequest(http, handle, &mut call)
            };
            if !started {
                std::mem::drop(response);
                return Err(SteamError::IOFailure);
            }
            register_call_result::<sys::HTTPRequestCompleted_t, _>(
                &inner,
                call,
                move |v, io_error| {
                    // The response is the answer, not a placeholder for it: `self` was forgotten
                    // above so the handle ownership moved here, and filling in the status keeps the
                    // closure's capture a whole `HttpResponse` -- which is `Send` by assertion, while
                    // capturing `http`/`handle` separately would not be.
                    let result = if io_error || !v.m_bRequestSuccessful {
                        // Release the request even on failure, since no caller-visible handle survives.
                        Err(SteamError::IOFailure)
                    } else {
                        response.status = v.m_eStatusCode as u16;
                        Ok(response)
                    };
                    callback(result);
                },
            );
        }
        Ok(())
    }
}

impl Drop for HttpRequest {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamHTTP_ReleaseHTTPRequest(self.http, self.handle);
        }
    }
}

impl Http {
    /// Creates a cookie container that survives until it is dropped.
    pub fn create_cookie_container(
        &self,
        supports_credentials: bool,
    ) -> Result<CookieContainer, SteamError> {
        let handle = unsafe {
            sys::SteamAPI_ISteamHTTP_CreateCookieContainer(self.http, supports_credentials)
        };
        if handle == 0 {
            return Err(SteamError::IOFailure);
        }
        Ok(CookieContainer {
            http: self.http,
            handle,
            _inner: self._inner.clone(),
        })
    }

    /// Starts building a request to an absolute URL.
    ///
    /// # Errors
    ///
    /// * `InvalidParameter` - the URL contains an interior nul byte
    /// * `IOFailure` - Steam refused to create the request
    pub fn request(&self, method: HttpMethod, url: &str) -> Result<HttpRequest, SteamError> {
        let url = cstr(url)?;
        let handle = unsafe {
            sys::SteamAPI_ISteamHTTP_CreateHTTPRequest(self.http, method.as_raw(), url.as_ptr())
        };
        if handle == 0 {
            return Err(SteamError::IOFailure);
        }
        Ok(HttpRequest {
            http: self.http,
            handle,
            _inner: self._inner.clone(),
        })
    }

    /// Downloads a URL and delivers its body.
    ///
    /// Convenience over [`Http::request`] plus a single callback, for the common
    /// case of fetching bytes with no extra configuration.
    pub fn get<F>(&self, url: &str, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<Vec<u8>, SteamError>) + 'static + Send,
    {
        self.request(HttpMethod::Get, url)?
            .send(move |result| callback(result.and_then(|response| response.body())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cstr_accepts_plain_text() {
        assert!(cstr("application/json").is_ok());
        assert!(cstr("").is_ok());
    }

    #[test]
    fn cstr_rejects_interior_nul_instead_of_panicking() {
        // Upstream panics here via CString::new(..).unwrap(); this must be an Err.
        assert!(matches!(
            cstr("bad\0header"),
            Err(SteamError::InvalidParameter)
        ));
        assert!(matches!(
            cstr("https://example.com/\0exploit"),
            Err(SteamError::InvalidParameter)
        ));
    }

    #[test]
    fn header_value_keeps_every_byte_when_steam_wrote_no_terminator() {
        // The reading Steam's own words allow: the reported size is the size of the *value*.
        // Dropping the last byte here deleted a real character of every header.
        assert_eq!(
            header_value_bytes(b"application/json"),
            &b"application/json"[..]
        );
        assert_eq!(header_value_bytes(b"a"), b"a");
    }

    #[test]
    fn header_value_stops_at_a_terminator_when_there_is_one() {
        assert_eq!(header_value_bytes(b"gzip\0"), b"gzip");
        assert_eq!(header_value_bytes(&[b'x', 0, b'y', 0]), b"x");
    }

    #[test]
    fn an_all_nul_buffer_reads_as_no_value() {
        // A size Steam reported but never filled must not become four junk characters.
        assert_eq!(header_value_bytes(&[0, 0, 0, 0]), b"");
        assert_eq!(header_value_bytes(&[]), b"");
    }

    #[test]
    fn every_http_method_maps_to_its_own_variant() {
        // A single mis-wired arm would silently turn a POST into a GET, so pin
        // all seven mappings and their distinctness.
        let mapped = [
            (HttpMethod::Get, sys::EHTTPMethod::k_EHTTPMethodGET),
            (HttpMethod::Head, sys::EHTTPMethod::k_EHTTPMethodHEAD),
            (HttpMethod::Post, sys::EHTTPMethod::k_EHTTPMethodPOST),
            (HttpMethod::Put, sys::EHTTPMethod::k_EHTTPMethodPUT),
            (HttpMethod::Delete, sys::EHTTPMethod::k_EHTTPMethodDELETE),
            (HttpMethod::Options, sys::EHTTPMethod::k_EHTTPMethodOPTIONS),
            (HttpMethod::Patch, sys::EHTTPMethod::k_EHTTPMethodPATCH),
        ];
        for (method, expected) in mapped {
            assert_eq!(method.as_raw(), expected, "{method:?} mapped wrongly");
        }

        let distinct: Vec<_> = mapped.iter().map(|(_, raw)| *raw).collect();
        for i in 0..distinct.len() {
            for j in (i + 1)..distinct.len() {
                assert_ne!(distinct[i], distinct[j], "two verbs collapse to one method");
            }
        }
    }

    #[test]
    fn no_http_method_maps_to_invalid() {
        // k_EHTTPMethodInvalid would make Steam reject every request; none of the
        // seven verbs may resolve to it.
        let invalid = sys::EHTTPMethod::k_EHTTPMethodInvalid;
        for method in [
            HttpMethod::Get,
            HttpMethod::Head,
            HttpMethod::Post,
            HttpMethod::Put,
            HttpMethod::Delete,
            HttpMethod::Options,
            HttpMethod::Patch,
        ] {
            assert_ne!(method.as_raw(), invalid, "{method:?} resolved to Invalid");
        }
    }
}
