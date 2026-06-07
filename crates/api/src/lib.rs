use std::{env, net::SocketAddr, sync::OnceLock};

use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
};
use axum_prometheus::{EndpointLabel, PrometheusMetricLayer, PrometheusMetricLayerBuilder};
use metrics_exporter_prometheus::PrometheusHandle;
use serde::Serialize;
use sqlx::PgPool;

pub mod auth;
pub mod db;

#[derive(Clone)]
pub struct ApiConfig {
    pub bind_addr: SocketAddr,
    database_url: Option<String>,
    synthetic_metadata_key: Option<[u8; 32]>,
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
        let synthetic_metadata_key = env::var("PV_SYNTHETIC_METADATA_KEY_B64")
            .ok()
            .and_then(nonempty_string)
            .map(|value| auth::encoding::decode_base64url_array::<32>(&value))
            .transpose()
            .map_err(|_| ConfigError::InvalidSyntheticMetadataKey)?;

        let run_migrations_on_startup = match env::var("PV_RUN_MIGRATIONS_ON_STARTUP") {
            Ok(value) => parse_bool(&value).ok_or(ConfigError::InvalidRunMigrationsOnStartup)?,
            Err(_) => false,
        };

        Ok(Self {
            bind_addr,
            database_url: database_url_present,
            synthetic_metadata_key,
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
            synthetic_metadata_key: None,
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

    fn synthetic_metadata_key(&self) -> Option<&[u8; 32]> {
        self.synthetic_metadata_key.as_ref()
    }
}

impl std::fmt::Debug for ApiConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ApiConfig")
            .field("bind_addr", &self.bind_addr)
            .field(
                "database_url",
                &self.database_url.as_ref().map(|_| "<configured>"),
            )
            .field(
                "synthetic_metadata_key",
                &self.synthetic_metadata_key.as_ref().map(|_| "<redacted>"),
            )
            .field("require_database", &self.require_database)
            .field("run_migrations_on_startup", &self.run_migrations_on_startup)
            .finish()
    }
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidBindAddr,
    InvalidRequireDatabase,
    InvalidRunMigrationsOnStartup,
    InvalidSyntheticMetadataKey,
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
            Self::InvalidSyntheticMetadataKey => {
                write!(
                    formatter,
                    "PV_SYNTHETIC_METADATA_KEY_B64 must be 32 bytes encoded as base64url without padding"
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
    let (metrics_layer, metrics_handle) = metrics_layer_and_handle();

    Router::new()
        .route("/", get(index))
        .route("/assets/app.css", get(app_css))
        .route("/assets/app.js", get(app_js))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(move || metrics(metrics_handle.clone())))
        .merge(auth::routes::router())
        .with_state(state)
        .layer(metrics_layer)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn app_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../static/app.css"),
    )
}

async fn app_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../static/app.js"),
    )
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

