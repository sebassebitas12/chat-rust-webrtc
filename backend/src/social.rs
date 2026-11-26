use crate::{ApiError, AppState, check_origin, client_key, require_user};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::MySqlPool;
use std::time::Duration;

#[derive(Clone, Serialize)]
pub struct UserCard {
    pub public_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub online: bool,
}

#[derive(Serialize)]
pub struct ContactRequestView {
    pub id: u64,
    pub user: UserCard,
}

#[derive(Serialize)]
pub struct ContactView {
    #[serde(flatten)]
    pub user: UserCard,
    pub conversation_id: u64,
    pub last_message: Option<String>,
    pub last_message_at: Option<String>,
}

#[derive(Serialize)]
pub struct ContactsResponse {
    pub contacts: Vec<ContactView>,
    pub incoming_requests: Vec<ContactRequestView>,
    pub outgoing_requests: Vec<ContactRequestView>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactRequestBody {
    target_public_id: String,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub user: UserCard,
    pub relationship: &'static str,
}

#[derive(Deserialize)]
pub struct MessageCursor {
    before: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendMessageRequest {
    body: String,
}

#[derive(Clone, Serialize)]
pub struct MessageView {
    pub id: u64,
    pub sender_public_id: String,
    pub sender_name: String,
    pub body: String,
    pub sent_at: String,
    pub mine: bool,
}

fn user_card(row: (u64, String, String, bool), state: &AppState, reveal_avatar: bool) -> UserCard {
    UserCard {
        public_id: row.1.clone(),
        display_name: row.2,
        avatar_url: (row.3 && reveal_avatar).then(|| format!("/api/v1/users/{}/avatar", row.1)),
        online: state.hub.is_online(row.0),
    }
}

pub async fn search_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<SearchResult>, ApiError> {
    let user_id = require_user(&headers, &state).await?;
    if !state.rate_limiter.allow(client_key(&headers, "search"), 30, Duration::from_secs(60)) {
        return Err(ApiError::rate_limited());
    }
    let public_id = query.get("public_id").map(|value| value.trim().to_ascii_uppercase()).unwrap_or_default();
    if public_id.len() != 8 || !public_id.bytes().all(|byte| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(&byte)) {
        return Err(ApiError::bad_request("Ingresá un ID válido de 8 caracteres."));
    }
    let row = sqlx::query_as::<_, (u64, String, String, bool)>(
        "SELECT id, public_id, display_name, avatar_jpeg IS NOT NULL FROM users WHERE public_id = ? AND id <> ?",
    )
    .bind(&public_id).bind(user_id).fetch_optional(&state.db).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("No encontramos una cuenta con ese ID."))?;
    let relationship = sqlx::query_scalar::<_, String>(
        "SELECT status FROM contact_requests WHERE (requester_id = ? AND recipient_id = ?) OR (requester_id = ? AND recipient_id = ?) ORDER BY status = 'accepted' DESC, created_at DESC LIMIT 1",
    )
    .bind(user_id).bind(row.0).bind(row.0).bind(user_id)
    .fetch_optional(&state.db).await.map_err(ApiError::internal)?;
    let (label, reveal_avatar) = match relationship.as_deref() {
        Some("accepted") => ("accepted", true),
        Some("pending") => {
            let inbound = sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM contact_requests WHERE requester_id = ? AND recipient_id = ? AND status = 'pending')")
                .bind(row.0).bind(user_id).fetch_one(&state.db).await.map_err(ApiError::internal)? == 1;
            (if inbound { "incoming" } else { "outgoing" }, false)
        }
        Some("rejected") => ("none", false),
        _ => ("none", false),
    };
    Ok(Json(SearchResult { user: user_card(row, &state, reveal_avatar), relationship: label }))
}

