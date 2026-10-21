use serde_json::Value;

use super::actions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererKind {
    Clock,
    Binary,
    NumericTrend,
    Markdown,
    Default,
}

/// entirely frontend card has no state and need no entity
pub fn local_renderer(card_type: &str) -> Option<RendererKind> {
    match card_type {
        "clock" => Some(RendererKind::Clock),
        "markdown" => Some(RendererKind::Markdown),
        _ => None,
    }
}

/// A single tile of the dashboard: the configuration of a lovelace card
/// together with the state of the entity it shows.
#[derive(Debug)]
pub struct Card {
    pub entity_id: String,
    /// The name shown on the card.
    pub name: String,
    /// The service to call when the card is activated, and its data.
    pub tap_action: Option<(String, Value)>,
    pub state: String,
    pub domain: String,
    pub services: Vec<String>,
    pub renderer: RendererKind,
    pub samples: Option<Vec<f32>>,
}

impl Card {
    pub fn new(entity_id: String, domain: String, card_type: String) -> Self {
        let renderer = Self::resolve_renderer(&domain, &card_type);
        Self {
            entity_id,
            name: String::new(),
            tap_action: None,
            state: String::new(),
            domain,
            services: vec![],
            renderer,
            samples: if renderer == RendererKind::NumericTrend {
                Some(Vec::new())
            } else {
                None
            },
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

    /// The service to call to toggle the entity.
    pub fn toggle_service(&self) -> Option<String> {
        // an action configured on the card wins over the guesswork
        if let Some((service, _)) = &self.tap_action {
            return Some(service.clone());
        }
        // a toggle service switches the entity atomically
        if self.services.iter().any(|service| service == "toggle") {
            return Some("toggle".to_string());
        }
        // the per domain action of the web app, if the domain has it
        let service = actions::toggle_action(&self.domain, !actions::is_off_state(&self.state));
        if self.services.iter().any(|available| available == service) {
            return Some(service.to_string());
        }
        None
    }

    pub fn tap_action_data(&self) -> Option<Value> {
        self.tap_action.as_ref().map(|(_, data)| data.clone())
    }

    pub fn push_sample(&mut self) {
        let Some(samples) = self.samples.as_mut() else {
            return;
        };
        let Ok(v) = self.state.parse::<f32>() else {
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

/// The status of the connection to Home Assistant.
/// Mirrors `ConnectionStatus` of the web app.
#[derive(Debug)]
pub enum ConnectionStatus {
    Connecting,
    /// Connected to Home Assistant, with its version.
    Connected(String),
    AuthInvalid,
    Disconnected,
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connecting => write!(f, "connecting"),
            Self::Connected(version) => write!(f, "connected ({version})"),
            Self::AuthInvalid => write!(f, "auth invalid"),
            Self::Disconnected => write!(f, "disconnected"),
        }
    }
}

#[derive(Debug)]
pub enum AppEvent {
    // from backend
    Snapshot {
        cards: Vec<Card>,
    },
    StateChanged { entity_id: String, state: String },
    EntityAdded { entity_id: String, name: String, state: String },
    Status(ConnectionStatus),
    Error(String),

    // from ui
    CallService { entity_id: String, service: String, data: Option<Value> },
}

pub struct AppState {
    pub title: &'static str,
    pub cards: Vec<Card>,
    pub selected: usize,
    pub status: ConnectionStatus,
    pub last_error: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            title: "Home Assistant",
            cards: Vec::new(),
            selected: 0,
            status: ConnectionStatus::Connecting,
            last_error: None,
        }
    }

    /// All cards showing the given entity.
    pub fn cards_for_entity(&mut self, entity_id: &str) -> Vec<&mut Card> {
        self.cards
            .iter_mut()
            .filter(|card| card.entity_id == entity_id)
            .collect()
    }
}