async fn metrics(metrics_handle: PrometheusHandle) -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        metrics_handle.render(),
    )
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
    let synthetic_metadata_key_status = synthetic_metadata_key_readiness(&state);
    let synthetic_metadata_key_ready = matches!(synthetic_metadata_key_status, "ok");

    let status = if database_ready && synthetic_metadata_key_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body_status = if database_ready && synthetic_metadata_key_ready {
        "ready"
    } else {
        "not_ready"
    };

    (
        status,
        Json(ReadyResponse {
            status: body_status,
            checks: vec![
                ReadyCheck {
                    name: "database_config",
                    status: database_status,
                },
                ReadyCheck {
                    name: "synthetic_metadata_key",
                    status: synthetic_metadata_key_status,
                },
            ],
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

fn synthetic_metadata_key_readiness(state: &AppState) -> &'static str {
    if state.config.database_url_present() || state.config.require_database {
        if state.config.synthetic_metadata_key().is_some() {
            "ok"
        } else {
            "missing"
        }
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

fn metrics_layer_and_handle() -> (PrometheusMetricLayer<'static>, PrometheusHandle) {
    static METRICS: OnceLock<(PrometheusMetricLayer<'static>, PrometheusHandle)> = OnceLock::new();
    let (layer, handle) = METRICS.get_or_init(|| {
        PrometheusMetricLayerBuilder::new()
            .with_endpoint_label_type(EndpointLabel::MatchedPathWithFallbackFn(
                unmatched_endpoint_label,
            ))
            .with_default_metrics()
            .build_pair()
    });
    (layer.clone(), handle.clone())
}

fn unmatched_endpoint_label(_: &str) -> String {
    "/<unmatched>".to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use axum::{body::Body, body::to_bytes, http::Request};
    use serde_json::Value;
    use tokio::sync::{Mutex, MutexGuard};
    use tower::ServiceExt;

    use crate::{
        auth::encoding::{decode_base64url, encode_base64url},
        db,
    };

    use super::{ApiConfig, app, build_app};

    static DB_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
    async fn metrics_records_low_cardinality_http_metrics() {
        let app = app(ApiConfig::local_test(false, false));

        let health_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health_response.status(), 200);

        for path in [
            "/not-found-cardinality-probe-a",
            "/not-found-cardinality-probe-b",
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), 404);
        }

        let metrics_response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(metrics_response.status(), 200);
        assert_eq!(
            metrics_response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/plain; version=0.0.4; charset=utf-8")
        );

        let body = to_bytes(metrics_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("axum_http_requests_total"));
        assert!(body.contains("endpoint=\"/healthz\""));
        assert!(body.contains("endpoint=\"/<unmatched>\""));
        assert!(body.contains("method=\"GET\""));
        assert!(!body.contains("not-found-cardinality-probe"));
        assert!(!body.contains("login_handle"));
    }

    #[tokio::test]
    async fn browser_preview_assets_are_served() {
        let app = app(ApiConfig::local_test(false, false));

        let index_response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(index_response.status(), 200);
        let index_body = to_bytes(index_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let index_body = String::from_utf8(index_body.to_vec()).unwrap();
        assert!(index_body.contains("Password Vault"));
        assert!(index_body.contains("/assets/app.css"));

        let css_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/assets/app.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(css_response.status(), 200);
        assert_eq!(
            css_response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/css; charset=utf-8")
        );

        let js_response = app
            .oneshot(
                Request::builder()
                    .uri("/assets/app.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(js_response.status(), 200);
        assert_eq!(
            js_response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("application/javascript; charset=utf-8")
        );
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
            synthetic_metadata_key: None,
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

    #[tokio::test]
    async fn auth_start_routes_persist_challenges_and_hide_account_existence() {
        let Some(database_url) = std::env::var("PV_TEST_DATABASE_URL").ok() else {
            eprintln!("skipping auth route database test because PV_TEST_DATABASE_URL is not set");
            return;
        };
        let _guard = db_test_guard().await;

        let pool = db::connect(&database_url)
            .await
            .expect("test database must be reachable");
        db::run_migrations(&pool)
            .await
            .expect("migrations must apply cleanly");
        reset_auth_route_test_data(&pool).await;
        insert_test_account(
            &pool,
            "00000000-0000-0000-0000-000000000901",
            "known@example.com",
        )
        .await;

        let router = build_app(ApiConfig {
            bind_addr: "127.0.0.1:0"
                .parse()
                .expect("hard-coded test socket address is valid"),
            database_url: Some(database_url.clone()),
            synthetic_metadata_key: Some([9u8; 32]),
            require_database: true,
            run_migrations_on_startup: false,
        })
        .await
        .expect("app builds with database");

        let client_nonce = encode_base64url(&[0x11; 32]);
        let known_response = router
            .clone()
            .oneshot(json_request(
                "/v1/auth/login/start",
                &format!(
                    r#"{{
                    "login_handle":"known@example.com",
                    "auth_protocol":"derived-auth-v1",
                    "client_nonce":"{client_nonce}"
                }}"#
                ),
            ))
            .await
            .expect("known login request succeeds");
        let missing_response = router
            .clone()
            .oneshot(json_request(
                "/v1/auth/login/start",
                &format!(
                    r#"{{
                    "login_handle":"missing@example.com",
                    "auth_protocol":"derived-auth-v1",
                    "client_nonce":"{client_nonce}"
                }}"#
                ),
            ))
            .await
            .expect("missing login request succeeds");

        assert_eq!(known_response.status(), 200);
        assert_eq!(missing_response.status(), 200);
        assert_eq!(
            known_response.headers().get("cache-control").unwrap(),
            "no-store"
        );
        assert_eq!(
            missing_response.headers().get("cache-control").unwrap(),
            "no-store"
        );

        let known_body = response_json(known_response).await;
        let missing_body = response_json(missing_response).await;
        assert_same_json_shape(&known_body, &missing_body);
        assert!(known_body.get("mfa_required_hint").is_none());
        assert!(missing_body.get("mfa_required_hint").is_none());
        assert_combined_nonce_is_client_then_server(&known_body, &[0x11; 32]);
        assert_combined_nonce_is_client_then_server(&missing_body, &[0x11; 32]);
        assert_login_challenge_persisted(&pool, "known@example.com", false).await;
        assert_login_challenge_persisted(&pool, "missing@example.com", true).await;

        let register_known = router
            .clone()
            .oneshot(json_request(
                "/v1/auth/register/start",
                r#"{"login_handle":"known@example.com","auth_protocol":"derived-auth-v1"}"#,
            ))
            .await
            .expect("duplicate register start succeeds");
        let register_new = router
            .clone()
            .oneshot(json_request(
                "/v1/auth/register/start",
                r#"{"login_handle":"new@example.com","auth_protocol":"derived-auth-v1"}"#,
            ))
            .await
            .expect("new register start succeeds");
        assert_eq!(register_known.status(), 200);
        assert_eq!(register_new.status(), 200);
        assert_eq!(
            register_known.headers().get("cache-control").unwrap(),
            "no-store"
        );
        assert_eq!(
            register_new.headers().get("cache-control").unwrap(),
            "no-store"
        );
        let register_known_body = response_json(register_known).await;
        let register_new_body = response_json(register_new).await;
        assert_same_json_shape(&register_known_body, &register_new_body);
        assert_eq!(account_count(&pool).await, 1);
        assert_register_challenge_persisted(&pool, "known@example.com").await;
        assert_register_challenge_persisted(&pool, "new@example.com").await;

        let bad_json_response = app(ApiConfig::local_test(false, false))
            .oneshot(json_request(
                "/v1/auth/login/start",
                r#"{"login_handle":"user@example.com","auth_protocol":"derived-auth-v1","client_nonce":"bad","extra":true}"#,
            ))
            .await
            .expect("bad request returns a response");
        assert_eq!(bad_json_response.status(), 400);
        assert_eq!(
            bad_json_response.headers().get("cache-control").unwrap(),
            "no-store"
        );
        let bad_json_body = response_json(bad_json_response).await;
        assert_eq!(bad_json_body["error"]["code"], "bad_request");

        assert_auth_start_rate_limit(&router).await;
    }

    #[tokio::test]
    async fn auth_register_finish_creates_account_key_material_and_setup_session() {
        let Some(database_url) = std::env::var("PV_TEST_DATABASE_URL").ok() else {
            eprintln!(
                "skipping register finish database test because PV_TEST_DATABASE_URL is not set"
            );
            return;
        };
        let _guard = db_test_guard().await;

        let pool = db::connect(&database_url)
            .await
            .expect("test database must be reachable");
        db::run_migrations(&pool)
            .await
            .expect("migrations must apply cleanly");
        reset_auth_route_test_data(&pool).await;

        let router = build_app(ApiConfig {
            bind_addr: "127.0.0.1:0"
                .parse()
                .expect("hard-coded test socket address is valid"),
            database_url: Some(database_url.clone()),
            synthetic_metadata_key: Some([9u8; 32]),
            require_database: true,
            run_migrations_on_startup: false,
        })
        .await
        .expect("app builds with database");

        let start_response = router
            .clone()
            .oneshot(json_request(
                "/v1/auth/register/start",
                r#"{"login_handle":"finish@example.com","auth_protocol":"derived-auth-v1"}"#,
            ))
            .await
            .expect("register start returns a response");
        assert_eq!(start_response.status(), 200);
        let start_body = response_json(start_response).await;
        let registration_id = start_body["registration_id"]
            .as_str()
            .expect("registration id is present");

        let account_keyset_nonce = encode_base64url(&[0x11; 12]);
        let account_keyset_ciphertext = encode_base64url(&[0x22; 48]);
        let vault_key_nonce = encode_base64url(&[0x33; 12]);
        let vault_key_ciphertext = encode_base64url(&[0x44; 48]);
        let auth_stored_key = encode_base64url(&[0x55; 32]);
        let auth_server_key = encode_base64url(&[0x66; 32]);
        let vault_id = "00000000-0000-4000-8000-000000000777";
        let finish_request = format!(
            r#"{{
                "registration_id":"{registration_id}",
                "auth_protocol":"derived-auth-v1",
                "auth_stored_key":"{auth_stored_key}",
                "auth_server_key":"{auth_server_key}",
                "encrypted_account_keyset":{{
                    "crypto_version":"account-keyset-v1",
                    "key_id":"user-key-v1",
                    "nonce":"{account_keyset_nonce}",
                    "ciphertext":"{account_keyset_ciphertext}"
                }},
                "initial_vault":{{
                    "vault_id":"{vault_id}",
                    "encrypted_vault_key":{{
                        "crypto_version":"vault-key-wrap-v1",
                        "key_id":"user-key-v1",
                        "nonce":"{vault_key_nonce}",
                        "ciphertext":"{vault_key_ciphertext}"
                    }}
                }},
                "device":{{
                    "label":"Firefox on laptop",
                    "client_type":"browser",
                    "public_metadata":{{"platform_hint":"web"}}
                }}
            }}"#
        );

        let finish_response = router
            .clone()
            .oneshot(json_request("/v1/auth/register/finish", &finish_request))
            .await
            .expect("register finish returns a response");
        assert_eq!(finish_response.status(), http::StatusCode::CREATED);
        assert_eq!(
            finish_response.headers().get("cache-control").unwrap(),
            "no-store"
        );
        let set_cookie = finish_response
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("register finish sets a session cookie");
        assert!(set_cookie.starts_with("__Host-pv_session="));
        assert!(set_cookie.contains("Path=/"));
        assert!(set_cookie.contains("Secure"));
        assert!(set_cookie.contains("HttpOnly"));
        assert!(set_cookie.contains("SameSite=Strict"));
        assert!(!set_cookie.contains("Domain="));

        let finish_body = response_json(finish_response).await;
        assert_eq!(finish_body["session"]["state"], "mfa_enrollment_required");
        assert_eq!(finish_body["session"]["vault_access"], false);
        assert_eq!(finish_body["next_step"], "enroll_totp");
        assert_register_finish_persisted(&pool, "finish@example.com", vault_id).await;

        let reused_response = router
            .clone()
            .oneshot(json_request("/v1/auth/register/finish", &finish_request))
            .await
            .expect("reused register finish returns a response");
        assert_eq!(reused_response.status(), http::StatusCode::CONFLICT);
        let reused_body = response_json(reused_response).await;
        assert_eq!(reused_body["error"]["code"], "registration_unavailable");
        assert_eq!(account_count(&pool).await, 1);
    }

    fn json_request(uri: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("test request builds")
    }

    async fn db_test_guard() -> MutexGuard<'static, ()> {
        DB_TEST_LOCK.get_or_init(|| Mutex::new(())).lock().await
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("response body reads");
        serde_json::from_slice(&body).expect("response is JSON")
    }

    fn assert_same_json_shape(left: &Value, right: &Value) {
        match (left, right) {
            (Value::Object(left), Value::Object(right)) => {
                let mut left_keys = left.keys().collect::<Vec<_>>();
                let mut right_keys = right.keys().collect::<Vec<_>>();
                left_keys.sort();
                right_keys.sort();
                assert_eq!(left_keys, right_keys);
                for key in left_keys {
                    assert_same_json_shape(&left[key], &right[key]);
                }
            }
            (Value::Array(left), Value::Array(right)) => {
                assert_eq!(left.len(), right.len());
            }
            (Value::String(left), Value::String(right)) => {
                assert_eq!(left.len(), right.len());
            }
            (Value::Number(_), Value::Number(_))
            | (Value::Bool(_), Value::Bool(_))
            | (Value::Null, Value::Null) => {}
            _ => panic!("JSON shape mismatch: {left:?} vs {right:?}"),
        }
    }

    fn assert_combined_nonce_is_client_then_server(body: &Value, client_nonce: &[u8; 32]) {
        let combined_nonce = decode_base64url(body["combined_nonce"].as_str().unwrap())
            .expect("combined nonce is base64url");
        let server_nonce =
            decode_base64url(body["server_nonce"].as_str().unwrap()).expect("server nonce decodes");

        assert_eq!(server_nonce.len(), 32);
        assert_eq!(combined_nonce.len(), 64);
        assert_eq!(&combined_nonce[..32], client_nonce);
        assert_eq!(&combined_nonce[32..], server_nonce.as_slice());
    }

    async fn assert_login_challenge_persisted(
        pool: &sqlx::PgPool,
        login_handle: &str,
        synthetic: bool,
    ) {
        let row: (String, Value, i64) = sqlx::query_as(
            "
            SELECT challenge_type, public_metadata, octet_length(server_nonce)::bigint
            FROM auth_challenges
            WHERE login_handle_normalized = $1
              AND challenge_type = 'login'
            ORDER BY created_at DESC
            LIMIT 1
            ",
        )
        .bind(login_handle)
        .fetch_one(pool)
        .await
        .expect("login challenge row exists");

        assert_eq!(row.0, "login");
        assert_eq!(row.1["synthetic"], synthetic);
        assert_eq!(
            row.1["client_nonce"].as_str().unwrap(),
            encode_base64url(&[0x11; 32])
        );
        assert_eq!(row.2, 32);
    }

    async fn assert_register_challenge_persisted(pool: &sqlx::PgPool, login_handle: &str) {
        let row: (String, Value, i64) = sqlx::query_as(
            "
            SELECT challenge_type, public_metadata, octet_length(server_nonce)::bigint
            FROM auth_challenges
            WHERE login_handle_normalized = $1
              AND challenge_type = 'register'
            ORDER BY created_at DESC
            LIMIT 1
            ",
        )
        .bind(login_handle)
        .fetch_one(pool)
        .await
        .expect("register challenge row exists");

        assert_eq!(row.0, "register");
        assert_eq!(row.1["auth_verifier_profile"], "pv-scram-sha-256-v1");
        assert_eq!(row.1["account_salt"].as_str().unwrap().len(), 43);
        assert_eq!(row.1["auth_verifier_salt"].as_str().unwrap().len(), 43);
        assert_eq!(row.2, 32);
    }

    async fn assert_register_finish_persisted(
        pool: &sqlx::PgPool,
        login_handle: &str,
        vault_id: &str,
    ) {
        let account_id: uuid::Uuid =
            sqlx::query_scalar("SELECT id FROM accounts WHERE login_handle_normalized = $1")
                .bind(login_handle)
                .fetch_one(pool)
                .await
                .expect("registered account exists");

        let account_keysets: i64 = sqlx::query_scalar(
            "
            SELECT COUNT(*)
            FROM account_keysets
            WHERE account_id = $1
              AND crypto_version = 'account-keyset-v1'
              AND key_id = 'user-key-v1'
              AND octet_length(nonce) = 12
              AND octet_length(ciphertext) = 48
            ",
        )
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("account keyset query succeeds");
        assert_eq!(account_keysets, 1);

        let vault_key_wraps: i64 = sqlx::query_scalar(
            "
            SELECT COUNT(*)
            FROM vault_key_wraps
            WHERE account_id = $1
              AND vault_id = $2::uuid
              AND crypto_version = 'vault-key-wrap-v1'
              AND key_id = 'user-key-v1'
              AND octet_length(nonce) = 12
              AND octet_length(ciphertext) = 48
            ",
        )
        .bind(account_id)
        .bind(vault_id)
        .fetch_one(pool)
        .await
        .expect("vault key wrap query succeeds");
        assert_eq!(vault_key_wraps, 1);

        let device: (String, String, Value) = sqlx::query_as(
            "
            SELECT display_name, client_type, public_metadata
            FROM devices
            WHERE account_id = $1
            ",
        )
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("device row exists");
        assert_eq!(device.0, "Firefox on laptop");
        assert_eq!(device.1, "browser");
        assert_eq!(device.2["platform_hint"], "web");

        let session: (String, i64, bool, bool) = sqlx::query_as(
            "
            SELECT
                session_state,
                octet_length(session_token_hash)::bigint,
                csrf_token_hash IS NULL,
                idle_expires_at <= absolute_expires_at
            FROM sessions
            WHERE account_id = $1
            ",
        )
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("session row exists");
        assert_eq!(session.0, "mfa_enrollment_required");
        assert_eq!(session.1, 32);
        assert!(session.2);
        assert!(session.3);

        let challenge_consumed: bool = sqlx::query_scalar(
            "
            SELECT consumed_at IS NOT NULL
            FROM auth_challenges
            WHERE login_handle_normalized = $1
              AND challenge_type = 'register'
            ",
        )
        .bind(login_handle)
        .fetch_one(pool)
        .await
        .expect("challenge row exists");
        assert!(challenge_consumed);

        let audit_events: i64 = sqlx::query_scalar(
            "
            SELECT COUNT(*)
            FROM audit_events
            WHERE account_id = $1
              AND event_type = 'account_registered'
            ",
        )
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("audit event query succeeds");
        assert_eq!(audit_events, 1);
    }

    async fn account_count(pool: &sqlx::PgPool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
            .fetch_one(pool)
            .await
            .expect("account count query succeeds")
    }

    async fn assert_auth_start_rate_limit(router: &axum::Router) {
        let client_nonce = encode_base64url(&[0x22; 32]);
        let mut last_status = http::StatusCode::OK;
        let mut last_cache_control = None;
        let mut last_body = Value::Null;

        for _ in 0..21 {
            let response = router
                .clone()
                .oneshot(json_request(
                    "/v1/auth/login/start",
                    &format!(
                        r#"{{
                        "login_handle":"limited@example.com",
                        "auth_protocol":"derived-auth-v1",
                        "client_nonce":"{client_nonce}"
                    }}"#
                    ),
                ))
                .await
                .expect("rate limit request returns a response");
            last_status = response.status();
            last_cache_control = response.headers().get("cache-control").cloned();
            last_body = response_json(response).await;
        }

        assert_eq!(last_status, 429);
        assert_eq!(last_cache_control.unwrap(), "no-store");
        assert_eq!(last_body["error"]["code"], "rate_limited");
    }

    async fn reset_auth_route_test_data(pool: &sqlx::PgPool) {
        sqlx::query(
            "
            TRUNCATE
                auth_challenges,
                sessions,
                recovery_codes,
                totp_factors,
                vault_key_wraps,
                account_keysets,
                vault_item_revisions,
                vault_items,
                vaults,
                devices,
                accounts
            RESTART IDENTITY CASCADE
            ",
        )
        .execute(pool)
        .await
        .expect("test data reset succeeds");
    }

    async fn insert_test_account(pool: &sqlx::PgPool, id: &str, login_handle: &str) {
        sqlx::query(
            "
            INSERT INTO accounts (
                id,
                login_handle_normalized,
                auth_protocol,
                kdf_profile,
                account_salt,
                auth_verifier_profile,
                auth_verifier_salt,
                auth_verifier_iterations,
                auth_stored_key,
                auth_server_key
            ) VALUES (
                $1::uuid,
                $2,
                'derived-auth-v1',
                $3::jsonb,
                $4,
                'pv-scram-sha-256-v1',
                $5,
                150000,
                $6,
                $7
            )
            ",
        )
        .bind(id)
        .bind(login_handle)
        .bind(r#"{"id":"argon2id-browser-v1","algorithm":"argon2id","memory_kib":19456,"iterations":2,"parallelism":1}"#)
        .bind(vec![0xaau8; 32])
        .bind(vec![0xbbu8; 32])
        .bind(vec![0xccu8; 32])
        .bind(vec![0xddu8; 32])
        .execute(pool)
        .await
        .expect("test account inserts");
    }
}