pub async fn list_contacts(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<ContactsResponse>, ApiError> {
    let user_id = require_user(&headers, &state).await?;
    let rows = sqlx::query_as::<_, (u64, String, String, bool, u64, Option<String>, Option<String>)>(
        "SELECT u.id, u.public_id, u.display_name, u.avatar_jpeg IS NOT NULL, c.id, (SELECT m.body FROM messages m WHERE m.conversation_id = c.id ORDER BY m.id DESC LIMIT 1), (SELECT DATE_FORMAT(m.created_at, '%Y-%m-%dT%H:%i:%s.%fZ') FROM messages m WHERE m.conversation_id = c.id ORDER BY m.id DESC LIMIT 1) FROM contact_requests cr JOIN users u ON u.id = CASE WHEN cr.requester_id = ? THEN cr.recipient_id ELSE cr.requester_id END JOIN conversations c ON c.participant_a = LEAST(cr.requester_id, cr.recipient_id) AND c.participant_b = GREATEST(cr.requester_id, cr.recipient_id) WHERE cr.status = 'accepted' AND (cr.requester_id = ? OR cr.recipient_id = ?) ORDER BY u.display_name",
    )
    .bind(user_id).bind(user_id).bind(user_id).fetch_all(&state.db).await.map_err(ApiError::internal)?;
    let contacts = rows.into_iter().map(|(id, public_id, display_name, has_avatar, conversation_id, last_message, last_message_at)| {
        ContactView {
            user: user_card((id, public_id, display_name, has_avatar), &state, true),
            conversation_id,
            last_message,
            last_message_at,
        }
    }).collect();

    let incoming_rows = sqlx::query_as::<_, (u64, u64, String, String, bool)>(
        "SELECT cr.id, u.id, u.public_id, u.display_name, u.avatar_jpeg IS NOT NULL FROM contact_requests cr JOIN users u ON u.id = cr.requester_id WHERE cr.recipient_id = ? AND cr.status = 'pending' ORDER BY cr.created_at DESC",
    )
    .bind(user_id).fetch_all(&state.db).await.map_err(ApiError::internal)?;
    let incoming_requests = incoming_rows.into_iter().map(|(id, user_id, public_id, name, has_avatar)| ContactRequestView {
        id,
        user: user_card((user_id, public_id, name, has_avatar), &state, false),
    }).collect();

    let outgoing_rows = sqlx::query_as::<_, (u64, u64, String, String, bool)>(
        "SELECT cr.id, u.id, u.public_id, u.display_name, u.avatar_jpeg IS NOT NULL FROM contact_requests cr JOIN users u ON u.id = cr.recipient_id WHERE cr.requester_id = ? AND cr.status = 'pending' ORDER BY cr.created_at DESC",
    )
    .bind(user_id).fetch_all(&state.db).await.map_err(ApiError::internal)?;
    let outgoing_requests = outgoing_rows.into_iter().map(|(id, user_id, public_id, name, has_avatar)| ContactRequestView {
        id,
        user: user_card((user_id, public_id, name, has_avatar), &state, false),
    }).collect();
    Ok(Json(ContactsResponse { contacts, incoming_requests, outgoing_requests }))
}

pub async fn request_contact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ContactRequestBody>,
) -> Result<StatusCode, ApiError> {
    check_origin(&headers, &state)?;
    let requester_id = require_user(&headers, &state).await?;
    if !state.rate_limiter.allow(client_key(&headers, "contact_request"), 12, Duration::from_secs(3600)) {
        return Err(ApiError::rate_limited());
    }
    let target_public_id = request.target_public_id.trim().to_ascii_uppercase();
    if target_public_id.len() != 8 || !target_public_id.bytes().all(|byte| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(&byte)) {
        return Err(ApiError::bad_request("Ingresá un ID válido de 8 caracteres."));
    }
    let recipient_id = sqlx::query_scalar::<_, u64>("SELECT id FROM users WHERE public_id = ?")
        .bind(&target_public_id).fetch_optional(&state.db).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("No encontramos una cuenta con ese ID."))?;
    if requester_id == recipient_id {
        return Err(ApiError::bad_request("No podés enviarte una solicitud a vos mismo."));
    }

    let mut tx = state.db.begin().await.map_err(ApiError::internal)?;
    let _locked = sqlx::query("SELECT id FROM users WHERE id IN (?, ?) ORDER BY id FOR UPDATE")
        .bind(requester_id).bind(recipient_id).fetch_all(&mut *tx).await.map_err(ApiError::internal)?;
    let existing = sqlx::query_as::<_, (u64, u64, u64, String)>(
        "SELECT id, requester_id, recipient_id, status FROM contact_requests WHERE (requester_id = ? AND recipient_id = ?) OR (requester_id = ? AND recipient_id = ?) LIMIT 1",
    )
    .bind(requester_id).bind(recipient_id).bind(recipient_id).bind(requester_id)
    .fetch_optional(&mut *tx).await.map_err(ApiError::internal)?;
    let request_id = match existing {
        Some((_, _, _, ref status)) if status == "accepted" => return Err(ApiError::conflict("Ya son contactos.")),
        Some((_, _, _, ref status)) if status == "pending" => return Err(ApiError::conflict("Ya existe una solicitud pendiente entre ustedes.")),
        Some((id, old_requester, old_recipient, _)) => {
            sqlx::query("UPDATE contact_requests SET requester_id = ?, recipient_id = ?, status = 'pending', created_at = CURRENT_TIMESTAMP(6) WHERE id = ?")
                .bind(requester_id).bind(recipient_id).bind(id).execute(&mut *tx).await.map_err(ApiError::internal)?;
            let _ = (old_requester, old_recipient);
            id
        }
        None => {
            sqlx::query("INSERT INTO contact_requests (requester_id, recipient_id, status) VALUES (?, ?, 'pending')")
                .bind(requester_id).bind(recipient_id).execute(&mut *tx).await.map_err(ApiError::internal)?.last_insert_id()
        }
    };
    tx.commit().await.map_err(ApiError::internal)?;
    if let Some(user) = lookup_card(requester_id, &state.db, &state).await {
        state.hub.send(recipient_id, json!({ "type": "contact:request", "request": { "id": request_id, "user": user } }));
    }
    Ok(StatusCode::CREATED)
}

