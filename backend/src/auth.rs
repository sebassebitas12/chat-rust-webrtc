use crate::{ApiError, AppState, check_origin, client_key, require_user, token_hash};
use argon2::{Argon2, PasswordHash, PasswordVerifier, password_hash::{PasswordHasher, SaltString, rand_core::{OsRng, RngCore}}};
use axum::{
    Json,
    body::Bytes,
    extract::{Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use email_address::EmailAddress;
use image::{ImageFormat, ImageReader, codecs::jpeg::JpegEncoder};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use std::{io::Cursor, time::Duration};

const SESSION_TTL_SECONDS: u64 = 60 * 60 * 24 * 7;
const MAX_AVATAR_BYTES: usize = 5 * 1024 * 1024;
const PUBLIC_ID_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
    display_name: String,
    email: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateProfileRequest {
    display_name: String,
}

#[derive(Clone, Serialize)]
pub struct AccountView {
    pub public_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Serialize)]
struct AuthResponse {
    user: AccountView,
}

fn valid_display_name(name: &str) -> bool {
    let length = name.chars().count();
    (2..=40).contains(&length) && !name.chars().any(char::is_control)
}

fn generate_public_id() -> Option<String> {
    let mut rng = OsRng;
    let mut bytes = [0_u8; 16];
    let mut value = String::with_capacity(8);
    while value.len() < 8 {
        rng.try_fill_bytes(&mut bytes).ok()?;
        for byte in bytes {
            if byte < 224 {
                value.push(PUBLIC_ID_ALPHABET[usize::from(byte % 32)] as char);
                if value.len() == 8 { break; }
            }
        }
    }
    Some(value)
}

fn new_session_token() -> Result<String, ApiError> {
    let mut bytes = [0_u8; 32];
    OsRng.try_fill_bytes(&mut bytes).map_err(ApiError::internal)?;
    Ok(hex::encode(bytes))
}

async fn hash_password_async(password: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default().hash_password(password.as_bytes(), &salt).map(|hash| hash.to_string())
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::internal)
}

async fn verify_password(password: String, stored_hash: String) -> Result<bool, ApiError> {
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&stored_hash).map_err(ApiError::internal)?;
        Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
    })
    .await
    .map_err(ApiError::internal)?
}

async fn insert_session(user_id: u64, db: &MySqlPool) -> Result<String, ApiError> {
    let token = new_session_token()?;
    sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, expires_at) VALUES (?, ?, DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 7 DAY))",
    )
    .bind(user_id)
    .bind(token_hash(&token))
    .execute(db)
    .await
    .map_err(ApiError::internal)?;
    Ok(token)
}

fn response_with_session(status: StatusCode, body: AuthResponse, token: &str, secure: bool) -> Response {
    let secure_flag = if secure { "; Secure" } else { "" };
    let cookie = format!("entre_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_TTL_SECONDS}{secure_flag}");
    let mut response = (status, Json(body)).into_response();
    response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).expect("hex session cookie is valid"));
    response
}

fn clear_session_cookie(secure: bool) -> Response {
    let secure_flag = if secure { "; Secure" } else { "" };
    let cookie = format!("entre_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure_flag}");
    let mut response = (StatusCode::NO_CONTENT, ()).into_response();
    response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).expect("static cookie is valid"));
    response
}

