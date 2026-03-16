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

/// Returns a flat Vec of **every** card in the entire Lovelace config
/// (including all nested ones). Owned structs, no cloning of the original JSON.
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