pub async fn accept_contact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    update_contact_request(&state, &headers, request_id, true).await
}

pub async fn reject_contact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    update_contact_request(&state, &headers, request_id, false).await
}

async fn update_contact_request(state: &AppState, headers: &HeaderMap, request_id: u64, accept: bool) -> Result<StatusCode, ApiError> {
    check_origin(headers, state)?;
    let recipient_id = require_user(headers, state).await?;
    let mut tx = state.db.begin().await.map_err(ApiError::internal)?;
    let requester_id = sqlx::query_scalar::<_, u64>(
        "SELECT requester_id FROM contact_requests WHERE id = ? AND recipient_id = ? AND status = 'pending' FOR UPDATE",
    )
    .bind(request_id).bind(recipient_id).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("La solicitud ya no está pendiente."))?;
    if accept {
        sqlx::query("UPDATE contact_requests SET status = 'accepted' WHERE id = ?")
            .bind(request_id).execute(&mut *tx).await.map_err(ApiError::internal)?;
        let participant_a = requester_id.min(recipient_id);
        let participant_b = requester_id.max(recipient_id);
        sqlx::query("INSERT IGNORE INTO conversations (participant_a, participant_b) VALUES (?, ?)")
            .bind(participant_a).bind(participant_b).execute(&mut *tx).await.map_err(ApiError::internal)?;
    } else {
        sqlx::query("UPDATE contact_requests SET status = 'rejected' WHERE id = ?")
            .bind(request_id).execute(&mut *tx).await.map_err(ApiError::internal)?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    let event = if accept { "contact:accepted" } else { "contact:rejected" };
    state.hub.send(requester_id, json!({ "type": event, "request_id": request_id }));
    state.hub.send(recipient_id, json!({ "type": event, "request_id": request_id }));
    Ok(StatusCode::NO_CONTENT)
}

async fn lookup_card(user_id: u64, db: &MySqlPool, state: &AppState) -> Option<UserCard> {
    let row = sqlx::query_as::<_, (u64, String, String, bool)>(
        "SELECT id, public_id, display_name, avatar_jpeg IS NOT NULL FROM users WHERE id = ?",
    )
    .bind(user_id).fetch_optional(db).await.ok()??;
    Some(user_card(row, state, false))
}

