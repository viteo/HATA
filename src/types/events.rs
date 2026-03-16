#![allow(dead_code)]
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

type EntityId = String;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Event {
    NormalEvent(HAEvent),
    CompressedEvent(SubscribeEntitiesEvent),
}

// https://github.com/home-assistant/core/blob/dev/homeassistant/core.py
// class Event(Generic[_DataT]):
// event: Event[EventStateChangedData]
/// ""Representation of an event within the bus."""
#[derive(Debug, Deserialize)]
pub struct HAEvent {
    pub context: Value,
    pub data: StateChangedData,
    pub event_type: String,
    pub origin: String,
    pub time_fired: String,
}

// https://github.com/home-assistant/core/blob/dev/homeassistant/core.py
// state_changed_data: EventStateChangedData
#[derive(Debug, Deserialize)]
pub struct StateChangedData {
    pub entity_id: Option<EntityId>,
    pub new_state: Option<State>,
    pub old_state: Option<State>,
}

// https://github.com/home-assistant/core/blob/dev/homeassistant/core.py
/// class State:
/// """Object to represent a state within the state machine."""
#[derive(Debug, Deserialize)]
pub struct State {
    pub entity_id: EntityId,
    pub state: String,
    pub attributes: Value,
    pub last_changed: String,
    pub last_updated: String,
    pub context: Option<Value>,
}

// def _state_diff_event from https://github.com/home-assistant/core/blob/master/homeassistant/components/websocket_api/messages.py
// compressed entities from https://github.com/home-assistant/android/blob/main/common/src/main/kotlin/io/homeassistant/companion/android/common/data/websocket/impl/entities/CompressedEntity.kt
/**
 * Represents a single event emitted in a `subscribe_entities` websocket subscription. One event can
 * contain state changes for multiple entities; properties map them as entity_id -> state.
 */
#[derive(Debug, Deserialize)]
pub struct SubscribeEntitiesEvent {
    /// current states of subscribed entities
    #[serde(rename = "a")]
    pub added: Option<HashMap<EntityId, CompressedEntityState>>,

    /// changes in subscribed entities
    #[serde(rename = "c")]
    pub changed: Option<HashMap<EntityId, CompressedStateDiff>>,

    /// unsubscribed from entities
    #[serde(rename = "r")]
    pub removed: Option<Vec<EntityId>>,
}

/**
 * A compressed version of [Entity] used for additions or changes in the entity's state in a
 * `subscribe_entities` websocket subscription.
 */
#[derive(Debug, Deserialize)]
pub struct CompressedEntityState {
    /// current state/value of entity
    #[serde(rename = "s")]
    pub state: String,

    /// "friendly_name", "mode", "min", "max" etc
    #[serde(rename = "a")]
    pub attributes: Option<HashMap<String, Value>>,

    /// some ID or object with "id","parent_id","user_id"
    #[serde(rename = "c")]
    pub context: Option<Value>,

    /// last changed/updated in Unix seconds
    #[serde(rename = "lc")]
    pub last_changed: Option<f64>,

    #[serde(rename = "lu")]
    pub last_updated: Option<f64>,
}

/**
 * Describes the difference in an [Entity] state in a `subscribe_entities` websocket subscription.
 * It will only include properties that have been changed.
 */
#[derive(Debug, Deserialize)]
pub struct CompressedStateDiff {
    #[serde(rename = "+")]
    pub additions: Option<CompressedEntityState>,

    #[serde(rename = "-")]
    pub removals: Option<CompressedEntityRemoved>,
}

/**
 * A compressed version of [Entity] used for removed properties from the entity's state in a
 * `subscribe_entities` websocket subscription. Only attributes are expected to be removed.
 */
#[derive(Debug, Deserialize)]
pub struct CompressedEntityRemoved {
    #[serde(rename = "a")]
    pub attributes: Vec<String>,
}
