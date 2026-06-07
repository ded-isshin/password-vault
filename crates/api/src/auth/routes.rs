use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, FromRequest, Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;
use sqlx::{PgPool, Row, types::Json as SqlJson};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{
        encoding::{decode_base64url_array, encode_base64url},
        scram::{DEFAULT_ITERATIONS, DEFAULT_SALT_BYTES, PROFILE_ID},
    },
};

const AUTH_PROTOCOL: &str = "derived-auth-v1";
const AUTH_BODY_LIMIT_BYTES: usize = 16 * 1024;
const CLIENT_NONCE_BYTES: usize = 32;
const SERVER_NONCE_BYTES: usize = 32;
const MAX_LOGIN_HANDLE_BYTES: usize = 320;
const AUTH_CHALLENGE_RATE_LIMIT: i64 = 20;
const AUTH_CHALLENGE_RATE_LIMIT_WINDOW: Duration = Duration::minutes(5);
const REGISTER_CHALLENGE_TTL: Duration = Duration::minutes(10);
const LOGIN_CHALLENGE_TTL: Duration = Duration::minutes(5);
type HmacSha256 = Hmac<Sha256>;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/auth/register/start", post(register_start))
        .route("/v1/auth/login/start", post(login_start))
        .layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT_BYTES))
        .layer(middleware::from_fn(add_no_store_header))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterStartRequest {
    login_handle: String,
    auth_protocol: String,
}

#[derive(Serialize)]
struct RegisterStartResponse {
    registration_id: Uuid,
    auth_protocol: &'static str,
    kdf_profile: Value,
    account_salt: String,
    auth_verifier_profile: &'static str,
    auth_verifier_salt: String,
    auth_verifier_iterations: u32,
    expires_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginStartRequest {
    login_handle: String,
    auth_protocol: String,
    client_nonce: String,
}

#[derive(Serialize)]
struct LoginStartResponse {
    login_challenge_id: Uuid,
    auth_protocol: &'static str,
    kdf_profile: Value,
    account_salt: String,
    auth_verifier_profile: &'static str,
    auth_verifier_salt: String,
    auth_verifier_iterations: u32,
    server_nonce: String,
    combined_nonce: String,
    expires_at: String,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorObject,
}

#[derive(Serialize)]
struct ErrorObject {
    code: &'static str,
    message: &'static str,
}

#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}

impl ApiError {
    fn bad_request() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: "Bad request.",
        }
    }

    fn service_unavailable() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "service_unavailable",
            message: "Service is temporarily unavailable.",
        }
    }

    fn rate_limited() -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "rate_limited",
            message: "Too many requests.",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        no_store_json(
            self.status,
            ErrorEnvelope {
                error: ErrorObject {
                    code: self.code,
                    message: self.message,
                },
            },
        )
    }
}

async fn register_start(
    State(state): State<AppState>,
    StrictJson(request): StrictJson<RegisterStartRequest>,
) -> Result<Response, ApiError> {
    ensure_supported_protocol(&request.auth_protocol)?;
    let pool = database_pool(&state)?;
    let login_handle_normalized = normalize_login_handle(&request.login_handle)?;
    cleanup_expired_challenges(pool).await?;
    enforce_challenge_rate_limit(pool, &login_handle_normalized, "register").await?;

    let registration_id = Uuid::new_v4();
    let account_salt = random_bytes::<DEFAULT_SALT_BYTES>();
    let auth_verifier_salt = random_bytes::<DEFAULT_SALT_BYTES>();
    let server_nonce = random_bytes::<SERVER_NONCE_BYTES>();
    let expires_at = now_utc_second()? + REGISTER_CHALLENGE_TTL;
    let metadata = RegisterChallengeMetadata {
        kdf_profile: default_kdf_profile(),
        account_salt: encode_base64url(&account_salt),
        auth_verifier_profile: PROFILE_ID,
        auth_verifier_salt: encode_base64url(&auth_verifier_salt),
        auth_verifier_iterations: DEFAULT_ITERATIONS,
    };

    insert_auth_challenge(InsertChallenge {
        pool,
        id: registration_id,
        account_id: None,
        login_handle_normalized: &login_handle_normalized,
        challenge_type: "register",
        server_nonce: &server_nonce,
        public_metadata: serde_json::to_value(&metadata)
            .map_err(|_| ApiError::service_unavailable())?,
        expires_at,
    })
    .await?;

    Ok(no_store_json(
        StatusCode::OK,
        RegisterStartResponse {
            registration_id,
            auth_protocol: AUTH_PROTOCOL,
            kdf_profile: metadata.kdf_profile,
            account_salt: metadata.account_salt,
            auth_verifier_profile: PROFILE_ID,
            auth_verifier_salt: metadata.auth_verifier_salt,
            auth_verifier_iterations: DEFAULT_ITERATIONS,
            expires_at: format_rfc3339(expires_at)?,
        },
    ))
}

