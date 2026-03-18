use crate::types::{app::{AppEvent, Card}, events::*, lovelace::extract_all_cards, responses::Response};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

struct WSClient {
    id: u64,
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ha_version: String,
}

#[allow(dead_code)]
impl WSClient {
    pub async fn new(ws_url: &str) -> Result<Self> {
        let (mut socket, _) = connect_async(ws_url).await.context(format!("Failed to connect to \"{}\"", ws_url))?;

        let ver = match socket.next().await.context("No connection")?? {
            msg => match serde_json::from_str::<Response>(&msg.to_string())? {
                Response::AuthRequired(req) => req.ha_version,
                other => return Err(anyhow::anyhow!("Expected auth_required, got: {:?}", other)),
            },
        };

        Ok(Self {
            id: 0,
            socket,
            ha_version: ver,
        })
    }

    pub async fn auth_longlivedtoken(&mut self, ha_token: &str) -> Result<()> {
        let auth_payload = json!({
            "type": "auth",
            "access_token": ha_token
        });
        self.socket.send(Message::Text(auth_payload.to_string().into())).await?;

        let _ = match self.socket.next().await.context("No connection")?? {
            msg => match serde_json::from_str::<Response>(&msg.to_string())? {
                Response::AuthOk(_) => {}
                Response::AuthInvalid(err) => anyhow::bail!("Authentication failed: {}", err.message),
                unknown => anyhow::bail!("Unexpected response after auth: {:?}", unknown),
            },
        };

        Ok(())
    }

    pub async fn fetch_all_states(&mut self) -> Result<Vec<(String, String)>> {
        self.id += 1;
        // request all entities
        let get_states_payload = json!({
            "id": self.id,
            "type": "get_states"
        });
        self.socket.send(Message::Text(get_states_payload.to_string().into())).await?;

        let states: Vec<State> = serde_json::from_value(self.get_result().await?.unwrap())?;
        Ok(states
            .iter()
            .map(|st| (st.entity_id.clone(), st.state.clone()))
            .collect())
    }

    pub async fn subscribe_all_state_changes(&mut self) -> Result<()> {
        // Subscribe to all state_changed events
        let subscribe_payload = json!({
            "id": self.id,
            "type": "subscribe_events",
            "event_type": "state_changed"
        });
        self.socket.send(Message::Text(subscribe_payload.to_string().into())).await?;
        Ok(())
    }

    pub async fn fetch_lovelace_dashboard(&mut self, dashboard_name: &str) -> Result<Vec<(String, Card)>> {
        self.id += 1;
        let get_states_payload = json!({
            "id": self.id,
            "type": "lovelace/config",
            "url_path": dashboard_name,
            "force": false
        });
        self.socket.send(Message::Text(get_states_payload.to_string().into())).await?;

        let cards = extract_all_cards(&self.get_result().await?.unwrap());
        Ok(cards
            .into_iter()
            .filter_map(|c| {
                let entity_id = c.entity?;
                let domain = entity_id.split_once('.').unwrap().0.to_string();
                Some((entity_id, Card {
                    state: String::new(),
                    friendly_name: String::new(),
                    domain: domain,
                    r#type: c.r#type,
                    services: vec![]
                }))
            })
            .collect())
    }

    pub async fn subscribe_entities(&mut self, entity_ids: Vec<&String>) -> Result<()> {
        self.id += 1;
        let subscribe_payload = json!({
            "id": self.id,
            "type": "subscribe_entities",
            "entity_ids": entity_ids
        });
        self.socket.send(Message::Text(subscribe_payload.to_string().into())).await?;

        _ = self.get_result().await?; // success
        
        Ok(())
    }

    pub async fn fetch_services_for_entity(&mut self, entity_id: &String) -> Result<Vec<String>> {
        self.id += 1;
        let fetch_services_payload = json!({
            "id": self.id,
            "type": "get_services_for_target",
            "target": {
                "entity_id": [entity_id]
            }
        });
        self.socket.send(Message::Text(fetch_services_payload.to_string().into())).await?;

        let services: Vec<String> = serde_json::from_value(self.get_result().await?.unwrap())?;
        let domain = entity_id.split_once('.').unwrap().0;
        Ok(services
            .into_iter()
            .filter(|s|
                s.starts_with(domain))
            .collect())
    }
    
