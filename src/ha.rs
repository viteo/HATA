use crate::types::{
    app::{AppEvent, Card, ConnectionStatus, Section, local_renderer},
    events::*,
    lovelace::{LovelaceSectionConfig, extract_sections},
    responses::Response};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

/// The websocket connection to Home Assistant.
struct WebSockConnection {
    id: u64,
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ha_version: String,
}

#[allow(dead_code)]
impl WebSockConnection {
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

        let states: Vec<HassEntity> = serde_json::from_value(self.get_result().await?.unwrap())?;
        Ok(states
            .iter()
            .map(|st| (st.entity_id.clone(), st.state.clone()))
            .collect())
    }

    pub async fn fetch_lovelace_config(&mut self, url_path: &str) -> Result<(Vec<Card>, Vec<Section>)> {
        self.id += 1;
        let get_states_payload = json!({
            "id": self.id,
            "type": "lovelace/config",
            "url_path": url_path,
            "force": false
        });
        self.socket.send(Message::Text(get_states_payload.to_string().into())).await?;

        let sections = extract_sections(&self.get_result().await?.unwrap());
        Ok(build_cards(&sections))
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

    pub async fn fetch_services(&mut self) -> Result<HashMap<String, Vec<String>>> {
        self.id += 1;
        let fetch_services_payload = json!({
            "id": self.id,
            "type": "get_services"
        });
        self.socket.send(Message::Text(fetch_services_payload.to_string().into())).await?;

        let domains: HashMap<String, serde_json::Map<String, Value>> =
            serde_json::from_value(self.get_result().await?.unwrap())?;
        Ok(domains
            .into_iter()
            .map(|(domain, services)| (domain, services.keys().cloned().collect()))
            .collect())
    }

    pub async fn call_service(&mut self, service: &str, entity_id: &str, data: Option<Value>) -> Result<()> {
        self.id += 1;
        // a service is either "domain.service" or a plain service name of the domain of the entity
        let (domain, name) = match service.split_once('.') {
            Some((domain, name)) => (domain.to_string(), name.to_string()),
            None => (
                service_domain(entity_id),
                service.to_string(),
            ),
        };
        let mut call_service_payload = json!({
            "id": self.id,
            "type": "call_service",
            "domain": domain,
            "service": name,
            "service_data": data.unwrap_or(json!({}))
        });
        // a frontend card has no entity to target
        if !entity_id.starts_with("local.") {
            call_service_payload["target"] = json!({ "entity_id": entity_id });
        }
        self.socket.send(Message::Text(call_service_payload.to_string().into())).await?;

        Ok(())
    }

    async fn get_result(&mut self) -> Result<Option<serde_json::Value>> {
        let msg = self.socket.next().await.context("No connection")??;

        match serde_json::from_str::<Response>(&msg.to_string())? {
            Response::Result(ws_result) if (ws_result.success && ws_result.id == self.id) => {
                Ok(ws_result.result)
            }
            Response::Result(ws) => anyhow::bail!("Wrong response: {:?}", ws),
            other => anyhow::bail!("Unexpected response type: {:?}", other),
        }
    }

    pub async fn ping_pong(&mut self) -> Result<()> {
        self.id += 1;
        let ping_payload = json!({
            "id": self.id,
            "type": "ping"
        });

        self.socket.send(Message::Ping(ping_payload.to_string().into())).await?;

        Ok(())
    }
}