async fn login_start(
    State(state): State<AppState>,
    StrictJson(request): StrictJson<LoginStartRequest>,
) -> Result<Response, ApiError> {
    ensure_supported_protocol(&request.auth_protocol)?;
    let pool = database_pool(&state)?;
    let synthetic_key = state
        .config
        .synthetic_metadata_key()
        .ok_or_else(ApiError::service_unavailable)?;
    let login_handle_normalized = normalize_login_handle(&request.login_handle)?;
    let client_nonce = decode_base64url_array::<CLIENT_NONCE_BYTES>(&request.client_nonce)
        .map_err(|_| ApiError::bad_request())?;
    cleanup_expired_challenges(pool).await?;
    enforce_challenge_rate_limit(pool, &login_handle_normalized, "login").await?;

    let metadata = load_login_metadata(pool, &login_handle_normalized)
        .await?
        .unwrap_or_else(|| synthetic_login_metadata(synthetic_key, &login_handle_normalized));
    let login_challenge_id = Uuid::new_v4();
    let server_nonce = random_bytes::<SERVER_NONCE_BYTES>();
    let mut combined_nonce = Vec::with_capacity(CLIENT_NONCE_BYTES + SERVER_NONCE_BYTES);
    combined_nonce.extend_from_slice(&client_nonce);
    combined_nonce.extend_from_slice(&server_nonce);
    let expires_at = now_utc_second()? + LOGIN_CHALLENGE_TTL;
    let challenge_metadata = LoginChallengeMetadata {
        client_nonce: encode_base64url(&client_nonce),
        server_nonce: encode_base64url(&server_nonce),
        combined_nonce: encode_base64url(&combined_nonce),
        auth_verifier_profile: PROFILE_ID,
        auth_verifier_iterations: metadata.auth_verifier_iterations,
        synthetic: metadata.account_id.is_none(),
    };

    insert_auth_challenge(InsertChallenge {
        pool,
        id: login_challenge_id,
        account_id: metadata.account_id,
        login_handle_normalized: &login_handle_normalized,
        challenge_type: "login",
        server_nonce: &server_nonce,
        public_metadata: serde_json::to_value(&challenge_metadata)
            .map_err(|_| ApiError::service_unavailable())?,
        expires_at,
    })
    .await?;

    Ok(no_store_json(
        StatusCode::OK,
        LoginStartResponse {
            login_challenge_id,
            auth_protocol: AUTH_PROTOCOL,
            kdf_profile: metadata.kdf_profile,
            account_salt: encode_base64url(&metadata.account_salt),
            auth_verifier_profile: PROFILE_ID,
            auth_verifier_salt: encode_base64url(&metadata.auth_verifier_salt),
            auth_verifier_iterations: metadata.auth_verifier_iterations,
            server_nonce: challenge_metadata.server_nonce,
            combined_nonce: challenge_metadata.combined_nonce,
            expires_at: format_rfc3339(expires_at)?,
        },
    ))
}

struct LoginMetadata {
    account_id: Option<Uuid>,
    kdf_profile: Value,
    account_salt: Vec<u8>,
    auth_verifier_salt: Vec<u8>,
    auth_verifier_iterations: u32,
}

#[derive(Serialize)]
struct RegisterChallengeMetadata {
    kdf_profile: Value,
    account_salt: String,
    auth_verifier_profile: &'static str,
    auth_verifier_salt: String,
    auth_verifier_iterations: u32,
}

