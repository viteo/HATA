use indexmap::IndexMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererKind {
    Clock,
    Binary,
    NumericTrend,
    Default,
}

#[derive(Debug)]
pub struct Card {
    pub friendly_name: String,
    pub state: String,
    pub domain: String,
    pub services: Vec<String>,
    pub renderer: RendererKind,
    pub has_trend: bool,
    pub history: Option<Vec<f32>>,
}

impl Card {
    pub fn new(domain: String, card_type: String) -> Self {
        let renderer = Self::resolve_renderer(&domain, &card_type);
        let has_trend = renderer == RendererKind::NumericTrend;
        Self {
            friendly_name: String::new(),
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
