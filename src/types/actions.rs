// https://github.com/home-assistant/frontend/blob/dev/src/common/entity/get_toggle_action.ts
//! The actions that can be performed on an entity.

/// The states that mean the entity is off.
const STATES_OFF: &[&str] = &["closed", "locked", "off"];

/// Domains with a nonstandard toggle behaviour:
/// (domain, service to turn on, service to turn off).
const SPECIAL_TOGGLE_ACTIONS: &[(&str, &str, Option<&str>)] = &[
    ("button", "press", None),
    ("camera", "turn_on", Some("turn_off")),
    ("climate", "turn_on", Some("turn_off")),
    ("cover", "open_cover", Some("close_cover")),
    ("input_button", "press", None),
    ("lock", "unlock", Some("lock")),
    ("media_player", "turn_on", Some("turn_off")),
    ("scene", "turn_on", None),
    ("siren", "turn_on", None),
    ("valve", "open_valve", Some("close_valve")),
];

/// Whether the state means the entity is off.
pub fn is_off_state(state: &str) -> bool {
    STATES_OFF.contains(&state)
}

/// The service that switches the entity on or off.
pub fn toggle_action(domain: &str, on: bool) -> &str {
    let special = SPECIAL_TOGGLE_ACTIONS
        .iter()
        .copied()
        .find(|(special_domain, _, _)| *special_domain == domain);
    match (special, on) {
        (Some((_, _, Some(off_service))), false) => off_service,
        (Some((_, on_service, _)), _) => on_service,
        (None, true) => "turn_on",
        (None, false) => "turn_off",
    }
}
