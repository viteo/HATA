use std::collections::HashMap;
use ratatui::widgets::ListState;

#[derive(Debug)]
pub enum AppEvent {
    // from backend
    Snapshot {
        title: String,
        views: usize,
        entities: Vec<(String, String)>,
    },
    StateChanged { entity_id: String, state: String },
    Status(String),
    Error(String),

    // from ui
    CallService { entity_id: String, service: String },
}

pub struct AppState {
    pub title: String,
    pub views: usize,
    pub entities: Vec<String>,
    pub states: HashMap<String, String>,
    pub selected: usize,
    pub list_state: ListState,
    pub status: String,
    pub last_error: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            title: "<connecting>".to_string(),
            views: 0,
            entities: Vec::new(),
            states: HashMap::new(),
            selected: 0,
            list_state: ListState::default(),
            status: "Starting…".to_string(),
            last_error: None,
        }
    }
}
