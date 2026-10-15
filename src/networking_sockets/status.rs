//! Reading a live connection: the info struct, the realtime status, and the lane
//! configuration that changes it.

use super::*;

impl NetworkingSockets {
    /// Returns basic information about the high-level state of the connection.
    ///
    /// Returns false if the connection handle is invalid.
    pub fn get_connection_info(
        &self,
        connection: &NetConnection,
    ) -> Result<NetConnectionInfo, bool> {
        let mut info: sys::SteamNetConnectionInfo_t = unsafe { std::mem::zeroed() };
        let was_successful = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_GetConnectionInfo(
                self.sockets,
                connection.handle,
                &mut info,
            )
        };
        if was_successful {
            Ok(NetConnectionInfo { inner: info })
        } else {
            Err(false)
        }
    }

    /// Returns a small set of information about the real-time state of the connection and the queue status of each lane.
    ///
    /// On entry, lanes specifies the length of the lanes array. This may be 0 if you do not wish to receive any lane data. It's OK for this to be smaller than the total number of configured lanes.
    ///
    /// pLanes points to an array that will receive lane-specific info. It can be NULL if this is not needed.
    pub fn get_realtime_connection_status(
        &self,
        connection: &NetConnection,
        lanes: i32,
    ) -> Result<
        (
            NetConnectionRealTimeInfo,
            Vec<NetConnectionRealTimeLaneStatus>,
        ),
        SteamError,
    > {
        let mut info: sys::SteamNetConnectionRealTimeStatus_t = unsafe { std::mem::zeroed() };
        let mut p_lanes: Vec<sys::SteamNetConnectionRealTimeLaneStatus_t> =
            Vec::with_capacity(lanes as usize);
        let result = unsafe {
            // Get a reference to the uninitialized part of our Vec's buffer
            let uninitialized = p_lanes.spare_capacity_mut();
            let status = sys::SteamAPI_ISteamNetworkingSockets_GetConnectionRealTimeStatus(
                self.sockets,
                connection.handle,
                &mut info,
                lanes,
                uninitialized.as_mut_ptr().cast(),
            );
            // Tell the Vec that we've manually initialized some elements
            p_lanes.set_len(lanes as usize);
            status
        };
        crate::to_steam_result(result).map(|_| {
            (
                NetConnectionRealTimeInfo { inner: info },
                p_lanes
                    .into_iter()
                    .map(|x| NetConnectionRealTimeLaneStatus { inner: x })
                    .collect(),
            )
        })
    }

    /// Configure multiple outbound messages streams ("lanes") on a connection, and control head-of-line blocking between them. Messages within a given lane are always sent in the order they are queued, but messages from different lanes may be sent out of order. Each lane has its own message number sequence. The first message sent on each lane will be assigned the number 1.
    ///
    /// Each lane has a "priority". Lower priority lanes will only be processed when all higher-priority lanes are empty. The magnitudes of the priority values are not relevant, only their sort order. Higher numeric values take priority over lower numeric values.
    ///
    /// Each lane also is assigned a weight, which controls the approximate proportion of the bandwidth that will be consumed by the lane, relative to other lanes of the same priority. (This is assuming the lane stays busy. An idle lane does not build up "credits" to be be spent once a message is queued.) This value is only meaningful as a proportion, relative to other lanes with the same priority. For lanes with different priorities, the strict priority order will prevail, and their weights relative to each other are not relevant. Thus, if a lane has a unique priority value, the weight value for that lane is not relevant.
    ///
    /// Example: 3 lanes, with priorities { 0, 10, 10 } and weights { (NA), 20, 5 }. Messages sent on the first will always be sent first, before messages in the other two lanes. Its weight value is irrelevant, since there are no other lanes with priority=0. The other two lanes will share bandwidth, with the second and third lanes sharing bandwidth using a ratio of approximately 4:1. (The weights { NA, 4, 1 } would be equivalent.)
    pub fn configure_connection_lanes(
        &self,
        connection: &NetConnection,
        num_lanes: i32,
        lane_priorities: &[i32],
        lane_weights: &[u16],
    ) -> Result<(), SteamError> {
        let result = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_ConfigureConnectionLanes(
                self.sockets,
                connection.handle,
                num_lanes,
                lane_priorities.as_ptr(),
                lane_weights.as_ptr(),
            )
        };
        crate::to_steam_result(result)
    }
}