    pub async fn call_service(&mut self, service: &str, entity_id: &str) -> Result<()> {
        self.id += 1;
        let call_service_payload = json!({
            "id": self.id,
            "type": "call_service",
            "domain": entity_id.split_once('.').unwrap().0,
            "service": service,
            "target": {
                "entity_id": entity_id
            }
        });
        self.socket.send(Message::Text(call_service_payload.to_string().into())).await?;

        Ok(())
    }

    async fn get_result(&mut self) -> Result<Option<serde_json::Value>> {
        let msg = self.socket.next().await.context("No connection")??;

        match serde_json::from_str::<Response>(&msg.to_string())? {
            Response::Result(ws_result) if (ws_result.success && ws_result.id == self.id) => {
                Ok(ws_result.result)
            },
            Response::Result(ws) => anyhow::bail!("Wrong response: {:?}", ws),
            other => anyhow::bail!("Unexpected response type: {:?}", other),
        }
    }
}

pub async fn ha_worker(ha_url: &str, ha_token: &str, ui_tx: &mpsc::Sender<AppEvent>, ev_rx: &mut mpsc::Receiver<AppEvent>) -> Result<()> {
    ui_tx.send(AppEvent::Status("Connecting...".to_string())).await?;
    let ws_url = build_ws_url(&ha_url)?;

    let mut ws_client = WSClient::new(&ws_url).await?;
    ui_tx.send(AppEvent::Status(format!("Connected to HA v{}", ws_client.ha_version))).await?;

    ws_client.auth_longlivedtoken(ha_token).await?;

    ui_tx.send(AppEvent::Status("Requesting entities".to_string())).await?;

    let mut entities: Vec<(String, Card)> = ws_client.fetch_lovelace_dashboard("dashboard-tui").await?;
    for (id, card) in &mut entities {
        card.services = ws_client.fetch_services_for_entity(&id).await?;
    }

    ws_client.subscribe_entities(entities.iter().map(|(id,_)| id).collect::<Vec<_>>()).await?;

    ui_tx.send(AppEvent::Snapshot {
        entities: entities,
    }).await?;
    ui_tx.send(AppEvent::Status("Displaying".to_string())).await?;
    
    // event loop
    loop {
        tokio::select! {
            Some(Ok(Message::Text(text))) = ws_client.socket.next() => {
                match serde_json::from_str::<Response>(&text) {
                    Ok(Response::Event(ws_event)) => {
                        match serde_json::from_value::<Event>(ws_event.event) {
                            Ok(Event::NormalEvent(event)) => {
                                if event.event_type == "state_changed" {
                                    let entity_id = event.data.entity_id.unwrap();
                                    if let Some(new_state) = event.data.new_state {
                                        let state = new_state.state;
                                        ui_tx.send(AppEvent::StateChanged { entity_id, state }).await?;
                                    }
                                }
                            }
                            Ok(Event::CompressedEvent(event)) => {
                                if let Some(added) = event.added {
                                    for (entity_id, state) in added{
                                        ui_tx.send(AppEvent::EventAdded { 
                                            friendly_name : state.attributes
                                                .as_ref()
                                                .and_then(|attr| attr.get("friendly_name"))
                                                .and_then(|value| value.as_str())
                                                .unwrap_or(&entity_id).to_string(),
                                            state : state.state,
                                            entity_id
                                        }).await?;
                                    }
                                }
                                if let Some(changed) = event.changed {
                                    for (entity_id, update) in changed {
                                        ui_tx.send(AppEvent::StateChanged { entity_id, state: update.additions.unwrap().state }).await?;
                                    }
                                }
                            }
                            _ => {
                                ui_tx.send(AppEvent::Error("Unsupported event type".to_string())).await?;
                            }
                        }
                    },
                    Ok(Response::Result(ws_result)) => {
                        if ws_result.success == false {
                            ui_tx.send(AppEvent::Error(format!("Result Error: {:?}", ws_result))).await?;
                        }
                    },
                    _ => {
                        ui_tx.send(AppEvent::Error("WS message unparsed".to_string())).await?;
                    }
                }
            },
            Some(cmd) = ev_rx.recv() => {
                match cmd {
                    AppEvent::CallService { entity_id, service } => {
                        ws_client.call_service(&service, &entity_id).await?;
                    },
                    _ => {},
                }
            }
        }
    }
}

fn build_ws_url(base: &str) -> Result<String> {
    let trimmed = base.trim_end_matches('/');
    let ws_base = if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else {
        trimmed.to_string()
    };
    Ok(format!("{}/api/websocket", ws_base))
}