pub async fn list_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(conversation_id): Path<u64>,
    Query(cursor): Query<MessageCursor>,
) -> Result<Json<Vec<MessageView>>, ApiError> {
    let user_id = require_user(&headers, &state).await?;
    ensure_conversation_member(&state.db, conversation_id, user_id).await?;
    let rows = if let Some(before) = cursor.before {
        sqlx::query_as::<_, (u64, String, String, String, String)>(
            "SELECT m.id, u.public_id, u.display_name, m.body, DATE_FORMAT(m.created_at, '%Y-%m-%dT%H:%i:%s.%fZ') FROM messages m JOIN users u ON u.id = m.sender_id WHERE m.conversation_id = ? AND m.id < ? ORDER BY m.id DESC LIMIT 100",
        )
        .bind(conversation_id).bind(before).fetch_all(&state.db).await.map_err(ApiError::internal)?
    } else {
        sqlx::query_as::<_, (u64, String, String, String, String)>(
            "SELECT m.id, u.public_id, u.display_name, m.body, DATE_FORMAT(m.created_at, '%Y-%m-%dT%H:%i:%s.%fZ') FROM messages m JOIN users u ON u.id = m.sender_id WHERE m.conversation_id = ? ORDER BY m.id DESC LIMIT 100",
        )
        .bind(conversation_id).fetch_all(&state.db).await.map_err(ApiError::internal)?
    };
    let own_public_id = current_public_id(user_id, &state.db).await.unwrap_or_default();
    let mut messages = rows.into_iter().map(|(id, sender_public_id, sender_name, body, sent_at)| MessageView {
        id, mine: sender_public_id == own_public_id,
        sender_public_id, sender_name, body, sent_at,
    }).collect::<Vec<_>>();
    messages.reverse();
    Ok(Json(messages))
}

pub async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(conversation_id): Path<u64>,
    Json(request): Json<SendMessageRequest>,
) -> Result<(StatusCode, Json<MessageView>), ApiError> {
    check_origin(&headers, &state)?;
    let user_id = require_user(&headers, &state).await?;
    let body = request.body.trim().to_owned();
    let length = body.chars().count();
    if length == 0 || length > 4000 || body.chars().any(|character| character.is_control() && character != '\n' && character != '\t') {
        return Err(ApiError::bad_request("El mensaje debe tener entre 1 y 4000 caracteres."));
    }
    let (participant_a, participant_b) = ensure_conversation_member(&state.db, conversation_id, user_id).await?;
    let id = sqlx::query("INSERT INTO messages (conversation_id, sender_id, body) VALUES (?, ?, ?)")
        .bind(conversation_id).bind(user_id).bind(&body).execute(&state.db).await.map_err(ApiError::internal)?.last_insert_id();
    let sender = crate::auth::me(State(state.clone()), headers.clone()).await?.0;
    let sent_at = sqlx::query_scalar::<_, String>("SELECT DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%s.%fZ') FROM messages WHERE id = ?")
        .bind(id).fetch_one(&state.db).await.map_err(ApiError::internal)?;
    let mine = MessageView {
        id, sender_public_id: sender.public_id.clone(), sender_name: sender.display_name.clone(), body: body.clone(), sent_at,
        mine: true,
    };
    state.hub.send(user_id, json!({ "type": "message:new", "conversation_id": conversation_id, "message": mine }));
    let recipient_id = if participant_a == user_id { participant_b } else { participant_a };
    let mut for_recipient = mine.clone();
    for_recipient.mine = false;
    state.hub.send(recipient_id, json!({ "type": "message:new", "conversation_id": conversation_id, "message": for_recipient }));
    Ok((StatusCode::CREATED, Json(mine)))
}

async fn ensure_conversation_member(db: &MySqlPool, conversation_id: u64, user_id: u64) -> Result<(u64, u64), ApiError> {
    sqlx::query_as::<_, (u64, u64)>(
        "SELECT participant_a, participant_b FROM conversations WHERE id = ? AND (participant_a = ? OR participant_b = ?)",
    )
    .bind(conversation_id).bind(user_id).bind(user_id).fetch_optional(db).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("No encontramos esa conversación."))
}

async fn current_public_id(user_id: u64, db: &MySqlPool) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT public_id FROM users WHERE id = ?").bind(user_id)
        .fetch_optional(db).await.ok().flatten()
}
