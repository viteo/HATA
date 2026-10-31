use ratatui::layout::Rect;
use serde_json::Value;

use super::actions;
use crate::layout::{DashboardLayout, FULL, ROW_LINES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererKind {
    Clock,
    Binary,
    NumericTrend,
    Markdown,
    Heading,
    Default,
}

/// entirely frontend card has no state and need no entity
pub fn local_renderer(card_type: &str) -> Option<RendererKind> {
    match card_type {
        "clock" => Some(RendererKind::Clock),
        "markdown" => Some(RendererKind::Markdown),
        "heading" => Some(RendererKind::Heading),
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
    /// The width on the grid of a section, in cells.
    pub columns: u16,
    /// The height on the grid, in rows.
    pub rows: u16,
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
            columns: 0,
            rows: 0,
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

    /// Sets the size on the grid, taking the default of the card for what is not given.
    pub fn set_size(&mut self, columns: Option<u16>, rows: Option<u16>) {
        let (default_columns, default_rows) = match self.renderer {
            // a heading is a line between the rows
            RendererKind::Heading => (FULL, 0),
            RendererKind::Markdown => {
                // as many rows as the text needs
                let lines = self.state.lines().count() as u16 + u16::from(!self.name.is_empty());
                (FULL, (lines + 1).div_ceil(ROW_LINES).max(1))
            }
            RendererKind::NumericTrend => (6, 2),
            _ => (6, 1),
        };
        self.columns = columns.unwrap_or(default_columns);
        self.rows = rows.unwrap_or(default_rows);
    }

    /// Whether the card can be selected; a heading only when it has an action.
    pub fn is_focusable(&self) -> bool {
        self.renderer != RendererKind::Heading || self.tap_action.is_some()
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

/// A group of cards of the dashboard, as the sections of the web app.
#[derive(Debug)]
pub struct Section {
    /// The width in columns of sections.
    pub column_span: u16,
    /// The indices of its cards.
    pub cards: Vec<usize>,
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
        sections: Vec<Section>,
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
    pub sections: Vec<Section>,
    /// Where the cards are, as last drawn.
    pub layout: DashboardLayout,
    pub selected: usize,
    /// The first line of the dashboard on the screen.
    pub scroll: u16,
    /// Where the dashboard is on the screen, as last drawn.
    pub dashboard_area: Rect,
    pub status: ConnectionStatus,
    pub last_error: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            title: "Home Assistant",
            cards: Vec::new(),
            sections: Vec::new(),
            layout: DashboardLayout::default(),
            selected: 0,
            scroll: 0,
            dashboard_area: Rect::default(),
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
