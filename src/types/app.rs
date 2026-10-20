use indexmap::IndexMap;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererKind {
    Clock,
    Binary,
    NumericTrend,
    Default,
}

#[derive(Debug)]
pub struct Card {
    pub entity_id: String,
    pub friendly_name: String,
    pub action: Option<(String, Value)>,
    pub state: String,
    pub domain: String,
    pub services: Vec<String>,
    pub renderer: RendererKind,
    pub has_trend: bool,
    pub history: Option<Vec<f32>>,
}

impl Card {
    pub fn new(entity_id: String, domain: String, card_type: String) -> Self {
        let renderer = Self::resolve_renderer(&domain, &card_type);
        let has_trend = renderer == RendererKind::NumericTrend;
        Self {
            entity_id,
            friendly_name: String::new(),
            action: None,
            state: String::new(),
            domain,
            services: vec![],
            renderer,
            has_trend,
            history: if has_trend { Some(Vec::new()) } else { None },
        }
    }

    fn resolve_renderer(domain: &str, card_type: &str) -> RendererKind {
        if card_type.contains("clock") {
            return RendererKind::Clock;
        }
        if card_type.contains("sensor")
            || card_type.contains("gauge")
            || card_type.contains("graph")
        {
            return RendererKind::NumericTrend;
        }
        if matches!(
            domain,
            "light" | "switch" | "binary_sensor" | "input_boolean" | "fan" | "automation"
        ) {
            return RendererKind::Binary;
        }
        RendererKind::Default
    }

    pub fn preferred_action(&self) -> Option<String> {
        // an action configured on the card wins over guesswork
        if let Some((service, _)) = &self.action {
            return Some(service.clone());
        }

        const GENERIC_PRIORITY: &[&str] = &[
            "toggle",
            "press",
            "trigger",
            "execute",
            "increment",
            "media_play_pause",
            "next_track",
            "open",
            "start",
            "turn_on",
            "close",
            "stop",
            "turn_off",
        ];

        for service in GENERIC_PRIORITY {
            if self.supports_service(service) {
                return Some(service.to_string());
            }
        }

        self.services.first().cloned()
    }

    fn supports_service(&self, service_name: &str) -> bool {
        self.services.iter().any(|svc| {
            svc.contains(service_name)
        })
    }

    pub fn action_data(&self) -> Option<Value> {
        self.action.as_ref().map(|(_, data)| data.clone())
    }

    pub fn maybe_push_state_sample(&mut self) {
        if !self.has_trend {
            return;
        }
        let Ok(v) = self.state.parse::<f32>() else {
            return;
        };

        let Some(samples) = self.history.as_mut() else {
            return;
        };
        samples.push(v);
        const MAX_SAMPLES: usize = 32;
        if samples.len() > MAX_SAMPLES {
            let drain = samples.len() - MAX_SAMPLES;
            samples.drain(0..drain);
        }
    }
}

#[derive(Debug)]
pub enum AppEvent {
    // from backend
    Snapshot {
        entities: Vec<(String, Card)>,
    },
    StateChanged { entity_id: String, state: String },
    EventAdded { entity_id: String, friendly_name: String, state: String },
    Status(String),
    Error(String),

    // from ui
    CallService { entity_id: String, service: String, data: Option<Value> },
}

pub struct AppState {
    pub title: &'static str,
    pub entities: IndexMap<String, Card>, // entity_id -> card
    pub selected: usize,
    pub status: String,
    pub last_error: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            title: "Home Assistant",
            entities: IndexMap::new(),
            selected: 0,
            status: "<connecting>".to_string(),
            last_error: None,
        }
    }

    /// All cards showing the given entity.
    pub fn get_entity_cards(&mut self, entity_id: &str) -> Vec<&mut Card> {
        self.entities
            .values_mut()
            .filter(|card| card.entity_id == entity_id)
            .collect()
    }
}