#[derive(Serialize)]
struct LoginChallengeMetadata {
    client_nonce: String,
    server_nonce: String,
    combined_nonce: String,
    auth_verifier_profile: &'static str,
    auth_verifier_iterations: u32,
    synthetic: bool,
}

struct InsertChallenge<'a> {
    pool: &'a PgPool,
    id: Uuid,
    account_id: Option<Uuid>,
    login_handle_normalized: &'a str,
    challenge_type: &'static str,
    server_nonce: &'a [u8],
    public_metadata: Value,
    expires_at: OffsetDateTime,
}

async fn insert_auth_challenge(input: InsertChallenge<'_>) -> Result<(), ApiError> {
    sqlx::query(
        "
        INSERT INTO auth_challenges (
            id,
            account_id,
            login_handle_normalized,
            challenge_type,
            auth_protocol,
            server_nonce,
            public_metadata,
            expires_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ",
    )
    .bind(input.id)
    .bind(input.account_id)
    .bind(input.login_handle_normalized)
    .bind(input.challenge_type)
    .bind(AUTH_PROTOCOL)
    .bind(input.server_nonce)
    .bind(SqlJson(input.public_metadata))
    .bind(input.expires_at)
    .execute(input.pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    Ok(())
}

async fn cleanup_expired_challenges(pool: &PgPool) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM auth_challenges WHERE expires_at < now()")
        .execute(pool)
        .await
        .map_err(|_| ApiError::service_unavailable())?;
    Ok(())
}

async fn enforce_challenge_rate_limit(
    pool: &PgPool,
    login_handle_normalized: &str,
    challenge_type: &str,
) -> Result<(), ApiError> {
    let window_start = now_utc_second()? - AUTH_CHALLENGE_RATE_LIMIT_WINDOW;
    let count = sqlx::query_scalar::<_, i64>(
        "
        SELECT COUNT(*)
        FROM auth_challenges
        WHERE login_handle_normalized = $1
          AND challenge_type = $2
          AND created_at >= $3
        ",
    )
    .bind(login_handle_normalized)
    .bind(challenge_type)
    .bind(window_start)
    .fetch_one(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    if count >= AUTH_CHALLENGE_RATE_LIMIT {
        Err(ApiError::rate_limited())
    } else {
        Ok(())
    }
}

async fn load_login_metadata(
    pool: &PgPool,
    login_handle_normalized: &str,
) -> Result<Option<LoginMetadata>, ApiError> {
    let row = sqlx::query(
        "
        SELECT
            id,
            kdf_profile,
            account_salt,
            auth_verifier_salt,
            auth_verifier_iterations
        FROM accounts
        WHERE login_handle_normalized = $1
        ",
    )
    .bind(login_handle_normalized)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let Some(row) = row else {
        return Ok(None);
    };

    let account_id = row
        .try_get::<Uuid, _>("id")
        .map_err(|_| ApiError::service_unavailable())?;
    let kdf_profile = row
        .try_get::<SqlJson<Value>, _>("kdf_profile")
        .map_err(|_| ApiError::service_unavailable())?
        .0;
    let account_salt = row
        .try_get::<Vec<u8>, _>("account_salt")
        .map_err(|_| ApiError::service_unavailable())?;
    let auth_verifier_salt = row
        .try_get::<Vec<u8>, _>("auth_verifier_salt")
        .map_err(|_| ApiError::service_unavailable())?;
    let auth_verifier_iterations = row
        .try_get::<i32, _>("auth_verifier_iterations")
        .map_err(|_| ApiError::service_unavailable())?
        .try_into()
        .map_err(|_| ApiError::service_unavailable())?;
    if account_salt.len() != DEFAULT_SALT_BYTES
        || auth_verifier_salt.len() != DEFAULT_SALT_BYTES
        || auth_verifier_iterations != DEFAULT_ITERATIONS
        || kdf_profile != default_kdf_profile()
    {
        return Err(ApiError::service_unavailable());
    }

    Ok(Some(LoginMetadata {
        account_id: Some(account_id),
        kdf_profile,
        account_salt,
        auth_verifier_salt,
        auth_verifier_iterations,
    }))
}

fn synthetic_login_metadata(
    synthetic_metadata_key: &[u8; 32],
    login_handle_normalized: &str,
) -> LoginMetadata {
    LoginMetadata {
        account_id: None,
        kdf_profile: default_kdf_profile(),
        account_salt: synthetic_bytes(
            synthetic_metadata_key,
            "password-vault/synthetic-account-salt/v1",
            login_handle_normalized,
        )
        .to_vec(),
        auth_verifier_salt: synthetic_bytes(
            synthetic_metadata_key,
            "password-vault/synthetic-auth-verifier-salt/v1",
            login_handle_normalized,
        )
        .to_vec(),
        auth_verifier_iterations: DEFAULT_ITERATIONS,
    }
}

fn synthetic_bytes(
    synthetic_metadata_key: &[u8; 32],
    domain: &str,
    login_handle_normalized: &str,
) -> [u8; 32] {
    let mut mac =
        HmacSha256::new_from_slice(synthetic_metadata_key).expect("HMAC accepts any key length");
    mac.update(domain.as_bytes());
    mac.update(&[0]);
    mac.update(login_handle_normalized.as_bytes());
    mac.finalize().into_bytes().into()
}

fn default_kdf_profile() -> Value {
    json!({
        "id": "argon2id-browser-v1",
        "algorithm": "argon2id",
        "memory_kib": 19456,
        "iterations": 2,
        "parallelism": 1,
    })
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut output = [0u8; N];
    OsRng.fill_bytes(&mut output);
    output
}

fn ensure_supported_protocol(auth_protocol: &str) -> Result<(), ApiError> {
    if auth_protocol == AUTH_PROTOCOL {
        Ok(())
    } else {
        Err(ApiError::bad_request())
    }
}

fn normalize_login_handle(login_handle: &str) -> Result<String, ApiError> {
    let normalized = login_handle.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > MAX_LOGIN_HANDLE_BYTES
        || normalized.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request());
    }
    Ok(normalized)
}