async fn account_view(user_id: u64, db: &MySqlPool) -> Result<AccountView, ApiError> {
    let row = sqlx::query_as::<_, (String, String, bool)>(
        "SELECT public_id, display_name, avatar_jpeg IS NOT NULL FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(ApiError::unauthorized)?;
    Ok(AccountView {
        avatar_url: row.2.then(|| format!("/api/v1/users/{}/avatar", row.0)),
        public_id: row.0,
        display_name: row.1,
    })
}

pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegisterRequest>,
) -> Result<Response, ApiError> {
    check_origin(&headers, &state)?;
    if !state.rate_limiter.allow(client_key(&headers, "register"), 5, Duration::from_secs(3600)) {
        return Err(ApiError::rate_limited());
    }
    let display_name = request.display_name.trim().to_owned();
    let email = request.email.trim().to_lowercase();
    if !valid_display_name(&display_name) || email.chars().count() > 254 || !EmailAddress::is_valid(&email) {
        return Err(ApiError::bad_request("Revisá el nombre visible y el correo electrónico."));
    }
    let password_length = request.password.chars().count();
    if !(12..=128).contains(&password_length) || request.password.len() > 1024 {
        return Err(ApiError::bad_request("La contraseña debe tener entre 12 y 128 caracteres."));
    }
    let hash = hash_password_async(request.password).await?;
    let token = new_session_token()?;
    let mut tx = state.db.begin().await.map_err(ApiError::internal)?;
    let mut created = None;
    for _ in 0..3 {
        let public_id = generate_public_id().ok_or_else(|| ApiError::internal("secure random generation failed"))?;
        match sqlx::query("INSERT INTO users (public_id, display_name, email, password_hash) VALUES (?, ?, ?, ?)")
            .bind(&public_id).bind(&display_name).bind(&email).bind(&hash)
            .execute(&mut *tx).await
        {
            Ok(result) => { created = Some((result.last_insert_id(), public_id)); break; }
            Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("1062") => {
                let email_used = sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM users WHERE email = ?)")
                    .bind(&email).fetch_one(&mut *tx).await.map_err(ApiError::internal)? == 1;
                if email_used { return Err(ApiError::conflict("No se pudo crear la cuenta con esos datos.")); }
            }
            Err(error) => return Err(ApiError::internal(error)),
        }
    }
    let (user_id, public_id) = created.ok_or_else(|| ApiError::internal("could not allocate public id"))?;
    sqlx::query("INSERT INTO sessions (user_id, token_hash, expires_at) VALUES (?, ?, DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 7 DAY))")
        .bind(user_id).bind(token_hash(&token)).execute(&mut *tx).await.map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(response_with_session(
        StatusCode::CREATED,
        AuthResponse { user: AccountView { public_id, display_name, avatar_url: None } },
        &token,
        state.secure_cookie,
    ))
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    check_origin(&headers, &state)?;
    if !state.rate_limiter.allow(client_key(&headers, "login"), 8, Duration::from_secs(300)) {
        return Err(ApiError::rate_limited());
    }
    let email = request.email.trim().to_lowercase();
    if email.chars().count() > 254 || !EmailAddress::is_valid(&email) || request.password.len() > 1024 {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "invalid_credentials", "El correo o la contraseña no coinciden."));
    }
    let account = sqlx::query_as::<_, (u64, String, String, String, bool)>(
        "SELECT id, public_id, display_name, password_hash, avatar_jpeg IS NOT NULL FROM users WHERE email = ?",
    )
    .bind(&email).fetch_optional(&state.db).await.map_err(ApiError::internal)?;
    let Some((user_id, public_id, display_name, stored_hash, has_avatar)) = account else {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "invalid_credentials", "El correo o la contraseña no coinciden."));
    };
    if !verify_password(request.password, stored_hash).await? {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "invalid_credentials", "El correo o la contraseña no coinciden."));
    }
    let token = insert_session(user_id, &state.db).await?;
    Ok(response_with_session(
        StatusCode::OK,
        AuthResponse { user: AccountView {
            public_id: public_id.clone(),
            display_name,
            avatar_url: has_avatar.then(|| format!("/api/v1/users/{public_id}/avatar")),
        } },
        &token,
        state.secure_cookie,
    ))
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    check_origin(&headers, &state)?;
    if let Some(token) = crate::session_token(&headers) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(token_hash(&token)).execute(&state.db).await.map_err(ApiError::internal)?;
    }
    Ok(clear_session_cookie(state.secure_cookie))
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<AccountView>, ApiError> {
    let user_id = require_user(&headers, &state).await?;
    Ok(Json(account_view(user_id, &state.db).await?))
}

pub async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateProfileRequest>,
) -> Result<Json<AccountView>, ApiError> {
    check_origin(&headers, &state)?;
    let user_id = require_user(&headers, &state).await?;
    let display_name = request.display_name.trim();
    if !valid_display_name(display_name) {
        return Err(ApiError::bad_request("El nombre visible debe tener entre 2 y 40 caracteres."));
    }
    sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
        .bind(display_name).bind(user_id).execute(&state.db).await.map_err(ApiError::internal)?;
    Ok(Json(account_view(user_id, &state.db).await?))
}

