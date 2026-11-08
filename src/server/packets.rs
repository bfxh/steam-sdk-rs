//! The GSHS relay traffic Steam hands the game to carry: a packet arriving from a player, and
//! the next packet owed to one.

use super::*;

impl Server {
    /// Server browser related query packet processing for shared socket mode.  These are used
    /// when you pass STEAMGAMESERVER_QUERY_PORT_SHARED as the query port to SteamGameServer_Init.
    /// IP address and port are in host order, i.e 127.0.0.1 == 0x7f000001
    ///
    /// Source games use this to simplify the job of the server admins, so they
    /// don't have to open up more ports on their firewalls.
    ///
    /// Call this when a packet that starts with 0xFFFFFFFF comes in on the shared socket.
    pub fn handle_incoming_packet(&self, data: &[u8], addr: SocketAddrV4) -> bool {
        unsafe {
            let result = sys::SteamAPI_ISteamGameServer_HandleIncomingPacket(
                self.server,
                data.as_ptr() as _,
                data.len() as _,
                addr.ip().to_bits(),
                addr.port(),
            );
            return result;
        }
    }

    /// Call this function after calling handle_incoming_packet. The callback
    /// function `cb` will be called for each outgoing packet that needs to be sent
    /// to an address. The buffer must be at least 16384 bytes in size.
    pub fn get_next_outgoing_packet(&self, buffer: &mut [u8], cb: impl Fn(SocketAddrV4, &[u8])) {
        assert!(
            buffer.len() >= 16 * 1024,
            "Buffer size must be at least 16 KiB"
        );

        loop {
            let mut addr = 0u32;
            let mut port = 0u16;

            let len = unsafe {
                sys::SteamAPI_ISteamGameServer_GetNextOutgoingPacket(
                    self.server,
                    buffer.as_mut_ptr() as *mut _,
                    buffer.len() as _,
                    &mut addr,
                    &mut port,
                )
            };

            if len == 0 {
                break; // No more packets to process
            }

            let addr = SocketAddrV4::new(Ipv4Addr::from_bits(addr), port);
            cb(addr, &buffer[..len as usize]);
        }
    }
}
