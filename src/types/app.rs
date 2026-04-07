use indexmap::IndexMap;

#[derive(Debug)]
pub struct Card {
    pub friendly_name: String,
    pub state: String,
    pub domain: String,
    pub services: Vec<String>,
    pub r#type: String,
}

impl Card {
    pub fn preferred_action(&self) -> Option<String> {

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
}