pub async fn upload_avatar(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<AccountView>, ApiError> {
    check_origin(&headers, &state)?;
    let user_id = require_user(&headers, &state).await?;
    if !state.rate_limiter.allow(client_key(&headers, "avatar"), 12, Duration::from_secs(3600)) {
        return Err(ApiError::rate_limited());
    }
    let mut image_data: Option<Bytes> = None;
    while let Some(field) = multipart.next_field().await.map_err(|_| ApiError::bad_request("El archivo enviado no es válido."))? {
        if field.name() == Some("avatar") {
            if image_data.is_some() { return Err(ApiError::bad_request("Enviá una sola foto.")); }
            let bytes = field.bytes().await.map_err(|_| ApiError::bad_request("No se pudo leer la foto."))?;
            if bytes.is_empty() || bytes.len() > MAX_AVATAR_BYTES {
                return Err(ApiError::bad_request("La foto debe pesar entre 1 byte y 5 MB."));
            }
            image_data = Some(bytes);
        }
    }
    let bytes = image_data.ok_or_else(|| ApiError::bad_request("Falta el campo de foto 'avatar'."))?;
    let reader = ImageReader::new(Cursor::new(bytes.as_ref())).with_guessed_format()
        .map_err(|_| ApiError::bad_request("Usá una imagen JPG, PNG o WebP válida."))?;
    let format = reader.format().ok_or_else(|| ApiError::bad_request("No se reconoció el formato de imagen."))?;
    if !matches!(format, ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP) {
        return Err(ApiError::bad_request("Usá una imagen JPG, PNG o WebP."));
    }
    let (width, height) = reader.into_dimensions().map_err(|_| ApiError::bad_request("La imagen está dañada."))?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 16_000_000 {
        return Err(ApiError::bad_request("La imagen debe tener como máximo 16 megapíxeles."));
    }
    let decoded = image::load_from_memory_with_format(bytes.as_ref(), format)
        .map_err(|_| ApiError::bad_request("La imagen está dañada."))?;
    let resized = decoded.thumbnail(512, 512);
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 82).encode_image(&resized)
        .map_err(|_| ApiError::bad_request("No se pudo procesar la imagen."))?;
    sqlx::query("UPDATE users SET avatar_jpeg = ? WHERE id = ?")
        .bind(jpeg).bind(user_id).execute(&state.db).await.map_err(ApiError::internal)?;
    Ok(Json(account_view(user_id, &state.db).await?))
}

pub async fn delete_avatar(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    check_origin(&headers, &state)?;
    let user_id = require_user(&headers, &state).await?;
    sqlx::query("UPDATE users SET avatar_jpeg = NULL WHERE id = ?")
        .bind(user_id).execute(&state.db).await.map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_avatar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<String>,
) -> Result<Response, ApiError> {
    let user_id = require_user(&headers, &state).await?;
    let image = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT target.avatar_jpeg FROM users target WHERE target.public_id = ? AND target.avatar_jpeg IS NOT NULL AND (target.id = ? OR EXISTS (SELECT 1 FROM contact_requests cr WHERE cr.status = 'accepted' AND ((cr.requester_id = ? AND cr.recipient_id = target.id) OR (cr.requester_id = target.id AND cr.recipient_id = ?))))",
    )
    .bind(public_id.to_ascii_uppercase()).bind(user_id).bind(user_id).bind(user_id)
    .fetch_optional(&state.db).await.map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found("No se encontró esa foto."))?;
    let mut response = (StatusCode::OK, image).into_response();
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=300"));
    response.headers_mut().insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_ids_use_the_expected_alphabet_and_length() {
        for _ in 0..128 {
            let id = generate_public_id().expect("operating system random source is available");
            assert_eq!(id.len(), 8);
            assert!(id.bytes().all(|byte| PUBLIC_ID_ALPHABET.contains(&byte)));
        }
    }

    #[test]
    fn session_tokens_are_random_64_character_hex_strings() {
        let first = new_session_token().expect("operating system random source is available");
        let second = new_session_token().expect("operating system random source is available");
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }
}
