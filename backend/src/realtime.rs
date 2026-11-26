use crate::{ApiError, AppState, check_origin, require_user};
use axum::{
    Json,
    extract::{State, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::MySqlPool;
use std::{collections::HashMap, sync::{Arc, Mutex}, time::Duration};
use tokio::sync::broadcast;

#[derive(Clone, Default)]
pub struct Hub {
    inner: Arc<Mutex<HubState>>,
}

#[derive(Default)]
struct HubState {
    channels: HashMap<u64, broadcast::Sender<Value>>,
    active_by_user: HashMap<u64, String>,
    calls: HashMap<String, ActiveCall>,
}

struct ActiveCall {
    caller_id: u64,
    callee_id: u64,
    ringing: bool,
    started_at: tokio::time::Instant,
}

impl Hub {
    pub fn subscribe(&self, user_id: u64) -> broadcast::Receiver<Value> {
        let mut state = self.inner.lock().expect("realtime hub lock poisoned");
        state
            .channels
            .entry(user_id)
            .or_insert_with(|| broadcast::channel(64).0)
            .subscribe()
    }

    pub fn send(&self, user_id: u64, event: Value) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|state| state.channels.get(&user_id).map(|sender| sender.send(event).is_ok()))
            .unwrap_or(false)
    }

    pub fn is_online(&self, user_id: u64) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|state| state.channels.get(&user_id).map(|sender| sender.receiver_count() > 0))
            .unwrap_or(false)
    }

    pub fn start_call(&self, caller_id: u64, callee_id: u64) -> Option<String> {
        let mut state = self.inner.lock().ok()?;
        if state.active_by_user.contains_key(&caller_id) || state.active_by_user.contains_key(&callee_id) {
            return None;
        }
        let mut random = [0_u8; 16];
        let mut rng = argon2::password_hash::rand_core::OsRng;
        argon2::password_hash::rand_core::RngCore::try_fill_bytes(&mut rng, &mut random).ok()?;
        let call_id = hex::encode(random);
        state.active_by_user.insert(caller_id, call_id.clone());
        state.active_by_user.insert(callee_id, call_id.clone());
        state.calls.insert(call_id.clone(), ActiveCall {
            caller_id,
            callee_id,
            ringing: true,
            started_at: tokio::time::Instant::now(),
        });
        Some(call_id)
    }

    pub fn accept_call(&self, call_id: &str, actor_id: u64) -> Option<u64> {
        let mut state = self.inner.lock().ok()?;
        let call = state.calls.get_mut(call_id)?;
        if call.callee_id != actor_id || !call.ringing {
            return None;
        }
        call.ringing = false;
        Some(call.caller_id)
    }

    pub fn call_peer(&self, call_id: &str, actor_id: u64) -> Option<u64> {
        let state = self.inner.lock().ok()?;
        let call = state.calls.get(call_id)?;
        if call.caller_id == actor_id {
            Some(call.callee_id)
        } else if call.callee_id == actor_id {
            Some(call.caller_id)
        } else {
            None
        }
    }

    pub fn close_call(&self, call_id: &str, actor_id: u64) -> Option<u64> {
        let mut state = self.inner.lock().ok()?;
        let call = state.calls.get(call_id)?;
        let peer_id = if call.caller_id == actor_id {
            call.callee_id
        } else if call.callee_id == actor_id {
            call.caller_id
        } else {
            return None;
        };
        state.calls.remove(call_id);
        state.active_by_user.remove(&actor_id);
        state.active_by_user.remove(&peer_id);
        Some(peer_id)
    }

    pub fn expire_ringing(&self, call_id: &str) -> Option<(u64, u64)> {
        let mut state = self.inner.lock().ok()?;
        let call = state.calls.get(call_id)?;
        if !call.ringing || call.started_at.elapsed() < Duration::from_secs(45) {
            return None;
        }
        let caller_id = call.caller_id;
        let callee_id = call.callee_id;
        state.calls.remove(call_id);
        state.active_by_user.remove(&caller_id);
        state.active_by_user.remove(&callee_id);
        Some((caller_id, callee_id))
    }

    pub fn disconnect(&self, user_id: u64) -> Vec<(u64, u64)> {
        let Ok(mut state) = self.inner.lock() else { return Vec::new() };
        if state.channels.get(&user_id).is_some_and(|sender| sender.receiver_count() == 0) {
            state.channels.remove(&user_id);
        }
        let call_id = state.active_by_user.get(&user_id).cloned();
        let Some(call_id) = call_id else { return Vec::new() };
        let Some(call) = state.calls.remove(&call_id) else {
            state.active_by_user.remove(&user_id);
            return Vec::new();
        };
        state.active_by_user.remove(&call.caller_id);
        state.active_by_user.remove(&call.callee_id);
        let peer_id = if call.caller_id == user_id { call.callee_id } else { call.caller_id };
        vec![(peer_id, user_id)]
    }
}

#[derive(Deserialize)]
struct ClientEvent {
    #[serde(rename = "type")]
    kind: String,
    target_public_id: Option<String>,
    call_id: Option<String>,
    sdp: Option<Value>,
    candidate: Option<Value>,
}

