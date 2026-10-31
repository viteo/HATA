use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::layout::FULL;

// https://github.com/home-assistant/frontend/blob/master/src/data/lovelace/config/types.ts
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LovelaceCardConfig {
    #[serde(rename = "type")]
    pub r#type: String,
    pub title: Option<String>,
    pub name: Option<String>,
    pub entity: Option<String>,
    pub entities: Option<Vec<Value>>,
    pub grid_options: Option<LovelaceGridOptions>,
    // Everything else (icon, theme, style, custom fields, etc.)
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// The size of a card in the grid of a section.
/// https://github.com/home-assistant/frontend/blob/dev/src/panels/lovelace/types.ts
#[derive(Debug, Deserialize)]
pub struct LovelaceGridOptions {
    /// A number of cells or "full".
    pub columns: Option<Value>,
    /// A number of rows or "auto".
    pub rows: Option<Value>,
}

/// https://github.com/home-assistant/frontend/blob/dev/src/data/lovelace/config/section.ts
#[derive(Debug, Deserialize)]
pub struct LovelaceSectionConfig {
    pub column_span: Option<u16>,
    #[serde(default)]
    pub cards: Vec<Value>,
}

/// https://github.com/home-assistant/frontend/blob/dev/src/data/lovelace/config/view.ts
#[derive(Debug, Deserialize)]
pub struct LovelaceViewConfig {
    pub sections: Option<Vec<LovelaceSectionConfig>>,
    pub cards: Option<Vec<Value>>,
}

/// https://github.com/home-assistant/frontend/blob/dev/src/data/lovelace/config/types.ts
#[derive(Debug, Deserialize)]
pub struct LovelaceConfig {
    #[serde(default)]
    pub views: Vec<LovelaceViewConfig>,
}

/// The configuration of an entity inside a card.
/// https://github.com/home-assistant/frontend/blob/dev/src/panels/lovelace/entity-rows/types.ts
#[derive(Debug, Clone)]
pub struct EntityConfig {
    pub entity: String,
    /// The name shown on the card, overrides the friendly name of the entity.
    pub name: Option<String>,
    /// The service to call when the card is activated, and its data.
    pub tap_action: Option<(String, Value)>,
}

impl LovelaceCardConfig {
    pub fn entity_configs(&self) -> Vec<EntityConfig> {
        let mut configs = Vec::new();
        if let Some(entity) = &self.entity {
            configs.push(EntityConfig {
                entity: entity.clone(),
                name: None,
                tap_action: None,
            });
        }
        if let Some(entities) = &self.entities {
            for entity in entities {
                match entity {
                    Value::String(entity) => configs.push(EntityConfig {
                        entity: entity.clone(),
                        name: None,
                        tap_action: None,
                    }),
                    Value::Object(obj) => {
                        let Some(Value::String(entity)) = obj.get("entity") else {
                            continue;
                        };
                        configs.push(EntityConfig {
                            entity: entity.clone(),
                            name: obj.get("name").and_then(|v| v.as_str()).map(str::to_string),
                            tap_action: obj.get("tap_action").and_then(parse_action),
                        });
                    }
                    _ => {}
                }
            }
        }
        configs
    }

    /// The service to call when the card itself is activated, and its data.
    pub fn tap_action(&self) -> Option<(String, Value)> {
        self.extra.get("tap_action").and_then(parse_action)
    }

    /// The size set in the grid options, in cells and rows; "full" is `FULL`, "auto" is none.
    pub fn grid_size(&self) -> (Option<u16>, Option<u16>) {
        let Some(options) = &self.grid_options else {
            return (None, None);
        };
        let columns = match &options.columns {
            Some(Value::String(full)) if full == "full" => Some(FULL),
            Some(columns) => columns.as_u64().map(|c| c as u16),
            None => None,
        };
        let rows = options.rows.as_ref().and_then(Value::as_u64).map(|r| r as u16);
        (columns, rows)
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

/// The sections of all views; each card of a view without sections makes a section of its own.
pub fn extract_sections(root: &Value) -> Vec<LovelaceSectionConfig> {
    let Ok(config) = serde_json::from_value::<LovelaceConfig>(root.clone()) else {
        return Vec::new();
    };
    let mut sections = Vec::new();
    for view in config.views {
        if let Some(view_sections) = view.sections {
            sections.extend(view_sections);
        } else {
            for card in view.cards.unwrap_or_default() {
                sections.push(LovelaceSectionConfig { column_span: None, cards: vec![card] });
            }
        }
    }
    sections
}

impl LovelaceSectionConfig {
    /// The cards of the section, with stacks replaced by the cards in them.
    pub fn flat_cards(&self) -> Vec<LovelaceCardConfig> {
        let mut cards = Vec::new();
        for card in &self.cards {
            collect_cards(card, &mut cards);
        }
        cards
    }
}

fn collect_cards(value: &Value, found_cards: &mut Vec<LovelaceCardConfig>) {
    // a stack shows its "cards", a conditional card shows its "card"
    if let Some(Value::Array(cards)) = value.get("cards") {
        for card in cards {
            collect_cards(card, found_cards);
        }
    } else if let Some(card) = value.get("card") {
        collect_cards(card, found_cards);
    } else if let Ok(card) = serde_json::from_value::<LovelaceCardConfig>(value.clone()) {
        found_cards.push(card);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nested_stacks_give_each_card_once() {
        let config = json!({"views": [{"sections": [{"cards": [
            {"type": "vertical-stack", "cards": [
                {"type": "vertical-stack", "cards": [{"type": "tile", "entity": "light.a"}]}
            ]}
        ]}]}]});
        let sections = extract_sections(&config);
        let cards = sections[0].flat_cards();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].entity.as_deref(), Some("light.a"));
    }

    #[test]
    fn cards_of_a_view_without_sections_make_sections() {
        let config = json!({"views": [{"cards": [
            {"type": "tile", "entity": "light.a"},
            {"type": "tile", "entity": "light.b"}
        ]}]});
        assert_eq!(extract_sections(&config).len(), 2);
    }

    #[test]
    fn grid_size_reads_full_and_auto() {
        let card: LovelaceCardConfig = serde_json::from_value(json!(
            {"type": "heading", "grid_options": {"columns": "full", "rows": "auto"}}
        ))
        .unwrap();
        assert_eq!(card.grid_size(), (Some(FULL), None));
    }
}