fn database_pool(state: &AppState) -> Result<&PgPool, ApiError> {
    state
        .database
        .as_ref()
        .ok_or_else(ApiError::service_unavailable)
}

fn no_store_json<T: Serialize>(status: StatusCode, body: T) -> Response {
    let mut response = (status, Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn format_rfc3339(value: OffsetDateTime) -> Result<String, ApiError> {
    value
        .format(&Rfc3339)
        .map_err(|_| ApiError::service_unavailable())
}

fn now_utc_second() -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .map_err(|_| ApiError::service_unavailable())
}

struct StrictJson<T>(T);

impl<S, T> FromRequest<S> for StrictJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state)
            .await
            .map_err(|_| ApiError::bad_request())?;
        Ok(Self(value))
    }
}

async fn add_no_store_header(request: axum::http::Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::{normalize_login_handle, synthetic_login_metadata};

    #[test]
    fn login_handle_normalization_is_lowercase_trimmed() {
        assert_eq!(
            normalize_login_handle(" User@Example.COM ").expect("valid handle normalizes"),
            "user@example.com"
        );
    }

    #[test]
    fn synthetic_metadata_is_deterministic_per_handle() {
        let key = [7u8; 32];
        let first = synthetic_login_metadata(&key, "missing@example.com");
        let second = synthetic_login_metadata(&key, "missing@example.com");
        let other = synthetic_login_metadata(&key, "other@example.com");

        assert_eq!(first.account_salt, second.account_salt);
        assert_eq!(first.auth_verifier_salt, second.auth_verifier_salt);
        assert_ne!(first.account_salt, other.account_salt);
        assert_ne!(first.auth_verifier_salt, other.auth_verifier_salt);
    }

    #[test]
    fn synthetic_metadata_depends_on_secret_key() {
        let first = synthetic_login_metadata(&[7u8; 32], "missing@example.com");
        let second = synthetic_login_metadata(&[8u8; 32], "missing@example.com");

        assert_ne!(first.account_salt, second.account_salt);
        assert_ne!(first.auth_verifier_salt, second.auth_verifier_salt);
    }
}
