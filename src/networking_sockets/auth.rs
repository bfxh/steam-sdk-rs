//! The authentication handshake surface -- kicking off `InitAuthentication` and reading the
//! status Steam reports for a connection.

use super::*;

impl NetworkingSockets {
    /// Indicate our desire to be ready participate in authenticated communications.
    /// If we are currently not ready, then steps will be taken to obtain the necessary
    /// certificates.   (This includes a certificate for us, as well as any CA certificates
    /// needed to authenticate peers.)
    ///
    /// You can call this at program init time if you know that you are going to
    /// be making authenticated connections, so that we will be ready immediately when
    /// those connections are attempted.  (Note that essentially all connections require
    /// authentication, with the exception of ordinary UDP connections with authentication
    /// disabled using k_ESteamNetworkingConfig_IP_AllowWithoutAuth.)  If you don't call
    /// this function, we will wait until a feature is utilized that that necessitates
    /// these resources.
    ///
    /// You can also call this function to force a retry, if failure has occurred.
    /// Once we make an attempt and fail, we will not automatically retry.
    /// In this respect, the behavior of the system after trying and failing is the same
    /// as before the first attempt: attempting authenticated communication or calling
    /// this function will call the system to attempt to acquire the necessary resources.
    ///
    /// You can use GetAuthenticationStatus or listen for SteamNetAuthenticationStatus_t
    /// to monitor the status.
    ///
    /// Returns the current value that would be returned from GetAuthenticationStatus.
    pub fn init_authentication(
        &self,
    ) -> Result<NetworkingAvailability, NetworkingAvailabilityError> {
        unsafe { sys::SteamAPI_ISteamNetworkingSockets_InitAuthentication(self.sockets).try_into() }
    }

    pub fn get_authentication_status(
        &self,
    ) -> Result<NetworkingAvailability, NetworkingAvailabilityError> {
        let mut details: sys::SteamNetAuthenticationStatus_t = unsafe { std::mem::zeroed() };
        let auth = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_GetAuthenticationStatus(
                self.sockets,
                &mut details,
            )
        };

        auth.try_into()
    }
}