pub async fn websocket(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    check_origin(&headers, &state)?;
    let user_id = require_user(&headers, &state).await?;
    let session_hash = crate::token_hash(&crate::session_token(&headers).ok_or_else(ApiError::unauthorized)?);
    Ok(ws
        .max_message_size(64 * 1024)
        .max_frame_size(64 * 1024)
        .on_upgrade(move |socket| handle_socket(socket, state, user_id, session_hash)))
}

async fn handle_socket(socket: WebSocket, state: AppState, user_id: u64, session_hash: String) {
    let mut events = state.hub.subscribe(user_id);
    let (mut outgoing, mut incoming) = socket.split();
    let contacts = accepted_contact_ids(&state.db, user_id).await;
    let own_public_id = sqlx::query_scalar::<_, String>("SELECT public_id FROM users WHERE id = ?")
        .bind(user_id).fetch_optional(&state.db).await.ok().flatten();
    if let Some(public_id) = own_public_id.as_deref() {
        for contact_id in &contacts {
            state.hub.send(*contact_id, json!({ "type": "presence", "public_id": public_id, "online": true }));
        }
    }

    let mut keepalive = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            received = incoming.next() => {
                match received {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(event) = serde_json::from_str::<ClientEvent>(text.as_str()) {
                            handle_client_event(&state, user_id, event).await;
                        } else {
                            let _ = outgoing.send(Message::Text(json!({"type":"error","message":"Evento no válido."}).to_string().into())).await;
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if outgoing.send(Message::Pong(payload)).await.is_err() { break; }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(Message::Binary(_))) | Some(Ok(Message::Pong(_))) => {}
                }
            }
            event = events.recv() => {
                match event {
                    Ok(value) => {
                        if outgoing.send(Message::Text(value.to_string().into())).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = keepalive.tick() => {
                let valid = sqlx::query_scalar::<_, i64>(
                    "SELECT EXISTS(SELECT 1 FROM sessions WHERE token_hash = ? AND expires_at > UTC_TIMESTAMP(6))",
                )
                .bind(&session_hash)
                .fetch_one(&state.db)
                .await
                .unwrap_or(0) == 1;
                if !valid { break; }
                if outgoing.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
            }
        }
    }

    drop(events);
    for (peer_id, ended_by) in state.hub.disconnect(user_id) {
        state.hub.send(peer_id, json!({ "type": "call:ended", "ended_by": ended_by }));
    }
    if !state.hub.is_online(user_id) {
        if let Some(public_id) = own_public_id.as_deref() {
            for contact_id in accepted_contact_ids(&state.db, user_id).await {
                state.hub.send(contact_id, json!({ "type": "presence", "public_id": public_id, "online": false }));
            }
        }
    }
}

async fn handle_client_event(state: &AppState, user_id: u64, event: ClientEvent) {
    match event.kind.as_str() {
        "ping" => { state.hub.send(user_id, json!({ "type": "pong" })); }
        "call:invite" => invite_call(state, user_id, event.target_public_id.as_deref().unwrap_or("")).await,
        "call:accept" => {
            if let Some(call_id) = event.call_id.as_deref() {
                if let Some(peer_id) = state.hub.accept_call(call_id, user_id) {
                    state.hub.send(peer_id, json!({ "type": "call:accepted", "call_id": call_id }));
                    state.hub.send(user_id, json!({ "type": "call:accepted", "call_id": call_id }));
                }
            }
        }
        "call:decline" => {
            if let Some(call_id) = event.call_id.as_deref() {
                if let Some(peer_id) = state.hub.close_call(call_id, user_id) {
                    state.hub.send(peer_id, json!({ "type": "call:declined", "call_id": call_id }));
                    state.hub.send(user_id, json!({ "type": "call:ended", "call_id": call_id }));
                }
            }
        }
        "call:end" => {
            if let Some(call_id) = event.call_id.as_deref() {
                if let Some(peer_id) = state.hub.close_call(call_id, user_id) {
                    state.hub.send(peer_id, json!({ "type": "call:ended", "call_id": call_id, "ended_by": user_id }));
                    state.hub.send(user_id, json!({ "type": "call:ended", "call_id": call_id, "ended_by": user_id }));
                }
            }
        }
        "call:offer" | "call:answer" | "call:ice" => {
            let Some(call_id) = event.call_id.as_deref() else { return };
            let Some(peer_id) = state.hub.call_peer(call_id, user_id) else { return };
            let Some(payload) = event.sdp.or(event.candidate) else { return };
            let kind = event.kind.clone();
            state.hub.send(peer_id, json!({ "type": kind, "call_id": call_id, "payload": payload }));
        }
        _ => { state.hub.send(user_id, json!({ "type": "error", "message": "Evento no reconocido." })); }
    }
}

async fn invite_call(state: &AppState, caller_id: u64, target_public_id: &str) {
    let target_public_id = target_public_id.trim().to_ascii_uppercase();
    if target_public_id.len() != 8 {
        state.hub.send(caller_id, json!({ "type": "call:error", "message": "Seleccioná un contacto válido." }));
        return;
    }
    let target = sqlx::query_as::<_, (u64, String, String, bool)>(
        "SELECT id, public_id, display_name, avatar_jpeg IS NOT NULL FROM users WHERE public_id = ?",
    )
    .bind(&target_public_id)
    .fetch_optional(&state.db)
    .await;
    let Ok(Some((callee_id, public_id, display_name, has_avatar))) = target else {
        state.hub.send(caller_id, json!({ "type": "call:error", "message": "No se encontró ese contacto." }));
        return;
    };
    let accepted = sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM contact_requests WHERE status = 'accepted' AND ((requester_id = ? AND recipient_id = ?) OR (requester_id = ? AND recipient_id = ?)))",
    )
    .bind(caller_id).bind(callee_id).bind(callee_id).bind(caller_id)
    .fetch_one(&state.db).await.unwrap_or(0) == 1;
    if !accepted {
        state.hub.send(caller_id, json!({ "type": "call:error", "message": "Solo podés llamar a un contacto aceptado." }));
        return;
    }
    if !state.hub.is_online(callee_id) {
        state.hub.send(caller_id, json!({ "type": "call:error", "message": "Ese contacto está desconectado." }));
        return;
    }
    let Some(call_id) = state.hub.start_call(caller_id, callee_id) else {
        state.hub.send(caller_id, json!({ "type": "call:error", "message": "Una de las dos personas ya está en una llamada." }));
        return;
    };
    let from = public_user(caller_id, &state.db).await;
    let Some(from) = from else {
        let _ = state.hub.close_call(&call_id, caller_id);
        state.hub.send(caller_id, json!({ "type": "call:error", "message": "No se pudo iniciar la llamada." }));
        return;
    };
    state.hub.send(callee_id, json!({ "type": "call:incoming", "call_id": call_id, "from": from }));
    state.hub.send(caller_id, json!({ "type": "call:outgoing", "call_id": call_id, "to": {
        "public_id": public_id,
        "display_name": display_name,
        "avatar_url": has_avatar.then(|| format!("/api/v1/users/{public_id}/avatar")),
        "online": true
    }}));

    let hub = state.hub.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(46)).await;
        if let Some((caller, callee)) = hub.expire_ringing(&call_id) {
            hub.send(caller, json!({ "type": "call:missed", "call_id": call_id }));
            hub.send(callee, json!({ "type": "call:missed", "call_id": call_id }));
        }
    });
}

