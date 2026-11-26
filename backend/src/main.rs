use axum::{
    extract::State,
    http::StatusCode,
    Json, Router,
    routing::get,
};
use serde::Serialize;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{env, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    database: Option<MySqlPool>,
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
        .fallback_service(ServeDir::new(static_dir))
        .with_state(AppState { database });

    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(address).await?;
    println!("chat-rust-webrtc listening on {address}");

    axum::serve(listener, app).await?;
    Ok(())
}
