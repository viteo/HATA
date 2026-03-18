use indexmap::IndexMap;
use ratatui::widgets::ListState;

#[derive(Debug)]
pub struct Card {
    pub friendly_name: String,
    pub state: String,
    pub domain: String,
    pub services: Vec<String>,
    pub r#type: String,
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
    CallService { entity_id: String, service: String },
}

pub struct AppState {
    pub title: &'static str,
    pub entities: IndexMap<String, Card>, // entity_id -> card
    pub selected: ListState,
    pub status: String,
    pub last_error: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            title: "Home Assistant",
            entities: IndexMap::new(),
            selected: ListState::default(),
            status: "<connecting>".to_string(),
            last_error: None,
        }
    }
}
