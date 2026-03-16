use crate::{
    app::AppEvent,
    types::{events::*, lovelace::extract_all_cards, responses::Response},
};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub async fn ha_worker(ha_url: &str, ha_token: &str, ui_tx: &mpsc::Sender<AppEvent>) -> Result<()> {
    ui_tx.send(AppEvent::Status("Connecting...".to_string())).await?;
    let ws_url = build_ws_url(&ha_url)?;
    let (mut socket, _) = connect_async(ws_url).await.context("Failed to connect")?;

    // Wait for auth_required
    let text = socket.next().await.unwrap()?.to_string();
    let response: Response = serde_json::from_str(&text).context("Failed to parse first message (auth_required)")?;

    let Response::AuthRequired(req) = response else {
        return Err(anyhow::anyhow!("Expected auth_required, got: {:?}", response));
    };
    ui_tx.send(AppEvent::Status(format!("Connected to HA v{}", req.ha_version))).await?;

    // Send auth
    let auth_payload = json!({
        "type": "auth",
        "access_token": ha_token
    });
    socket.send(Message::Text(auth_payload.to_string().into())).await?;

    // Wait for auth_ok
    let text = socket.next().await.unwrap()?.to_string();
    let response: Response = serde_json::from_str(&text)?;
    match response {
        Response::AuthOk(auth) => ui_tx.send(AppEvent::Status(format!("Authenticated to {}", auth.ha_version))).await?,
        Response::AuthInvalid(err) => anyhow::bail!("Authentication failed: {}", err.message),
        unknown => anyhow::bail!("Unexpected response after auth: {:?}", unknown),
    }

    ui_tx.send(AppEvent::Status("Requesting entities".to_string())).await?;

    // request all entities
    // let get_states_payload = json!({
    //     "id": 1,
    //     "type": "get_states"
    // });
    let get_states_payload = json!({
        "id": 1,
        "type": "lovelace/config",
        "url_path": "dashboard-tui",
        "force": false
    });
    socket.send(Message::Text(get_states_payload.to_string().into())).await?;

    // Subscribe to all state_changed events
    // let subscribe_payload = json!({
    //     "id": 2,
    //     "type": "subscribe_events",
    //     "event_type": "state_changed"
    // });
    // socket
    //     .send(Message::Text(subscribe_payload.to_string().into()))
    //     .await?;

    // parser loop
    while let Some(msg) = socket.next().await {
        if let Message::Text(text) = msg? {
            let value: Value = serde_json::from_str(&text)?;

            match serde_json::from_value::<Response>(value) {
                Ok(Response::Result(ws_result)) => {
                    if ws_result.success && ws_result.result.is_some() {
                        let value = ws_result.result.unwrap();

                        let mut entities: Vec<(String, String)> = if value.is_object() {
                            let cards = extract_all_cards(&value);

                            cards
                                .iter()
                                .filter_map(|c| {
                                    let en = c.entity.clone()?;
                                    Some((en, "TBU".to_string()))
                                })
                                .collect()
                        } else {
                            let states: Vec<State> = serde_json::from_value(value)?;

                            states
                                .iter()
                                .map(|st| (st.entity_id.clone(), st.state.clone()))
                                .collect()
                        };
                        let subscribe_payload = json!({
                            "id": 2,
                            "type": "subscribe_entities",
                            "entity_ids": entities.iter().map(|(id,_)| id).collect::<Vec<_>>()
                        });
                        socket.send(Message::Text(subscribe_payload.to_string().into())).await?;

                        entities.sort_by(|a, b| a.0.cmp(&b.0));

                        ui_tx.send(AppEvent::Snapshot {
                                title: "Home Assistant".to_string(),
                                views: 0,
                                entities,
                            }).await?;
                        ui_tx.send(AppEvent::Status("Displaying".to_string())).await?;

                    } else if let Some(err) = ws_result.error {
                        ui_tx.send(AppEvent::Error(format!("Response failed with {}", err))).await?;
                    }
                }
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
                                    ui_tx.send(AppEvent::StateChanged { entity_id, state : state.state }).await?;
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
                }
                _ => {
                    ui_tx.send(AppEvent::Error("WS message unparsed".to_string())).await?;
                }
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
