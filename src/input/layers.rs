//! Action sets and layers: activating a set, activating/deactivating/listing layers, and the
//! current set -- including the clamp that never reads past Steam's own layer array.

use super::*;

impl Input {
    /// Add an action set layer to a controller, on top of its active action set.
    pub fn activate_action_set_layer(
        &self,
        controller: sys::InputHandle_t,
        layer: sys::InputActionSetHandle_t,
    ) {
        unsafe { sys::SteamAPI_ISteamInput_ActivateActionSetLayer(self.input, controller, layer) }
    }

    /// Remove one action set layer.
    pub fn deactivate_action_set_layer(
        &self,
        controller: sys::InputHandle_t,
        layer: sys::InputActionSetHandle_t,
    ) {
        unsafe { sys::SteamAPI_ISteamInput_DeactivateActionSetLayer(self.input, controller, layer) }
    }

    /// Remove every action set layer at once.
    pub fn deactivate_all_action_set_layers(&self, controller: sys::InputHandle_t) {
        unsafe { sys::SteamAPI_ISteamInput_DeactivateAllActionSetLayers(self.input, controller) }
    }

    /// Enumerate the currently active layers.
    ///
    /// Steam documents the out array as `STEAM_INPUT_MAX_ACTIVE_LAYERS` sized and returns "the
    /// number of handles written"; this wrapper allocates exactly that and clamps Steam's count to
    /// it, so an over-reported number cannot read past the array.
    pub fn active_action_set_layers(
        &self,
        controller: sys::InputHandle_t,
    ) -> Vec<sys::InputActionSetHandle_t> {
        let capacity = sys::STEAM_INPUT_MAX_ACTIVE_LAYERS as usize;
        let mut handles = vec![0u64; capacity];
        let written = unsafe {
            sys::SteamAPI_ISteamInput_GetActiveActionSetLayers(
                self.input,
                controller,
                handles.as_mut_ptr(),
            )
        };
        handles.truncate(layers_to_take(written, capacity));
        handles
    }

    /// The action set currently active on a controller, as Steam reports it.
    pub fn current_action_set(
        &self,
        controller: sys::InputHandle_t,
    ) -> sys::InputActionSetHandle_t {
        unsafe { sys::SteamAPI_ISteamInput_GetCurrentActionSet(self.input, controller) }
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;

    #[test]
    fn the_layer_list_never_reads_past_the_array_steam_fills() {
        let capacity = sys::STEAM_INPUT_MAX_ACTIVE_LAYERS as usize;
        assert_eq!(
            capacity, 16,
            "the bound this wrapper allocates is Steam's own constant"
        );
        assert_eq!(layers_to_take(3, capacity), 3);
        assert_eq!(layers_to_take(16, capacity), 16);
        assert_eq!(
            layers_to_take(40, capacity),
            16,
            "Steam's written count is clamped to the array it was given"
        );
        assert_eq!(
            layers_to_take(-1, capacity),
            0,
            "a negative count must not wrap"
        );
    }

    #[test]
    fn an_absent_binding_revision_is_none_not_a_zero_pair() {
        // A false answer drops the pair, because Steam may not have written either out
        // parameter at all. Even the all-zero pair is refused, so a caller cannot mistake an
        // unanswered query for "revision 0.0".
        assert_eq!(binding_revision(false, 0, 0), None);
        assert_eq!(binding_revision(false, 5, 2), None);
        assert_eq!(binding_revision(true, 5, 2), Some((5, 2)));
        assert_eq!(binding_revision(true, 0, 0), Some((0, 0)));
    }
}
