use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

// https://github.com/home-assistant/frontend/blob/master/src/data/lovelace/config/types.ts
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LovelaceCard {
    #[serde(rename = "type")]
    pub r#type: String,
    pub title: Option<String>,
    pub name: Option<String>,
    pub entity: Option<String>,
    pub entities: Option<Vec<Value>>,
    // Everything else (icon, theme, style, custom fields, etc.)
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// The configuration of an entity inside a card.
/// https://github.com/home-assistant/frontend/blob/dev/src/panels/lovelace/entity-rows/types.ts
#[derive(Debug, Clone)]
pub struct EntityConfig {
    pub entity_id: String,
    /// The name shown on the card, overrides the friendly name of the entity.
    pub name: Option<String>,
    /// The service to call when the card is activated, and its data.
    pub action: Option<(String, Value)>,
}

impl LovelaceCard {
    pub fn entity_configs(&self) -> Vec<EntityConfig> {
        let mut configs = Vec::new();
        if let Some(entity_id) = &self.entity {
            configs.push(EntityConfig {
                entity_id: entity_id.clone(),
                name: None,
                action: None,
            });
        }
        if let Some(entities) = &self.entities {
            for entity in entities {
                match entity {
                    Value::String(entity_id) => configs.push(EntityConfig {
                        entity_id: entity_id.clone(),
                        name: None,
                        action: None,
                    }),
                    Value::Object(obj) => {
                        let Some(Value::String(entity_id)) = obj.get("entity") else {
                            continue;
                        };
                        configs.push(EntityConfig {
                            entity_id: entity_id.clone(),
                            name: obj.get("name").and_then(|v| v.as_str()).map(str::to_string),
                            action: obj.get("tap_action").and_then(parse_action),
                        });
                    }
                    _ => {}
                }
            }
        }
        configs
    }
}

/// The service to call and its data, if the action is a service call.
/// https://github.com/home-assistant/frontend/blob/dev/src/data/lovelace/config/action.ts
fn parse_action(value: &Value) -> Option<(String, Value)> {
    let action = value.get("action")?.as_str()?;
    if action != "call-service" && action != "perform-action" {
        return None;
    }
    // "service" is the deprecated name of "perform_action"
    let service = value
        .get("perform_action")
        .or_else(|| value.get("service"))?
        .as_str()?
        .to_string();
    // "service_data" is the deprecated name of "data"
    let mut data = value
        .get("data")
        .or_else(|| value.get("service_data"))
        .cloned()
        .unwrap_or(Value::Object(Default::default()));
    if let Some(obj) = data.as_object_mut() {
        if !obj.contains_key("entity_id") {
            if let Some(target) = value
                .get("target")
                .and_then(|t| t.get("entity_id"))
                .and_then(|v| v.as_str())
            {
                obj.insert(
                    "entity_id".to_string(),
                    Value::String(target.to_string()),
                );
            }
        }
    }
    Some((service, data))
}

/// Returns a flat Vec of **every** card in the entire Lovelace config
pub fn extract_all_cards(root: &Value) -> Vec<LovelaceCard> {
    let mut cards = Vec::new();
    collect_cards(root, &mut cards);
    cards
}

/// Recursively look for card objects in json
fn collect_cards(value: &Value, found_cards: &mut Vec<LovelaceCard>) {
    match value {
        Value::Object(obj) => {
            // "cards" array
            if let Some(Value::Array(cards_arr)) = obj.get("cards") {
                for card_value in cards_arr {
                    if let Ok(card) = serde_json::from_value::<LovelaceCard>(card_value.clone()) {
                        found_cards.push(card);
                    }
                    collect_cards(card_value, found_cards); // recurse deeper
                }
            }

            // singular card
            if let Some(card_val) = obj.get("card") {
                if let Ok(card) = serde_json::from_value::<LovelaceCard>(card_val.clone()) {
                    found_cards.push(card);
                }
                collect_cards(card_val, found_cards);
            }

            // recurse into everything else
            for v in obj.values() {
                collect_cards(v, found_cards);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_cards(item, found_cards);
            }
        }
        _ => {}
    }
}
