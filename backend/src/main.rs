use axum::{
    extract::State,
    http::StatusCode,
    Json, Router,
    routing::{get, post},
};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::{OsRng, RngCore}},
};
use email_address::EmailAddress;
use serde::{Deserialize, Serialize};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{env, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

const PUBLIC_ID_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

#[derive(Clone)]
struct AppState {
    database: Option<MySqlPool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterRequest {
    display_name: String,
    email: String,
    password: String,
}

#[derive(Serialize)]
struct RegisterResponse {
    public_id: String,
    display_name: String,
}

#[derive(Serialize)]
struct ApiError {
    code: &'static str,
    message: &'static str,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    database: &'static str,
}

async fn health(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    match state.database.as_ref() {
        None => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "online",
                service: "chat-rust-webrtc-api",
                database: "not_configured",
            }),
        ),
        Some(pool) => match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => (
                StatusCode::OK,
                Json(HealthResponse {
                    status: "ok",
                    service: "chat-rust-webrtc-api",
                    database: "connected",
                }),
            ),
            Err(error) => {
                eprintln!("database health check failed: {error}");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(HealthResponse {
                        status: "degraded",
                        service: "chat-rust-webrtc-api",
                        database: "unavailable",
                    }),
                )
            }
        },
    }
}

fn api_error(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
) -> (StatusCode, Json<ApiError>) {
    (status, Json(ApiError { code, message }))
}

fn generate_public_id() -> Option<String> {
    let mut rng = OsRng;
    let mut public_id = String::with_capacity(8);

    while public_id.len() < 8 {
        let mut bytes = [0_u8; 16];
        rng.try_fill_bytes(&mut bytes).ok()?;

        for byte in bytes {
            // 224 is the largest multiple of 32 below 256; rejecting the rest avoids modulo bias.
            if byte < 224 {
                public_id.push(PUBLIC_ID_ALPHABET[usize::from(byte % 32)] as char);
                if public_id.len() == 8 {
                    break;
                }
            }
        }
    }

    Some(public_id)
}

async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), (StatusCode, Json<ApiError>)> {
    let Some(pool) = state.database else {
        return Err(api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "database_unavailable",
            "El servicio de cuentas todavía no está disponible.",
        ));
    };

    let display_name = request.display_name.trim().to_owned();
    let email = request.email.trim().to_lowercase();
    let display_name_length = display_name.chars().count();
    if !(2..=40).contains(&display_name_length)
        || display_name.chars().any(char::is_control)
        || email.chars().count() > 254
        || !EmailAddress::is_valid(&email)
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_account_data",
            "Revisá el nombre visible y el correo electrónico.",
        ));
    }

    let password_length = request.password.chars().count();
    if !(12..=128).contains(&password_length) || request.password.len() > 1024 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_password",
            "La contraseña debe tener entre 12 y 128 caracteres.",
        ));
    }

    // Argon2 es deliberadamente costoso; ejecutarlo fuera del async worker evita bloquear Tokio.
    let password_hash = match tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(request.password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
    })
    .await
    {
        Ok(Ok(hash)) => hash,
        Ok(Err(error)) => {
            eprintln!("password hashing failed: {error}");
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "registration_failed",
                "No se pudo crear la cuenta en este momento.",
            ));
        }
        Err(error) => {
            eprintln!("password hashing task failed: {error}");
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "registration_failed",
                "No se pudo crear la cuenta en este momento.",
            ));
        }
    };

    for _ in 0..3 {
        let Some(public_id) = generate_public_id() else {
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "registration_failed",
                "No se pudo crear la cuenta en este momento.",
            ));
        };

        let result = sqlx::query(
            "INSERT INTO users (public_id, display_name, email, password_hash) VALUES (?, ?, ?, ?)",
        )
        .bind(&public_id)
        .bind(&display_name)
        .bind(&email)
        .bind(&password_hash)
        .execute(&pool)
        .await;

        match result {
            Ok(_) => {
                return Ok((
                    StatusCode::CREATED,
                    Json(RegisterResponse {
                        public_id,
                        display_name,
                    }),
                ));
            }
            Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("1062") => {
                let email_is_used = sqlx::query("SELECT id FROM users WHERE email = ?")
                    .bind(&email)
                    .fetch_optional(&pool)
                    .await
                    .map_err(|error| {
                        eprintln!("duplicate account lookup failed: {error}");
                        api_error(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "registration_failed",
                            "No se pudo crear la cuenta en este momento.",
                        )
                    })?
                    .is_some();

                if email_is_used {
                    return Err(api_error(
                        StatusCode::CONFLICT,
                        "account_conflict",
                        "No se pudo crear la cuenta con esos datos.",
                    ));
                }
            }
            Err(error) => {
                eprintln!("account insert failed: {error}");
                return Err(api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "registration_failed",
                    "No se pudo crear la cuenta en este momento.",
                ));
            }
        }
    }

    Err(api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "registration_failed",
        "No se pudo crear la cuenta en este momento.",
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    let database_url = env::var("DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let database = if let Some(database_url) = database_url {
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        println!("MySQL connected; migrations applied");
        Some(pool)
    } else {
        println!("DATABASE_URL is not set; starting without MySQL");
        None
    };

    let static_dir = env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../frontend/dist"));

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/register", post(register))
        .fallback_service(ServeDir::new(static_dir))
        .with_state(AppState { database });

    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(address).await?;
    println!("chat-rust-webrtc listening on {address}");

    axum::serve(listener, app).await?;
    Ok(())
}
