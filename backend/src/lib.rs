pub mod auth;
pub mod realtime;
pub mod social;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
};
use serde::Serialize;
use sqlx::MySqlPool;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tower_http::set_header::SetResponseHeaderLayer;

#[derive(Clone)]
pub struct AppState {
    pub db: MySqlPool,
    pub hub: realtime::Hub,
    pub rate_limiter: RateLimiter,
    pub app_origin: String,
    pub secure_cookie: bool,
    pub turn_key_id: Option<String>,
    pub turn_api_token: Option<String>,
    pub http_client: reqwest::Client,
}

#[derive(Clone, Default)]
pub struct RateLimiter {
    entries: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

impl RateLimiter {
    pub fn allow(&self, key: String, limit: usize, window: Duration) -> bool {
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        let now = Instant::now();
        entries.retain(|_, attempts| {
            attempts.retain(|attempt| now.duration_since(*attempt) < window);
            !attempts.is_empty()
        });
        if !entries.contains_key(&key) && entries.len() >= 20_000 {
            return false;
        }
        let attempts = entries.entry(key).or_default();
        if attempts.len() >= limit {
            return false;
        }
        attempts.push_back(now);
        true
    }
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self { status, code, message }
    }

    pub fn bad_request(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", "Iniciá sesión para continuar.")
    }

    pub fn forbidden(message: &'static str) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }

    pub fn not_found(message: &'static str) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn conflict(message: &'static str) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", message)
    }

    pub fn rate_limited() -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Hiciste varios intentos. Esperá un momento y probá de nuevo.")
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        eprintln!("request failed: {error}");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "No se pudo completar la solicitud.")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody { code: self.code, message: self.message }),
        )
            .into_response()
    }
}

pub fn client_key(headers: &HeaderMap, scope: &str) -> String {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| headers.get("x-real-ip").and_then(|value| value.to_str().ok()))
        .unwrap_or("unknown");
    format!("{scope}:{ip}")
}

pub fn check_origin(headers: &HeaderMap, state: &AppState) -> Result<(), ApiError> {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    let origin = origin.to_str().map_err(|_| ApiError::forbidden("Origen de solicitud no válido."))?;
    if origin != state.app_origin {
        return Err(ApiError::forbidden("Origen de solicitud no permitido."));
    }
    Ok(())
}

pub fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find_map(|(name, value)| (name == "entre_session").then(|| value.to_owned()))
        .filter(|token| token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

pub fn token_hash(token: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub async fn require_user(headers: &HeaderMap, state: &AppState) -> Result<u64, ApiError> {
    let token = session_token(headers).ok_or_else(ApiError::unauthorized)?;
    sqlx::query_scalar::<_, u64>(
        "SELECT user_id FROM sessions WHERE token_hash = ? AND expires_at > UTC_TIMESTAMP(6)",
    )
    .bind(token_hash(&token))
    .fetch_optional(&state.db)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(ApiError::unauthorized)
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/me", get(auth::me))
        .route("/api/v1/profile", patch(auth::update_profile))
        .route("/api/v1/profile/avatar", post(auth::upload_avatar).delete(auth::delete_avatar))
        .route("/api/v1/users/{public_id}/avatar", get(auth::get_avatar))
        .route("/api/v1/users/search", get(social::search_user))
        .route("/api/v1/contacts", get(social::list_contacts))
        .route("/api/v1/contacts/requests", post(social::request_contact))
        .route("/api/v1/contacts/requests/{request_id}/accept", post(social::accept_contact))
        .route("/api/v1/contacts/requests/{request_id}/reject", post(social::reject_contact))
        .route("/api/v1/conversations/{conversation_id}/messages", get(social::list_messages).post(social::send_message))
        .route("/api/v1/calls/ice-config", get(realtime::ice_config))
        .route("/api/v1/realtime", get(realtime::websocket))
        .fallback_service(tower_http::services::ServeDir::new(
            std::env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/dist".to_owned()),
        ))
        .layer(DefaultBodyLimit::max(5 * 1024 * 1024 + 64 * 1024))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::CONTENT_SECURITY_POLICY,
            "default-src 'self'; img-src 'self' blob: data:; media-src 'self' blob:; connect-src 'self' ws: wss:; style-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'".parse().expect("valid content security policy"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            "nosniff".parse().expect("valid header"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::REFERRER_POLICY,
            "strict-origin-when-cross-origin".parse().expect("valid header"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::HeaderName::from_static("permissions-policy"),
            "microphone=(self)".parse().expect("valid header"),
        ))
        .with_state(state)
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    database: &'static str,
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(HealthResponse {
        status: "ok",
        service: "chat-rust-webrtc-api",
        database: "connected",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_cookie_parser_rejects_malformed_tokens() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, "other=value; entre_session=abc".parse().unwrap());
        assert!(session_token(&headers).is_none());

        headers.insert(header::COOKIE, format!("entre_session={}", "a".repeat(64)).parse().unwrap());
        assert_eq!(session_token(&headers), Some("a".repeat(64)));
    }

    #[test]
    fn rate_limiter_keeps_independent_keys_and_enforces_the_limit() {
        let limiter = RateLimiter::default();
        let window = Duration::from_secs(60);
        assert!(limiter.allow("login:client-a".to_owned(), 2, window));
        assert!(limiter.allow("login:client-a".to_owned(), 2, window));
        assert!(!limiter.allow("login:client-a".to_owned(), 2, window));
        assert!(limiter.allow("login:client-b".to_owned(), 2, window));
    }
}
