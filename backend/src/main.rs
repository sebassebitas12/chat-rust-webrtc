use chat_rust_webrtc_api::{AppState, RateLimiter, realtime::Hub, router};
use sqlx::mysql::MySqlPoolOptions;
use std::{env, net::SocketAddr};
use tokio::net::TcpListener;

fn normalize_origin(value: &str) -> String {
    let value = value.trim().trim_end_matches('/');
    let Some(scheme_end) = value.find("://") else { return value.to_owned() };
    let scheme = &value[..scheme_end];
    let rest = &value[scheme_end + 3..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    format!("{scheme}://{authority}")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL is required; configure MySQL before starting the service")?;
    let database = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&database).await?;
    println!("MySQL connected; migrations applied");

    let base_url = env::var("APP_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| env::var("RAILWAY_PUBLIC_DOMAIN").ok().map(|domain| format!("https://{domain}")))
        .unwrap_or_else(|| "http://localhost:5173".to_owned());
    let secure_cookie = env::var("COOKIE_SECURE")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or_else(|_| base_url.starts_with("https://"));
    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    let state = AppState {
        db: database,
        hub: Hub::default(),
        rate_limiter: RateLimiter::default(),
        app_origin: normalize_origin(&base_url),
        secure_cookie,
        turn_key_id: env::var("CF_TURN_KEY_ID").ok().filter(|value| !value.trim().is_empty()),
        turn_api_token: env::var("CF_TURN_API_TOKEN").ok().filter(|value| !value.trim().is_empty()),
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?,
    };

    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(address).await?;
    println!("ENTRE chat service listening on {address}");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