fn build_cards(sections: &[LovelaceSectionConfig]) -> (Vec<Card>, Vec<Section>) {
    let mut entities: Vec<Card> = Vec::new();
    let mut built_sections: Vec<Section> = Vec::new();

    for section in sections {
        let first = entities.len();
        for card in section.flat_cards() {
            let card_type = card.r#type.clone();
            let card_entities = card.entity_configs();
            let is_empty = card_entities.is_empty();
            let (columns, rows) = card.grid_size();

            for entity in card_entities {
                let Some((domain, _)) = entity.entity.split_once('.') else {
                    continue;
                };
                let domain = domain.to_string();
                let mut card = Card::new(entity.entity.clone(), domain, card_type.clone());
                card.name = entity.name.unwrap_or_default();
                card.tap_action = entity.tap_action;
                card.set_size(columns, rows);
                entities.push(card);
            }

            // a frontend card without entity
            if is_empty {
                if let Some(renderer) = local_renderer(&card_type) {
                    let entity_id = format!("local.{}", card_type);
                    // a heading card names itself in "heading"
                    let title = card
                        .title
                        .clone()
                        .or_else(|| card.extra.get("heading").and_then(|v| v.as_str()).map(str::to_string))
                        .unwrap_or_default();
                    let content = card
                        .extra
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    let mut local_card = Card::new(entity_id, "local".to_string(), card_type.clone());
                    local_card.renderer = renderer;
                    local_card.name = title;
                    local_card.state = content.unwrap_or_default();
                    local_card.tap_action = card.tap_action();
                    local_card.set_size(columns, rows);
                    entities.push(local_card);
                }
            }
        }
        built_sections.push(Section {
            column_span: section.column_span.unwrap_or(1),
            cards: (first..entities.len()).collect(),
        });
    }
    (entities, built_sections)
}

/// The domain the services of the entity live in.
fn service_domain(entity_id: &str) -> String {
    let domain = entity_id
        .split_once('.')
        .map(|(domain, _)| domain.to_string())
        .unwrap_or_default();
    if domain == "group" {
        "homeassistant".to_string()
    } else {
        domain
    }
}

pub async fn ha_worker(ha_url: &str, ha_token: &str, dashboard: &str, ui_tx: &mpsc::Sender<AppEvent>, cmd_rx: &mut mpsc::Receiver<AppEvent>) -> Result<()> {
    ui_tx.send(AppEvent::Status(ConnectionStatus::Connecting)).await?;
    let ws_url = build_ws_url(&ha_url)?;

    let mut ws_client = WebSockConnection::new(&ws_url).await?;

    if let Err(e) = ws_client.auth_longlivedtoken(ha_token).await {
        let _ = ui_tx.send(AppEvent::Status(ConnectionStatus::AuthInvalid)).await;
        return Err(e);
    }
    ui_tx.send(AppEvent::Status(ConnectionStatus::Connected(ws_client.ha_version.clone()))).await?;

    let (mut cards, sections) = ws_client.fetch_lovelace_config(dashboard).await?;
    let services = ws_client.fetch_services().await?;
    for card in &mut cards {
        let domain = card.domain.clone();
        card.services = services.get(&domain).cloned().unwrap_or_default();
    }

    // subscribe once per entity
    let mut entity_ids: Vec<String> = Vec::new();
    for card in &cards {
        if !entity_ids.contains(&card.entity_id) {
            entity_ids.push(card.entity_id.clone());
        }
    }
    ws_client
        .subscribe_entities(entity_ids.iter().collect::<Vec<_>>())
        .await?;

    ui_tx.send(AppEvent::Snapshot { cards, sections }).await?;

    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(30));
    // event loop
    loop {
        tokio::select! {
            ws_result = ws_client.socket.next() => {
                let text = match ws_result {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(_)) => continue,
                    Some(Err(err)) => {
                        let _ = ui_tx.send(AppEvent::Error(format!("WebSocket error: {}", err))).await;
                        let _ = ui_tx.send(AppEvent::Status(ConnectionStatus::Disconnected)).await;
                        break;
                    }
                    None => {
                        let _ = ui_tx.send(AppEvent::Error("WebSocket closed by HA".to_string())).await;
                        let _ = ui_tx.send(AppEvent::Status(ConnectionStatus::Disconnected)).await;
                        break;
                    }
                };
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
                                            ui_tx.send(AppEvent::EntityAdded {
                                                name : state.attributes
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
            cmd = cmd_rx.recv() => {
                if let Some(cmd) = cmd {
                    match cmd {
                        AppEvent::CallService { entity_id, service, data } => {
                            ws_client.call_service(&service, &entity_id, data).await?;
                        },
                        _ => {},
                    }
                }
            },
            _ = heartbeat.tick() => {
                ws_client.ping_pong().await?;
            }
        }
    }
    Ok(())
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
