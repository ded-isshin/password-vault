use std::{env, net::SocketAddr};

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;
use sqlx::PgPool;

pub mod auth;
pub mod db;

#[derive(Clone, Debug)]
pub struct ApiConfig {
    pub bind_addr: SocketAddr,
    database_url: Option<String>,
    pub require_database: bool,
    pub run_migrations_on_startup: bool,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_addr = env::var("PV_BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidBindAddr)?;

        let require_database = match env::var("PV_REQUIRE_DATABASE") {
            Ok(value) => parse_bool(&value).ok_or(ConfigError::InvalidRequireDatabase)?,
            Err(_) => false,
        };

        let database_url_present = env::var("PV_DATABASE_URL").ok().and_then(nonempty_string);

        let run_migrations_on_startup = match env::var("PV_RUN_MIGRATIONS_ON_STARTUP") {
            Ok(value) => parse_bool(&value).ok_or(ConfigError::InvalidRunMigrationsOnStartup)?,
            Err(_) => false,
        };

        Ok(Self {
            bind_addr,
            database_url: database_url_present,
            require_database,
            run_migrations_on_startup,
        })
    }

    pub fn local_test(require_database: bool, database_url_present: bool) -> Self {
        Self {
            bind_addr: "127.0.0.1:0"
                .parse()
                .expect("hard-coded test socket address is valid"),
            database_url: database_url_present
                .then(|| "postgres://test:test@127.0.0.1:5432/test".to_string()),
            require_database,
            run_migrations_on_startup: false,
        }
    }

    pub fn database_url_present(&self) -> bool {
        self.database_url.is_some()
    }

    fn database_url(&self) -> Option<&str> {
        self.database_url.as_deref()
    }
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidBindAddr,
    InvalidRequireDatabase,
    InvalidRunMigrationsOnStartup,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBindAddr => write!(formatter, "PV_BIND_ADDR must be a socket address"),
            Self::InvalidRequireDatabase => {
                write!(formatter, "PV_REQUIRE_DATABASE must be true or false")
            }
            Self::InvalidRunMigrationsOnStartup => {
                write!(
                    formatter,
                    "PV_RUN_MIGRATIONS_ON_STARTUP must be true or false"
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug)]
pub enum ApiInitError {
    Database(sqlx::Error),
    Migration(sqlx::migrate::MigrateError),
}

impl std::fmt::Display for ApiInitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(_) => write!(formatter, "database connection failed"),
            Self::Migration(_) => write!(formatter, "database migration failed"),
        }
    }
}

impl std::error::Error for ApiInitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Migration(error) => Some(error),
        }
    }
}

impl From<sqlx::Error> for ApiInitError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl From<sqlx::migrate::MigrateError> for ApiInitError {
    fn from(error: sqlx::migrate::MigrateError) -> Self {
        Self::Migration(error)
    }
}

#[derive(Clone)]
struct AppState {
    config: ApiConfig,
    database: Option<PgPool>,
}

pub fn app(config: ApiConfig) -> Router {
    router(AppState {
        config,
        database: None,
    })
}

pub async fn build_app(config: ApiConfig) -> Result<Router, ApiInitError> {
    let database = if let Some(database_url) = config.database_url() {
        let pool = if config.run_migrations_on_startup {
            let pool = db::connect(database_url).await?;
            db::run_migrations(&pool).await?;
            pool
        } else {
            db::connect_lazy(database_url)?
        };
        Some(pool)
    } else {
        None
    };

    Ok(router(AppState { config, database }))
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .with_state(state)
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "password-vault-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Serialize)]
struct ReadyResponse {
    status: &'static str,
    checks: Vec<ReadyCheck>,
}

#[derive(Serialize)]
struct ReadyCheck {
    name: &'static str,
    status: &'static str,
}

async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<ReadyResponse>) {
    let database_status = database_readiness(&state).await;
    let database_ready = matches!(database_status, "ok");

    let status = if database_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body_status = if database_ready { "ready" } else { "not_ready" };

    (
        status,
        Json(ReadyResponse {
            status: body_status,
            checks: vec![ReadyCheck {
                name: "database_config",
                status: database_status,
            }],
        }),
    )
}

async fn database_readiness(state: &AppState) -> &'static str {
    if let Some(pool) = &state.database {
        return match db::ping(pool).await {
            Ok(()) => "ok",
            Err(_) => "unavailable",
        };
    }

    if state.config.require_database && state.config.database_url_present() {
        "unavailable"
    } else if state.config.require_database {
        "missing"
    } else {
        "ok"
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn nonempty_string(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "password_vault_api=info".into()))
        .try_init();
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, body::to_bytes, http::Request};
    use tower::ServiceExt;

    use super::{ApiConfig, app, build_app};

    #[tokio::test]
    async fn healthz_returns_ok_without_database() {
        let response = app(ApiConfig::local_test(false, false))
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("\"status\":\"ok\""));
        assert!(body.contains("\"service\":\"password-vault-api\""));
    }

    #[tokio::test]
    async fn readyz_returns_ok_when_database_is_not_required() {
        let response = app(ApiConfig::local_test(false, false))
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("\"status\":\"ready\""));
        assert!(body.contains("\"database_config\""));
    }

    #[tokio::test]
    async fn readyz_returns_503_when_required_database_is_missing() {
        let response = app(ApiConfig::local_test(true, false))
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 503);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("\"status\":\"not_ready\""));
        assert!(body.contains("\"status\":\"missing\""));
    }

    #[tokio::test]
    async fn readyz_returns_503_when_required_database_url_has_no_pool() {
        let response = app(ApiConfig::local_test(true, true))
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 503);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("\"status\":\"not_ready\""));
        assert!(body.contains("\"status\":\"unavailable\""));
    }

    #[tokio::test]
    async fn readyz_returns_503_when_configured_database_is_unreachable() {
        let config = ApiConfig {
            bind_addr: "127.0.0.1:0"
                .parse()
                .expect("hard-coded test socket address is valid"),
            database_url: Some("postgres://test:test@127.0.0.1:1/test".to_string()),
            require_database: true,
            run_migrations_on_startup: false,
        };

        let response = build_app(config)
            .await
            .expect("lazy database pool should not require an immediate connection")
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 503);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("\"status\":\"not_ready\""));
        assert!(body.contains("\"status\":\"unavailable\""));
    }
}
