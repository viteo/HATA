use indexmap::IndexMap;
use ratatui::widgets::ListState;

#[derive(Debug)]
pub enum AppEvent {
    // from backend
    Snapshot {
        entities: Vec<(String, String)>,
    },
    StateChanged { entity_id: String, state: String },
    Status(String),
    Error(String),

    // from ui
    CallService { entity_id: String, service: String },
}

pub struct AppState {
    pub title: &'static str,
    pub entities: IndexMap<String, String>,
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