async fn public_user(user_id: u64, db: &MySqlPool) -> Option<Value> {
    let row = sqlx::query_as::<_, (String, String, bool)>(
        "SELECT public_id, display_name, avatar_jpeg IS NOT NULL FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .ok()??;
    Some(json!({
        "public_id": row.0,
        "display_name": row.1,
        "avatar_url": row.2.then(|| format!("/api/v1/users/{}/avatar", row.0)),
        "online": true
    }))
}

async fn accepted_contact_ids(db: &MySqlPool, user_id: u64) -> Vec<u64> {
    sqlx::query_scalar::<_, u64>(
        "SELECT CASE WHEN requester_id = ? THEN recipient_id ELSE requester_id END FROM contact_requests WHERE status = 'accepted' AND (requester_id = ? OR recipient_id = ?)",
    )
    .bind(user_id).bind(user_id).bind(user_id)
    .fetch_all(db).await.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_signaling_is_limited_to_its_two_participants() {
        let hub = Hub::default();
        let call_id = hub.start_call(10, 20).expect("free users can start a call");

        assert!(hub.start_call(10, 30).is_none());
        assert!(hub.accept_call(&call_id, 30).is_none());
        assert_eq!(hub.call_peer(&call_id, 30), None);
        assert_eq!(hub.accept_call(&call_id, 20), Some(10));
        assert_eq!(hub.call_peer(&call_id, 10), Some(20));
        assert_eq!(hub.close_call(&call_id, 30), None);
        assert_eq!(hub.close_call(&call_id, 20), Some(10));
        assert!(hub.start_call(10, 30).is_some());
    }
}

pub async fn ice_config(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    require_user(&headers, &state).await?;
    let (Some(key_id), Some(api_token)) = (&state.turn_key_id, &state.turn_api_token) else {
        return Ok(Json(json!({ "iceServers": [{ "urls": ["stun:stun.cloudflare.com:3478"] }] })));
    };
    let response = state.http_client
        .post(format!("https://rtc.live.cloudflare.com/v1/turn/keys/{key_id}/credentials/generate-ice-servers"))
        .bearer_auth(api_token)
        .json(&json!({ "ttl": 21_600 }))
        .send().await.map_err(ApiError::internal)?;
    if !response.status().is_success() {
        eprintln!("TURN credential request returned HTTP {}", response.status());
        return Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "turn_unavailable", "No se pudieron preparar los servidores de llamada."));
    }
    let body = response.json::<Value>().await.map_err(ApiError::internal)?;
    if body.get("iceServers").and_then(Value::as_array).is_none() {
        return Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "turn_unavailable", "La respuesta de TURN no es válida."));
    }
    Ok(Json(body))
}
