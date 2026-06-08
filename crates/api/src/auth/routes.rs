use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, FromRequest, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit as AeadKeyInit, Payload},
};
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction, types::Json as SqlJson};
use subtle::ConstantTimeEq;
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{
        encoding::{decode_base64url, decode_base64url_array, encode_base64url},
        scram::{self, DEFAULT_ITERATIONS, DEFAULT_SALT_BYTES, PROFILE_ID},
        tokens,
        totp::{self, TotpAlgorithm, TotpProfile},
        transcript::{self, LoginAuthMessage},
    },
    telemetry,
};

const AUTH_PROTOCOL: &str = "derived-auth-v1";
pub(crate) const AUTH_BODY_LIMIT_BYTES: usize = 128 * 1024;
const CLIENT_NONCE_BYTES: usize = 32;
const SERVER_NONCE_BYTES: usize = 32;
const AUTH_KEY_BYTES: usize = 32;
const AEAD_NONCE_BYTES: usize = 12;
const PBKDF2_BROWSER_PROFILE_ID: &str = "pbkdf2-sha256-browser-v1";
const PBKDF2_BROWSER_ITERATIONS: u32 = 600_000;
const MAX_ENCRYPTED_ENVELOPE_BYTES: usize = 64 * 1024;
const MAX_LOGIN_HANDLE_BYTES: usize = 320;
const MAX_DEVICE_LABEL_BYTES: usize = 128;
const MAX_DEVICE_PUBLIC_METADATA_BYTES: usize = 2048;
const AUTH_CHALLENGE_RATE_LIMIT: i64 = 20;
const MFA_CHALLENGE_MAX_ATTEMPTS: i32 = 5;
const AUTH_CHALLENGE_RATE_LIMIT_WINDOW: Duration = Duration::minutes(5);
const REGISTER_CHALLENGE_TTL: Duration = Duration::minutes(10);
const LOGIN_CHALLENGE_TTL: Duration = Duration::minutes(5);
const SESSION_IDLE_TTL: Duration = Duration::minutes(30);
const SESSION_ABSOLUTE_TTL: Duration = Duration::hours(12);
const ACCOUNT_KEYSET_CRYPTO_VERSION: &str = "account-keyset-v1";
const VAULT_KEY_WRAP_CRYPTO_VERSION: &str = "vault-key-wrap-v1";
const VAULT_CRYPTO_PROFILE_ID: &str = "vault-crypto-v1";
const SESSION_COOKIE_NAME: &str = "__Host-pv_session";
const TOTP_ISSUER: &str = "Password Vault";
const TOTP_SEED_BYTES: usize = 20;
const TOTP_SEED_AEAD: &str = "xchacha20poly1305-v1";
const TOTP_SEED_KEY_ID: &str = "app-totp-seed-key-v1";
const XCHACHA20POLY1305_NONCE_BYTES: usize = 24;
const RECOVERY_CODE_COUNT: usize = 10;
const RECOVERY_CODE_RANDOM_BYTES: usize = 16;
const RECOVERY_CODE_SALT_BYTES: usize = 16;
const MAX_RECOVERY_CODE_BYTES: usize = 128;
type HmacSha256 = Hmac<Sha256>;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/auth/register/start", post(register_start))
        .route("/v1/auth/register/finish", post(register_finish))
        .route("/v1/auth/login/start", post(login_start))
        .route("/v1/auth/login/finish", post(login_finish))
        .route("/v1/auth/mfa/totp/verify", post(totp_verify))
        .route(
            "/v1/auth/mfa/recovery-code/verify",
            post(recovery_code_verify),
        )
        .route("/v1/auth/logout", post(logout))
        .route("/v1/mfa/totp/enroll/start", post(totp_enroll_start))
        .route("/v1/mfa/totp/enroll/confirm", post(totp_enroll_confirm))
        .route("/v1/csrf", get(csrf_token))
        .route("/v1/session", get(session_status))
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
pub struct RegisterFinishRequest {
    registration_id: Uuid,
    auth_protocol: String,
    auth_stored_key: String,
    auth_server_key: String,
    encrypted_account_keyset: EncryptedEnvelopeRequest,
    initial_vault: InitialVaultRequest,
    device: DeviceRequest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncryptedEnvelopeRequest {
    crypto_version: String,
    key_id: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialVaultRequest {
    vault_id: Uuid,
    encrypted_vault_key: EncryptedEnvelopeRequest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceRequest {
    label: String,
    client_type: String,
    public_metadata: Value,
}

#[derive(Serialize)]
struct RegisterFinishResponse {
    account_id: Uuid,
    session: SessionResponse,
    next_step: &'static str,
}

#[derive(Serialize)]
struct SessionResponse {
    state: &'static str,
    vault_access: bool,
    idle_expires_at: String,
    absolute_expires_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogoutRequest {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TotpEnrollStartRequest {}

#[derive(Serialize)]
struct TotpEnrollStartResponse {
    factor_id: Uuid,
    status: &'static str,
    totp_profile: TotpProfileResponse,
    otpauth_uri: String,
    manual_secret: String,
    expires_at: String,
}

#[derive(Serialize)]
struct TotpProfileResponse {
    algorithm: &'static str,
    digits: u32,
    period: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TotpEnrollConfirmRequest {
    factor_id: Uuid,
    code: String,
}

#[derive(Serialize)]
struct TotpEnrollConfirmResponse {
    factor_id: Uuid,
    status: &'static str,
    session: SessionResponse,
    recovery_codes: Vec<String>,
}

#[derive(Serialize)]
struct CsrfTokenResponse {
    csrf_token: String,
    expires_at: String,
}

#[derive(Serialize)]
struct AuthenticatedSessionResponse {
    authenticated: bool,
    account_id: Uuid,
    device_id: Option<Uuid>,
    session_state: String,
    vault_access: bool,
    idle_expires_at: String,
    absolute_expires_at: String,
}

#[derive(Serialize)]
struct UnauthenticatedSessionResponse {
    authenticated: bool,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginFinishRequest {
    login_challenge_id: Uuid,
    auth_protocol: String,
    client_nonce: String,
    server_nonce: String,
    client_final_without_proof: String,
    client_proof: String,
    device: DeviceRequest,
}

#[derive(Serialize)]
struct LoginFinishMfaRequiredResponse {
    result: &'static str,
    mfa_challenge_id: Uuid,
    available_methods: Vec<&'static str>,
    expires_at: String,
}

#[derive(Serialize)]
struct LoginFinishSessionCreatedResponse {
    result: &'static str,
    session: SessionResponse,
    next_step: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TotpVerifyRequest {
    mfa_challenge_id: Uuid,
    code: String,
}

#[derive(Serialize)]
struct TotpVerifyResponse {
    result: &'static str,
    session: SessionResponse,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryCodeVerifyRequest {
    mfa_challenge_id: Uuid,
    recovery_code: String,
}

#[derive(Serialize)]
struct RecoveryCodeVerifyResponse {
    result: &'static str,
    session: SessionResponse,
    next_step: &'static str,
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
    pub(crate) fn bad_request() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: "Bad request.",
        }
    }

    pub(crate) fn service_unavailable() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "service_unavailable",
            message: "Service is temporarily unavailable.",
        }
    }

    pub(crate) fn rate_limited() -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "rate_limited",
            message: "Too many requests.",
        }
    }

    fn registration_unavailable() -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "registration_unavailable",
            message: "Registration is unavailable.",
        }
    }

    pub(crate) fn session_required() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "session_required",
            message: "A valid session is required.",
        }
    }

    pub(crate) fn csrf_required() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "csrf_required",
            message: "A valid CSRF token is required.",
        }
    }

    pub(crate) fn mfa_required() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "mfa_required",
            message: "MFA enrollment or verification is required.",
        }
    }

    pub(crate) fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "Not found.",
        }
    }

    fn mfa_verification_failed() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "mfa_verification_failed",
            message: "MFA verification failed.",
        }
    }

    fn auth_failed() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "auth_failed",
            message: "Authentication failed.",
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
        auth_verifier_profile: PROFILE_ID.to_string(),
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

    telemetry::registration_event("start", "issued");

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

async fn register_finish(
    State(state): State<AppState>,
    StrictJson(request): StrictJson<RegisterFinishRequest>,
) -> Result<Response, ApiError> {
    ensure_supported_protocol(&request.auth_protocol)?;
    let pool = database_pool(&state)?;
    let auth_stored_key = decode_base64url_array::<AUTH_KEY_BYTES>(&request.auth_stored_key)
        .map_err(|_| ApiError::bad_request())?;
    let auth_server_key = decode_base64url_array::<AUTH_KEY_BYTES>(&request.auth_server_key)
        .map_err(|_| ApiError::bad_request())?;
    if auth_stored_key == auth_server_key {
        return Err(ApiError::bad_request());
    }
    let encrypted_account_keyset = ValidatedEncryptedEnvelope::from_request(
        &request.encrypted_account_keyset,
        ACCOUNT_KEYSET_CRYPTO_VERSION,
    )?;
    let encrypted_vault_key = ValidatedEncryptedEnvelope::from_request(
        &request.initial_vault.encrypted_vault_key,
        VAULT_KEY_WRAP_CRYPTO_VERSION,
    )?;
    if encrypted_account_keyset.key_id != encrypted_vault_key.key_id {
        return Err(ApiError::bad_request());
    }
    let device = ValidatedDevice::from_request(&request.device)?;

    let now = now_utc_second()?;
    let idle_expires_at = now + SESSION_IDLE_TTL;
    let absolute_expires_at = now + SESSION_ABSOLUTE_TTL;
    let account_id = Uuid::new_v4();
    let account_keyset_id = Uuid::new_v4();
    let vault_key_wrap_id = Uuid::new_v4();
    let device_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let session_token = tokens::random_token();
    let session_token_hash = tokens::sha256_verifier(&session_token);
    let genesis_head_hash = genesis_head_hash(request.initial_vault.vault_id);

    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    let challenge = sqlx::query(
        "
        SELECT login_handle_normalized, public_metadata
        FROM auth_challenges
        WHERE id = $1
          AND challenge_type = 'register'
          AND auth_protocol = $2
          AND consumed_at IS NULL
          AND expires_at >= $3
        FOR UPDATE
        ",
    )
    .bind(request.registration_id)
    .bind(AUTH_PROTOCOL)
    .bind(now)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let Some(challenge) = challenge else {
        return Err(ApiError::registration_unavailable());
    };

    let login_handle_normalized = challenge
        .try_get::<String, _>("login_handle_normalized")
        .map_err(|_| ApiError::service_unavailable())?;
    let public_metadata = challenge
        .try_get::<SqlJson<Value>, _>("public_metadata")
        .map_err(|_| ApiError::service_unavailable())?
        .0;
    let challenge_metadata = decode_register_challenge_metadata(public_metadata)?;
    let account_salt =
        decode_base64url_array::<DEFAULT_SALT_BYTES>(&challenge_metadata.account_salt)
            .map_err(|_| ApiError::service_unavailable())?;
    let auth_verifier_salt =
        decode_base64url_array::<DEFAULT_SALT_BYTES>(&challenge_metadata.auth_verifier_salt)
            .map_err(|_| ApiError::service_unavailable())?;

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
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ",
    )
    .bind(account_id)
    .bind(&login_handle_normalized)
    .bind(AUTH_PROTOCOL)
    .bind(SqlJson(challenge_metadata.kdf_profile.clone()))
    .bind(account_salt.as_slice())
    .bind(PROFILE_ID)
    .bind(auth_verifier_salt.as_slice())
    .bind(challenge_metadata.auth_verifier_iterations as i32)
    .bind(auth_stored_key.as_slice())
    .bind(auth_server_key.as_slice())
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::registration_unavailable()
        } else {
            ApiError::service_unavailable()
        }
    })?;

    sqlx::query(
        "
        INSERT INTO account_keysets (
            id,
            account_id,
            crypto_version,
            key_id,
            nonce,
            ciphertext
        ) VALUES ($1, $2, $3, $4, $5, $6)
        ",
    )
    .bind(account_keyset_id)
    .bind(account_id)
    .bind(&encrypted_account_keyset.crypto_version)
    .bind(&encrypted_account_keyset.key_id)
    .bind(encrypted_account_keyset.nonce.as_slice())
    .bind(encrypted_account_keyset.ciphertext.as_slice())
    .execute(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    sqlx::query(
        "
        INSERT INTO devices (
            id,
            account_id,
            display_name,
            user_agent_hash,
            client_type,
            public_metadata
        ) VALUES ($1, $2, $3, $4, $5, $6)
        ",
    )
    .bind(device_id)
    .bind(account_id)
    .bind(&device.label)
    .bind(Option::<Vec<u8>>::None)
    .bind(&device.client_type)
    .bind(SqlJson(device.public_metadata))
    .execute(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    sqlx::query(
        "
        INSERT INTO vaults (
            id,
            account_id,
            crypto_profile_id,
            genesis_head_hash,
            head_hash
        ) VALUES ($1, $2, $3, $4, $5)
        ",
    )
    .bind(request.initial_vault.vault_id)
    .bind(account_id)
    .bind(VAULT_CRYPTO_PROFILE_ID)
    .bind(genesis_head_hash.as_slice())
    .bind(genesis_head_hash.as_slice())
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::registration_unavailable()
        } else {
            ApiError::service_unavailable()
        }
    })?;

    sqlx::query(
        "
        INSERT INTO vault_key_wraps (
            id,
            vault_id,
            account_id,
            key_id,
            crypto_version,
            nonce,
            ciphertext
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        ",
    )
    .bind(vault_key_wrap_id)
    .bind(request.initial_vault.vault_id)
    .bind(account_id)
    .bind(&encrypted_vault_key.key_id)
    .bind(&encrypted_vault_key.crypto_version)
    .bind(encrypted_vault_key.nonce.as_slice())
    .bind(encrypted_vault_key.ciphertext.as_slice())
    .execute(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    sqlx::query(
        "
        INSERT INTO sessions (
            id,
            account_id,
            device_id,
            session_token_hash,
            csrf_token_hash,
            session_state,
            expires_at,
            idle_expires_at,
            absolute_expires_at
        ) VALUES ($1, $2, $3, $4, $5, 'mfa_enrollment_required', $6, $6, $7)
        ",
    )
    .bind(session_id)
    .bind(account_id)
    .bind(device_id)
    .bind(session_token_hash.as_slice())
    .bind(Option::<&[u8]>::None)
    .bind(idle_expires_at)
    .bind(absolute_expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    sqlx::query(
        "
        INSERT INTO audit_events (
            account_id,
            actor_device_id,
            event_type,
            event_metadata
        ) VALUES ($1, $2, 'account_registered', $3)
        ",
    )
    .bind(account_id)
    .bind(device_id)
    .bind(SqlJson(json!({
        "auth_protocol": AUTH_PROTOCOL,
        "session_state": "mfa_enrollment_required"
    })))
    .execute(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    sqlx::query("UPDATE auth_challenges SET consumed_at = $1 WHERE id = $2")
        .bind(now)
        .bind(request.registration_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    transaction
        .commit()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    telemetry::registration_event("finish", "success");
    telemetry::account_created("success");
    telemetry::session_event("created", "mfa_enrollment_required");

    let mut response = no_store_json(
        StatusCode::CREATED,
        RegisterFinishResponse {
            account_id,
            session: SessionResponse {
                state: "mfa_enrollment_required",
                vault_access: false,
                idle_expires_at: format_rfc3339(idle_expires_at)?,
                absolute_expires_at: format_rfc3339(absolute_expires_at)?,
            },
            next_step: "enroll_totp",
        },
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie(&session_token))
            .map_err(|_| ApiError::service_unavailable())?,
    );

    Ok(response)
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
        auth_verifier_profile: PROFILE_ID.to_string(),
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

    telemetry::login_start("issued");

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

async fn login_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    StrictJson(request): StrictJson<LoginFinishRequest>,
) -> Result<Response, ApiError> {
    ensure_unsafe_request_context(&headers)?;
    let pool = database_pool(&state)?;
    if request.auth_protocol != AUTH_PROTOCOL {
        telemetry::login_attempt("failed", "unsupported_protocol");
        return Err(ApiError::auth_failed());
    }
    let client_nonce = decode_base64url_array::<CLIENT_NONCE_BYTES>(&request.client_nonce)
        .map_err(|_| ApiError::bad_request())?;
    let server_nonce = decode_base64url_array::<SERVER_NONCE_BYTES>(&request.server_nonce)
        .map_err(|_| ApiError::bad_request())?;
    let client_final_without_proof = decode_base64url(&request.client_final_without_proof)
        .map_err(|_| ApiError::bad_request())?;
    if client_final_without_proof.is_empty()
        || client_final_without_proof.len() > MAX_DEVICE_PUBLIC_METADATA_BYTES
    {
        return Err(ApiError::bad_request());
    }
    let client_proof = decode_base64url_array::<AUTH_KEY_BYTES>(&request.client_proof)
        .map_err(|_| ApiError::bad_request())?;
    let device = ValidatedDevice::from_request(&request.device)?;
    let now = now_utc_second()?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    let challenge = sqlx::query(
        "
        SELECT
            id,
            account_id,
            login_handle_normalized,
            server_nonce,
            public_metadata
        FROM auth_challenges
        WHERE id = $1
          AND challenge_type = 'login'
          AND auth_protocol = $2
          AND consumed_at IS NULL
          AND expires_at >= $3
        FOR UPDATE
        ",
    )
    .bind(request.login_challenge_id)
    .bind(AUTH_PROTOCOL)
    .bind(now)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let Some(challenge) = challenge else {
        telemetry::login_attempt("failed", "challenge_unavailable");
        return Err(ApiError::auth_failed());
    };

    let challenge_id = challenge
        .try_get::<Uuid, _>("id")
        .map_err(|_| ApiError::service_unavailable())?;
    let account_id = challenge
        .try_get::<Option<Uuid>, _>("account_id")
        .map_err(|_| ApiError::service_unavailable())?;
    let login_handle_normalized = challenge
        .try_get::<String, _>("login_handle_normalized")
        .map_err(|_| ApiError::service_unavailable())?;
    let stored_server_nonce = challenge
        .try_get::<Vec<u8>, _>("server_nonce")
        .map_err(|_| ApiError::service_unavailable())?;
    let public_metadata = challenge
        .try_get::<SqlJson<Value>, _>("public_metadata")
        .map_err(|_| ApiError::service_unavailable())?
        .0;
    let challenge_metadata = decode_login_challenge_metadata(public_metadata)?;

    let metadata_matches = challenge_metadata.client_nonce == encode_base64url(&client_nonce)
        && challenge_metadata.server_nonce == encode_base64url(&server_nonce)
        && stored_server_nonce == server_nonce
        && challenge_metadata.auth_verifier_profile == PROFILE_ID
        && challenge_metadata.auth_verifier_iterations == DEFAULT_ITERATIONS;
    if !metadata_matches {
        consume_auth_challenge(&mut transaction, challenge_id, now).await?;
        transaction
            .commit()
            .await
            .map_err(|_| ApiError::service_unavailable())?;
        telemetry::login_attempt("failed", "challenge_mismatch");
        return Err(ApiError::auth_failed());
    }

    let auth_message = transcript::login_auth_message(LoginAuthMessage {
        challenge_id,
        auth_protocol: AUTH_PROTOCOL,
        login_handle_normalized: &login_handle_normalized,
        client_nonce: &client_nonce,
        server_nonce: &server_nonce,
        client_final_without_proof: &client_final_without_proof,
    });

    let mut proof_ok = false;
    if let Some(account_id) = account_id.filter(|_| !challenge_metadata.synthetic) {
        let account = sqlx::query(
            "
            SELECT auth_stored_key
            FROM accounts
            WHERE id = $1
              AND login_handle_normalized = $2
              AND auth_protocol = $3
            ",
        )
        .bind(account_id)
        .bind(&login_handle_normalized)
        .bind(AUTH_PROTOCOL)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| ApiError::service_unavailable())?;

        if let Some(account) = account {
            let stored_key = auth_key_from_vec(
                account
                    .try_get::<Vec<u8>, _>("auth_stored_key")
                    .map_err(|_| ApiError::service_unavailable())?,
            )?;
            proof_ok = scram::verify_client_proof(&stored_key, &auth_message, &client_proof)
                .map_err(|_| ApiError::auth_failed())?;
        }

        if !proof_ok {
            insert_audit_event(
                &mut transaction,
                account_id,
                None,
                "auth_login_failed",
                json!({ "reason": "proof_failed" }),
            )
            .await?;
        }
    } else {
        let _ = scram::verify_client_proof(&[0u8; AUTH_KEY_BYTES], &auth_message, &client_proof);
    }

    if !proof_ok {
        consume_auth_challenge(&mut transaction, challenge_id, now).await?;
        transaction
            .commit()
            .await
            .map_err(|_| ApiError::service_unavailable())?;
        telemetry::login_attempt("failed", "proof_failed");
        return Err(ApiError::auth_failed());
    }

    let account_id = account_id.ok_or_else(ApiError::service_unavailable)?;
    consume_auth_challenge(&mut transaction, challenge_id, now).await?;

    let active_totp_factor = sqlx::query_scalar::<_, Uuid>(
        "
        SELECT id
        FROM totp_factors
        WHERE account_id = $1
          AND verified_at IS NOT NULL
        LIMIT 1
        ",
    )
    .bind(account_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    if active_totp_factor.is_some() {
        let mfa_challenge_id = Uuid::new_v4();
        let mfa_server_nonce = random_bytes::<SERVER_NONCE_BYTES>();
        let expires_at = now + LOGIN_CHALLENGE_TTL;
        insert_auth_challenge_tx(
            &mut transaction,
            InsertChallengeTx {
                id: mfa_challenge_id,
                account_id: Some(account_id),
                login_handle_normalized: &login_handle_normalized,
                challenge_type: "pre_mfa",
                server_nonce: &mfa_server_nonce,
                public_metadata: serde_json::to_value(PreMfaChallengeMetadata {
                    login_challenge_id: challenge_id,
                    device: DeviceChallengeMetadata::from_validated(&device),
                })
                .map_err(|_| ApiError::service_unavailable())?,
                expires_at,
            },
        )
        .await?;
        insert_audit_event(
            &mut transaction,
            account_id,
            None,
            "auth_login_password_verified",
            json!({ "mfa_required": true }),
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|_| ApiError::service_unavailable())?;

        telemetry::login_attempt("success", "none");
        telemetry::mfa_event("totp_login", "challenge_issued");

        return Ok(no_store_json(
            StatusCode::OK,
            LoginFinishMfaRequiredResponse {
                result: "mfa_required",
                mfa_challenge_id,
                available_methods: vec!["totp", "recovery_code"],
                expires_at: format_rfc3339(expires_at)?,
            },
        ));
    }

    let (_device_id, session_token, idle_expires_at, absolute_expires_at) =
        create_session_with_device(
            &mut transaction,
            account_id,
            &device,
            "mfa_enrollment_required",
            now,
        )
        .await?;
    insert_audit_event(
        &mut transaction,
        account_id,
        None,
        "auth_login_setup_session_created",
        json!({ "mfa_required": false }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    telemetry::login_attempt("success", "none");
    telemetry::session_event("created", "mfa_enrollment_required");

    let mut response = no_store_json(
        StatusCode::OK,
        LoginFinishSessionCreatedResponse {
            result: "session_created",
            session: SessionResponse {
                state: "mfa_enrollment_required",
                vault_access: false,
                idle_expires_at: format_rfc3339(idle_expires_at)?,
                absolute_expires_at: format_rfc3339(absolute_expires_at)?,
            },
            next_step: "enroll_totp",
        },
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie(&session_token))
            .map_err(|_| ApiError::service_unavailable())?,
    );
    Ok(response)
}

async fn totp_verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    StrictJson(request): StrictJson<TotpVerifyRequest>,
) -> Result<Response, ApiError> {
    ensure_unsafe_request_context(&headers)?;
    let pool = database_pool(&state)?;
    let totp_seed_key = state
        .config
        .totp_seed_key()
        .ok_or_else(ApiError::service_unavailable)?;
    let now = now_utc_second()?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    let challenge = sqlx::query(
        "
        SELECT
            id,
            account_id,
            public_metadata,
            attempts
        FROM auth_challenges
        WHERE id = $1
          AND challenge_type = 'pre_mfa'
          AND auth_protocol = $2
          AND consumed_at IS NULL
          AND expires_at >= $3
        FOR UPDATE
        ",
    )
    .bind(request.mfa_challenge_id)
    .bind(AUTH_PROTOCOL)
    .bind(now)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let Some(challenge) = challenge else {
        telemetry::mfa_event("totp_login", "challenge_unavailable");
        return Err(ApiError::mfa_verification_failed());
    };
    let challenge_id = challenge
        .try_get::<Uuid, _>("id")
        .map_err(|_| ApiError::service_unavailable())?;
    let account_id = challenge
        .try_get::<Option<Uuid>, _>("account_id")
        .map_err(|_| ApiError::service_unavailable())?
        .ok_or_else(ApiError::service_unavailable)?;
    let attempts = challenge
        .try_get::<i32, _>("attempts")
        .map_err(|_| ApiError::service_unavailable())?;
    if attempts >= MFA_CHALLENGE_MAX_ATTEMPTS {
        telemetry::mfa_event("totp_login", "attempts_exhausted");
        return Err(ApiError::mfa_verification_failed());
    }
    let public_metadata = challenge
        .try_get::<SqlJson<Value>, _>("public_metadata")
        .map_err(|_| ApiError::service_unavailable())?
        .0;
    let pre_mfa_metadata = decode_pre_mfa_challenge_metadata(public_metadata)?;
    let device = pre_mfa_metadata.device.into_validated()?;

    let factor = sqlx::query(
        "
        SELECT
            id,
            account_id,
            seed_ciphertext,
            seed_nonce,
            seed_key_id,
            seed_aead,
            algorithm,
            digits,
            period_seconds,
            last_accepted_step,
            verified_at
        FROM totp_factors
        WHERE account_id = $1
          AND verified_at IS NOT NULL
        FOR UPDATE
        ",
    )
    .bind(account_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let Some(factor) = factor else {
        telemetry::mfa_event("totp_login", "factor_unavailable");
        return Err(ApiError::mfa_verification_failed());
    };
    let factor_id = factor
        .try_get::<Uuid, _>("id")
        .map_err(|_| ApiError::service_unavailable())?;
    let profile = totp_profile_from_row(&factor)?;
    let seed_key_id = factor
        .try_get::<String, _>("seed_key_id")
        .map_err(|_| ApiError::service_unavailable())?;
    let seed_aead = factor
        .try_get::<String, _>("seed_aead")
        .map_err(|_| ApiError::service_unavailable())?;
    if seed_key_id != TOTP_SEED_KEY_ID || seed_aead != TOTP_SEED_AEAD {
        return Err(ApiError::service_unavailable());
    }
    let seed_ciphertext = factor
        .try_get::<Vec<u8>, _>("seed_ciphertext")
        .map_err(|_| ApiError::service_unavailable())?;
    let seed_nonce = factor
        .try_get::<Vec<u8>, _>("seed_nonce")
        .map_err(|_| ApiError::service_unavailable())?;
    let last_accepted_step = factor
        .try_get::<Option<i64>, _>("last_accepted_step")
        .map_err(|_| ApiError::service_unavailable())?
        .map(u64::try_from)
        .transpose()
        .map_err(|_| ApiError::service_unavailable())?;
    let aad = totp_seed_aad(account_id, factor_id, profile);
    let seed = decrypt_totp_seed(totp_seed_key, &seed_nonce, &aad, &seed_ciphertext)?;
    let accepted_step = totp::verify(
        &seed,
        unix_time_seconds(now)?,
        &request.code,
        profile,
        last_accepted_step,
    )
    .ok()
    .flatten();

    let Some(accepted_step) = accepted_step else {
        increment_challenge_attempts(&mut transaction, challenge_id, now).await?;
        insert_audit_event(
            &mut transaction,
            account_id,
            None,
            "mfa_totp_login_failed",
            json!({ "factor_id": factor_id }),
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|_| ApiError::service_unavailable())?;
        telemetry::mfa_event("totp_login", "failed");
        return Err(ApiError::mfa_verification_failed());
    };

    let (device_id, session_token, idle_expires_at, absolute_expires_at) =
        create_session_with_device(&mut transaction, account_id, &device, "mfa_verified", now)
            .await?;

    sqlx::query(
        "
        UPDATE totp_factors
        SET last_accepted_step = $1,
            updated_at = $2
        WHERE id = $3
        ",
    )
    .bind(i64::try_from(accepted_step).map_err(|_| ApiError::service_unavailable())?)
    .bind(now)
    .bind(factor_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    consume_auth_challenge(&mut transaction, challenge_id, now).await?;
    insert_audit_event(
        &mut transaction,
        account_id,
        Some(device_id),
        "mfa_totp_login_verified",
        json!({ "factor_id": factor_id }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    telemetry::mfa_event("totp_login", "verified");
    telemetry::session_event("created", "mfa_verified");

    let mut response = no_store_json(
        StatusCode::OK,
        TotpVerifyResponse {
            result: "session_created",
            session: SessionResponse {
                state: "mfa_verified",
                vault_access: true,
                idle_expires_at: format_rfc3339(idle_expires_at)?,
                absolute_expires_at: format_rfc3339(absolute_expires_at)?,
            },
        },
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie(&session_token))
            .map_err(|_| ApiError::service_unavailable())?,
    );
    Ok(response)
}

async fn recovery_code_verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    StrictJson(request): StrictJson<RecoveryCodeVerifyRequest>,
) -> Result<Response, ApiError> {
    ensure_unsafe_request_context(&headers)?;
    let pool = database_pool(&state)?;
    let now = now_utc_second()?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    let challenge = load_pre_mfa_challenge(&mut transaction, request.mfa_challenge_id, now).await?;
    let Some(challenge) = challenge else {
        telemetry::mfa_event("recovery_code_login", "challenge_unavailable");
        return Err(ApiError::mfa_verification_failed());
    };
    if challenge.attempts >= MFA_CHALLENGE_MAX_ATTEMPTS {
        telemetry::mfa_event("recovery_code_login", "attempts_exhausted");
        return Err(ApiError::mfa_verification_failed());
    }
    let device = challenge.metadata.device.into_validated()?;

    let recovery_code = validate_recovery_code(&request.recovery_code).ok();
    let recovery_code_id = if let Some(recovery_code) = recovery_code {
        find_matching_recovery_code(&mut transaction, challenge.account_id, &recovery_code).await?
    } else {
        None
    };
    let Some(recovery_code_id) = recovery_code_id else {
        increment_challenge_attempts(&mut transaction, challenge.id, now).await?;
        insert_audit_event(
            &mut transaction,
            challenge.account_id,
            None,
            "mfa_recovery_code_login_failed",
            json!({}),
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|_| ApiError::service_unavailable())?;
        telemetry::mfa_event("recovery_code_login", "failed");
        return Err(ApiError::mfa_verification_failed());
    };

    consume_recovery_code(
        &mut transaction,
        challenge.account_id,
        recovery_code_id,
        now,
    )
    .await?;

    let (device_id, session_token, idle_expires_at, absolute_expires_at) =
        create_session_with_device(
            &mut transaction,
            challenge.account_id,
            &device,
            "mfa_recovery",
            now,
        )
        .await?;

    consume_auth_challenge(&mut transaction, challenge.id, now).await?;
    insert_audit_event(
        &mut transaction,
        challenge.account_id,
        Some(device_id),
        "mfa_recovery_code_login_verified",
        json!({}),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    telemetry::mfa_event("recovery_code_login", "verified");
    telemetry::session_event("created", "mfa_recovery");

    let mut response = no_store_json(
        StatusCode::OK,
        RecoveryCodeVerifyResponse {
            result: "session_created",
            session: SessionResponse {
                state: "mfa_recovery",
                vault_access: false,
                idle_expires_at: format_rfc3339(idle_expires_at)?,
                absolute_expires_at: format_rfc3339(absolute_expires_at)?,
            },
            next_step: "reenroll_totp",
        },
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie(&session_token))
            .map_err(|_| ApiError::service_unavailable())?,
    );
    Ok(response)
}

async fn session_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let pool = database_pool(&state)?;
    let now = now_utc_second()?;
    let Some(session) = load_current_session(pool, &headers, now).await? else {
        let mut response = no_store_json(
            StatusCode::OK,
            UnauthenticatedSessionResponse {
                authenticated: false,
            },
        );
        if session_token_from_headers(&headers).is_some() {
            response.headers_mut().insert(
                header::SET_COOKIE,
                HeaderValue::from_static(clear_session_cookie()),
            );
        }
        return Ok(response);
    };

    let session = refresh_session_activity(pool, session, now).await?;
    Ok(no_store_json(
        StatusCode::OK,
        AuthenticatedSessionResponse {
            authenticated: true,
            account_id: session.account_id,
            device_id: session.device_id,
            session_state: session.session_state.clone(),
            vault_access: session.vault_access(),
            idle_expires_at: format_rfc3339(session.idle_expires_at)?,
            absolute_expires_at: format_rfc3339(session.absolute_expires_at)?,
        },
    ))
}

async fn csrf_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let pool = database_pool(&state)?;
    let now = now_utc_second()?;
    let session = load_current_session(pool, &headers, now)
        .await?
        .ok_or_else(ApiError::session_required)?;
    let session = refresh_session_activity(pool, session, now).await?;
    let csrf_token = tokens::random_token();
    let csrf_token_hash = tokens::sha256_verifier(&csrf_token);

    sqlx::query(
        "
        UPDATE sessions
        SET csrf_token_hash = $1
        WHERE id = $2
        ",
    )
    .bind(csrf_token_hash.as_slice())
    .bind(session.id)
    .execute(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    Ok(no_store_json(
        StatusCode::OK,
        CsrfTokenResponse {
            csrf_token: encode_base64url(&csrf_token),
            expires_at: format_rfc3339(session.idle_expires_at)?,
        },
    ))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    StrictJson(_request): StrictJson<LogoutRequest>,
) -> Result<Response, ApiError> {
    let pool = database_pool(&state)?;
    let now = now_utc_second()?;
    if let Some(session) = load_current_session(pool, &headers, now).await? {
        ensure_unsafe_request_context(&headers)?;
        ensure_csrf_token(pool, &headers, session.id).await?;
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(session.id)
            .execute(pool)
            .await
            .map_err(|_| ApiError::service_unavailable())?;
    }

    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static(clear_session_cookie()),
    );
    Ok(response)
}

async fn totp_enroll_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    StrictJson(_request): StrictJson<TotpEnrollStartRequest>,
) -> Result<Response, ApiError> {
    let pool = database_pool(&state)?;
    let totp_seed_key = state
        .config
        .totp_seed_key()
        .ok_or_else(ApiError::service_unavailable)?;
    let now = now_utc_second()?;
    let session = load_current_session(pool, &headers, now)
        .await?
        .ok_or_else(ApiError::session_required)?;
    ensure_totp_enrollment_session(&session)?;
    ensure_unsafe_request_context(&headers)?;
    ensure_csrf_token(pool, &headers, session.id).await?;
    let session = refresh_session_activity(pool, session, now).await?;

    let factor_id = Uuid::new_v4();
    let seed = random_bytes::<TOTP_SEED_BYTES>();
    let nonce = random_bytes::<XCHACHA20POLY1305_NONCE_BYTES>();
    let profile = TotpProfile::google_authenticator_default();
    let aad = totp_seed_aad(session.account_id, factor_id, profile);
    let seed_ciphertext = encrypt_totp_seed(totp_seed_key, &nonce, &aad, &seed)?;
    let login_handle = account_login_handle(pool, session.account_id).await?;
    let otpauth_uri = totp::provisioning_uri(TOTP_ISSUER, &login_handle, &seed, profile)
        .map_err(|_| ApiError::service_unavailable())?;
    let manual_secret = totp::base32_no_padding(&seed);

    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    sqlx::query("DELETE FROM totp_factors WHERE account_id = $1")
        .bind(session.account_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    sqlx::query(
        "
        INSERT INTO totp_factors (
            id,
            account_id,
            seed_ciphertext,
            seed_nonce,
            seed_key_id,
            seed_aead,
            algorithm,
            digits,
            period_seconds,
            last_accepted_step,
            verified_at
        ) VALUES ($1, $2, $3, $4, $5, $6, 'SHA1', 6, 30, NULL, NULL)
        ",
    )
    .bind(factor_id)
    .bind(session.account_id)
    .bind(seed_ciphertext.as_slice())
    .bind(nonce.as_slice())
    .bind(TOTP_SEED_KEY_ID)
    .bind(TOTP_SEED_AEAD)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    insert_audit_event(
        &mut transaction,
        session.account_id,
        session.device_id,
        "mfa_totp_enrollment_started",
        json!({ "factor_id": factor_id }),
    )
    .await?;

    transaction
        .commit()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    telemetry::mfa_event("totp_enrollment", "started");

    Ok(no_store_json(
        StatusCode::OK,
        TotpEnrollStartResponse {
            factor_id,
            status: "pending",
            totp_profile: totp_profile_response(profile),
            otpauth_uri,
            manual_secret,
            expires_at: format_rfc3339(session.idle_expires_at)?,
        },
    ))
}

async fn totp_enroll_confirm(
    State(state): State<AppState>,
    headers: HeaderMap,
    StrictJson(request): StrictJson<TotpEnrollConfirmRequest>,
) -> Result<Response, ApiError> {
    let pool = database_pool(&state)?;
    let totp_seed_key = state
        .config
        .totp_seed_key()
        .ok_or_else(ApiError::service_unavailable)?;
    let now = now_utc_second()?;
    let session = load_current_session(pool, &headers, now)
        .await?
        .ok_or_else(ApiError::session_required)?;
    ensure_totp_enrollment_session(&session)?;
    ensure_unsafe_request_context(&headers)?;
    ensure_csrf_token(pool, &headers, session.id).await?;

    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    let factor = sqlx::query(
        "
        SELECT
            id,
            account_id,
            seed_ciphertext,
            seed_nonce,
            seed_key_id,
            seed_aead,
            algorithm,
            digits,
            period_seconds,
            last_accepted_step,
            verified_at
        FROM totp_factors
        WHERE id = $1
          AND account_id = $2
        FOR UPDATE
        ",
    )
    .bind(request.factor_id)
    .bind(session.account_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let Some(factor) = factor else {
        telemetry::mfa_event("totp_enrollment", "factor_unavailable");
        return Err(ApiError::mfa_verification_failed());
    };

    let profile = totp_profile_from_row(&factor)?;
    let verified_at = factor
        .try_get::<Option<OffsetDateTime>, _>("verified_at")
        .map_err(|_| ApiError::service_unavailable())?;
    if verified_at.is_some() {
        telemetry::mfa_event("totp_enrollment", "already_verified");
        return Err(ApiError::mfa_verification_failed());
    }
    let seed_key_id = factor
        .try_get::<String, _>("seed_key_id")
        .map_err(|_| ApiError::service_unavailable())?;
    let seed_aead = factor
        .try_get::<String, _>("seed_aead")
        .map_err(|_| ApiError::service_unavailable())?;
    if seed_key_id != TOTP_SEED_KEY_ID || seed_aead != TOTP_SEED_AEAD {
        return Err(ApiError::service_unavailable());
    }
    let seed_ciphertext = factor
        .try_get::<Vec<u8>, _>("seed_ciphertext")
        .map_err(|_| ApiError::service_unavailable())?;
    let seed_nonce = factor
        .try_get::<Vec<u8>, _>("seed_nonce")
        .map_err(|_| ApiError::service_unavailable())?;
    let last_accepted_step = factor
        .try_get::<Option<i64>, _>("last_accepted_step")
        .map_err(|_| ApiError::service_unavailable())?
        .map(u64::try_from)
        .transpose()
        .map_err(|_| ApiError::service_unavailable())?;
    let aad = totp_seed_aad(session.account_id, request.factor_id, profile);
    let seed = decrypt_totp_seed(totp_seed_key, &seed_nonce, &aad, &seed_ciphertext)?;
    let accepted_step = totp::verify(
        &seed,
        unix_time_seconds(now)?,
        &request.code,
        profile,
        last_accepted_step,
    )
    .ok()
    .flatten();

    let Some(accepted_step) = accepted_step else {
        sqlx::query(
            "
            DELETE FROM totp_factors
            WHERE id = $1
              AND account_id = $2
              AND verified_at IS NULL
            ",
        )
        .bind(request.factor_id)
        .bind(session.account_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApiError::service_unavailable())?;
        insert_audit_event(
            &mut transaction,
            session.account_id,
            session.device_id,
            "mfa_totp_enrollment_failed",
            json!({ "factor_id": request.factor_id }),
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|_| ApiError::service_unavailable())?;
        telemetry::mfa_event("totp_enrollment", "failed");
        return Err(ApiError::mfa_verification_failed());
    };

    let recovery_codes = generate_recovery_codes();
    sqlx::query("DELETE FROM recovery_codes WHERE account_id = $1")
        .bind(session.account_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    for code in &recovery_codes {
        let salt = random_bytes::<RECOVERY_CODE_SALT_BYTES>();
        let hash = recovery_code_hash(session.account_id, &salt, code);
        sqlx::query(
            "
            INSERT INTO recovery_codes (
                id,
                account_id,
                code_salt,
                code_hash
            ) VALUES ($1, $2, $3, $4)
            ",
        )
        .bind(Uuid::new_v4())
        .bind(session.account_id)
        .bind(salt.as_slice())
        .bind(hash.as_slice())
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApiError::service_unavailable())?;
    }

    let session_token = tokens::random_token();
    let session_token_hash = tokens::sha256_verifier(&session_token);
    let idle_expires_at = min_time(now + SESSION_IDLE_TTL, session.absolute_expires_at);

    sqlx::query(
        "
        UPDATE totp_factors
        SET verified_at = $1,
            last_accepted_step = $2,
            updated_at = $1
        WHERE id = $3
        ",
    )
    .bind(now)
    .bind(i64::try_from(accepted_step).map_err(|_| ApiError::service_unavailable())?)
    .bind(request.factor_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    sqlx::query(
        "
        UPDATE sessions
        SET session_token_hash = $1,
            csrf_token_hash = NULL,
            session_state = 'mfa_verified',
            last_seen_at = $2,
            idle_expires_at = $3,
            expires_at = $3
        WHERE id = $4
        ",
    )
    .bind(session_token_hash.as_slice())
    .bind(now)
    .bind(idle_expires_at)
    .bind(session.id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    insert_audit_event(
        &mut transaction,
        session.account_id,
        session.device_id,
        "mfa_totp_enrollment_confirmed",
        json!({
            "factor_id": request.factor_id,
            "recovery_code_count": recovery_codes.len()
        }),
    )
    .await?;

    transaction
        .commit()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    telemetry::mfa_event("totp_enrollment", "confirmed");
    telemetry::session_event("upgraded", "mfa_verified");

    let mut response = no_store_json(
        StatusCode::OK,
        TotpEnrollConfirmResponse {
            factor_id: request.factor_id,
            status: "active",
            session: SessionResponse {
                state: "mfa_verified",
                vault_access: true,
                idle_expires_at: format_rfc3339(idle_expires_at)?,
                absolute_expires_at: format_rfc3339(session.absolute_expires_at)?,
            },
            recovery_codes,
        },
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie(&session_token))
            .map_err(|_| ApiError::service_unavailable())?,
    );

    Ok(response)
}

struct LoginMetadata {
    account_id: Option<Uuid>,
    kdf_profile: Value,
    account_salt: Vec<u8>,
    auth_verifier_salt: Vec<u8>,
    auth_verifier_iterations: u32,
}

#[derive(Deserialize, Serialize)]
struct RegisterChallengeMetadata {
    kdf_profile: Value,
    account_salt: String,
    auth_verifier_profile: String,
    auth_verifier_salt: String,
    auth_verifier_iterations: u32,
}

#[derive(Deserialize, Serialize)]
struct LoginChallengeMetadata {
    client_nonce: String,
    server_nonce: String,
    combined_nonce: String,
    auth_verifier_profile: String,
    auth_verifier_iterations: u32,
    synthetic: bool,
}

#[derive(Deserialize, Serialize)]
struct PreMfaChallengeMetadata {
    login_challenge_id: Uuid,
    device: DeviceChallengeMetadata,
}

#[derive(Deserialize, Serialize)]
struct DeviceChallengeMetadata {
    label: String,
    client_type: String,
    public_metadata: Value,
}

pub(crate) struct CurrentSession {
    pub(crate) id: Uuid,
    pub(crate) account_id: Uuid,
    pub(crate) device_id: Option<Uuid>,
    pub(crate) session_state: String,
    pub(crate) idle_expires_at: OffsetDateTime,
    pub(crate) absolute_expires_at: OffsetDateTime,
}

impl CurrentSession {
    pub(crate) fn vault_access(&self) -> bool {
        self.session_state == "mfa_verified"
    }
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

struct InsertChallengeTx<'a> {
    id: Uuid,
    account_id: Option<Uuid>,
    login_handle_normalized: &'a str,
    challenge_type: &'static str,
    server_nonce: &'a [u8],
    public_metadata: Value,
    expires_at: OffsetDateTime,
}

struct PreMfaChallenge {
    id: Uuid,
    account_id: Uuid,
    metadata: PreMfaChallengeMetadata,
    attempts: i32,
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

async fn insert_auth_challenge_tx(
    transaction: &mut Transaction<'_, Postgres>,
    input: InsertChallengeTx<'_>,
) -> Result<(), ApiError> {
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
    .execute(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    Ok(())
}

async fn consume_auth_challenge(
    transaction: &mut Transaction<'_, Postgres>,
    challenge_id: Uuid,
    now: OffsetDateTime,
) -> Result<(), ApiError> {
    sqlx::query("UPDATE auth_challenges SET consumed_at = $1 WHERE id = $2")
        .bind(now)
        .bind(challenge_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    Ok(())
}

async fn load_pre_mfa_challenge(
    transaction: &mut Transaction<'_, Postgres>,
    challenge_id: Uuid,
    now: OffsetDateTime,
) -> Result<Option<PreMfaChallenge>, ApiError> {
    let challenge = sqlx::query(
        "
        SELECT
            id,
            account_id,
            public_metadata,
            attempts
        FROM auth_challenges
        WHERE id = $1
          AND challenge_type = 'pre_mfa'
          AND auth_protocol = $2
          AND consumed_at IS NULL
          AND expires_at >= $3
        FOR UPDATE
        ",
    )
    .bind(challenge_id)
    .bind(AUTH_PROTOCOL)
    .bind(now)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let Some(challenge) = challenge else {
        return Ok(None);
    };
    let id = challenge
        .try_get::<Uuid, _>("id")
        .map_err(|_| ApiError::service_unavailable())?;
    let account_id = challenge
        .try_get::<Option<Uuid>, _>("account_id")
        .map_err(|_| ApiError::service_unavailable())?
        .ok_or_else(ApiError::service_unavailable)?;
    let public_metadata = challenge
        .try_get::<SqlJson<Value>, _>("public_metadata")
        .map_err(|_| ApiError::service_unavailable())?
        .0;
    let attempts = challenge
        .try_get::<i32, _>("attempts")
        .map_err(|_| ApiError::service_unavailable())?;

    Ok(Some(PreMfaChallenge {
        id,
        account_id,
        metadata: decode_pre_mfa_challenge_metadata(public_metadata)?,
        attempts,
    }))
}

async fn increment_challenge_attempts(
    transaction: &mut Transaction<'_, Postgres>,
    challenge_id: Uuid,
    now: OffsetDateTime,
) -> Result<(), ApiError> {
    sqlx::query(
        "
        UPDATE auth_challenges
        SET attempts = attempts + 1,
            consumed_at = CASE
                WHEN attempts + 1 >= $1 THEN $2
                ELSE consumed_at
            END
        WHERE id = $3
        ",
    )
    .bind(MFA_CHALLENGE_MAX_ATTEMPTS)
    .bind(now)
    .bind(challenge_id)
    .execute(&mut **transaction)
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

struct ValidatedEncryptedEnvelope {
    crypto_version: String,
    key_id: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

impl ValidatedEncryptedEnvelope {
    fn from_request(
        request: &EncryptedEnvelopeRequest,
        expected_crypto_version: &'static str,
    ) -> Result<Self, ApiError> {
        if request.crypto_version != expected_crypto_version {
            return Err(ApiError::bad_request());
        }
        let key_id = validate_short_text(&request.key_id, 128)?;
        let nonce = decode_base64url_array::<AEAD_NONCE_BYTES>(&request.nonce)
            .map_err(|_| ApiError::bad_request())?
            .to_vec();
        let ciphertext =
            decode_base64url(&request.ciphertext).map_err(|_| ApiError::bad_request())?;
        if ciphertext.is_empty() || ciphertext.len() > MAX_ENCRYPTED_ENVELOPE_BYTES {
            return Err(ApiError::bad_request());
        }

        Ok(Self {
            crypto_version: request.crypto_version.clone(),
            key_id,
            nonce,
            ciphertext,
        })
    }
}

struct ValidatedDevice {
    label: String,
    client_type: String,
    public_metadata: Value,
}

impl DeviceChallengeMetadata {
    fn from_validated(device: &ValidatedDevice) -> Self {
        Self {
            label: device.label.clone(),
            client_type: device.client_type.clone(),
            public_metadata: device.public_metadata.clone(),
        }
    }

    fn into_validated(self) -> Result<ValidatedDevice, ApiError> {
        ValidatedDevice::from_parts(self.label, self.client_type, self.public_metadata)
    }
}

impl ValidatedDevice {
    fn from_request(request: &DeviceRequest) -> Result<Self, ApiError> {
        Self::from_parts(
            request.label.clone(),
            request.client_type.clone(),
            request.public_metadata.clone(),
        )
    }

    fn from_parts(
        label: String,
        client_type: String,
        public_metadata: Value,
    ) -> Result<Self, ApiError> {
        let label = validate_short_text(&label, MAX_DEVICE_LABEL_BYTES)?;
        if !matches!(
            client_type.as_str(),
            "browser" | "browser-extension" | "ios" | "android" | "cli"
        ) {
            return Err(ApiError::bad_request());
        }
        if !public_metadata.is_object() {
            return Err(ApiError::bad_request());
        }
        let metadata_len = serde_json::to_vec(&public_metadata)
            .map_err(|_| ApiError::bad_request())?
            .len();
        if metadata_len > MAX_DEVICE_PUBLIC_METADATA_BYTES {
            return Err(ApiError::bad_request());
        }

        Ok(Self {
            label,
            client_type,
            public_metadata,
        })
    }
}

fn decode_register_challenge_metadata(value: Value) -> Result<RegisterChallengeMetadata, ApiError> {
    let metadata = serde_json::from_value::<RegisterChallengeMetadata>(value)
        .map_err(|_| ApiError::service_unavailable())?;
    if metadata.kdf_profile != default_kdf_profile()
        || metadata.auth_verifier_profile != PROFILE_ID
        || metadata.auth_verifier_iterations != DEFAULT_ITERATIONS
    {
        return Err(ApiError::service_unavailable());
    }
    Ok(metadata)
}

fn decode_login_challenge_metadata(value: Value) -> Result<LoginChallengeMetadata, ApiError> {
    let metadata = serde_json::from_value::<LoginChallengeMetadata>(value)
        .map_err(|_| ApiError::service_unavailable())?;
    if metadata.auth_verifier_profile != PROFILE_ID
        || metadata.auth_verifier_iterations != DEFAULT_ITERATIONS
    {
        return Err(ApiError::service_unavailable());
    }
    Ok(metadata)
}

fn decode_pre_mfa_challenge_metadata(value: Value) -> Result<PreMfaChallengeMetadata, ApiError> {
    serde_json::from_value::<PreMfaChallengeMetadata>(value)
        .map_err(|_| ApiError::service_unavailable())
}

fn ensure_totp_enrollment_session(session: &CurrentSession) -> Result<(), ApiError> {
    match session.session_state.as_str() {
        "mfa_enrollment_required" | "mfa_recovery" => Ok(()),
        _ => Err(ApiError::mfa_required()),
    }
}

async fn account_login_handle(pool: &PgPool, account_id: Uuid) -> Result<String, ApiError> {
    sqlx::query_scalar::<_, String>(
        "
        SELECT login_handle_normalized
        FROM accounts
        WHERE id = $1
        ",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())
}

fn totp_profile_response(profile: TotpProfile) -> TotpProfileResponse {
    TotpProfileResponse {
        algorithm: profile.algorithm.as_uri_value(),
        digits: profile.digits,
        period: profile.period_seconds,
    }
}

fn totp_profile_from_row(row: &sqlx::postgres::PgRow) -> Result<TotpProfile, ApiError> {
    let algorithm = match row
        .try_get::<String, _>("algorithm")
        .map_err(|_| ApiError::service_unavailable())?
        .as_str()
    {
        "SHA1" => TotpAlgorithm::Sha1,
        "SHA256" => TotpAlgorithm::Sha256,
        "SHA512" => TotpAlgorithm::Sha512,
        _ => return Err(ApiError::service_unavailable()),
    };
    let digits = row
        .try_get::<i32, _>("digits")
        .map_err(|_| ApiError::service_unavailable())?
        .try_into()
        .map_err(|_| ApiError::service_unavailable())?;
    let period_seconds = row
        .try_get::<i32, _>("period_seconds")
        .map_err(|_| ApiError::service_unavailable())?
        .try_into()
        .map_err(|_| ApiError::service_unavailable())?;
    Ok(TotpProfile {
        algorithm,
        digits,
        period_seconds,
    })
}

fn totp_seed_aad(account_id: Uuid, factor_id: Uuid, profile: TotpProfile) -> Vec<u8> {
    let mut aad = Vec::with_capacity(128);
    aad.extend_from_slice(b"password-vault/totp-seed/v1");
    aad.push(0);
    aad.extend_from_slice(account_id.as_bytes());
    aad.push(0);
    aad.extend_from_slice(factor_id.as_bytes());
    aad.push(0);
    aad.extend_from_slice(TOTP_SEED_KEY_ID.as_bytes());
    aad.push(0);
    aad.extend_from_slice(profile.algorithm.as_uri_value().as_bytes());
    aad.push(0);
    aad.extend_from_slice(&profile.digits.to_be_bytes());
    aad.push(0);
    aad.extend_from_slice(&profile.period_seconds.to_be_bytes());
    aad
}

fn encrypt_totp_seed(
    key: &[u8; 32],
    nonce: &[u8; XCHACHA20POLY1305_NONCE_BYTES],
    aad: &[u8],
    seed: &[u8; TOTP_SEED_BYTES],
) -> Result<Vec<u8>, ApiError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .encrypt(XNonce::from_slice(nonce), Payload { msg: seed, aad })
        .map_err(|_| ApiError::service_unavailable())
}

fn decrypt_totp_seed(
    key: &[u8; 32],
    nonce: &[u8],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, ApiError> {
    if nonce.len() != XCHACHA20POLY1305_NONCE_BYTES {
        return Err(ApiError::service_unavailable());
    }
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| ApiError::service_unavailable())
}

fn generate_recovery_codes() -> Vec<String> {
    (0..RECOVERY_CODE_COUNT)
        .map(|_| recovery_code_from_random(random_bytes::<RECOVERY_CODE_RANDOM_BYTES>()))
        .collect()
}

fn recovery_code_from_random(random: [u8; RECOVERY_CODE_RANDOM_BYTES]) -> String {
    let secret = totp::base32_no_padding(&random).to_ascii_lowercase();
    let mut output = String::with_capacity(5 + secret.len() + 6);
    output.push_str("pvrc-");
    for (index, chunk) in secret.as_bytes().chunks(4).enumerate() {
        if index > 0 {
            output.push('-');
        }
        output.push_str(std::str::from_utf8(chunk).expect("base32 emits ASCII"));
    }
    output
}

fn recovery_code_hash(
    account_id: Uuid,
    salt: &[u8; RECOVERY_CODE_SALT_BYTES],
    code: &str,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"password-vault/recovery-code/v1");
    hasher.update([0]);
    hasher.update(account_id.as_bytes());
    hasher.update([0]);
    hasher.update(salt);
    hasher.update([0]);
    hasher.update(code.trim().to_ascii_lowercase().as_bytes());
    hasher.finalize().into()
}

fn validate_recovery_code(code: &str) -> Result<String, ApiError> {
    let code = code.trim().to_ascii_lowercase();
    if code.is_empty()
        || code.len() > MAX_RECOVERY_CODE_BYTES
        || !code.starts_with("pvrc-")
        || code.chars().any(char::is_whitespace)
        || code.chars().any(|character| {
            !(character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
        })
    {
        return Err(ApiError::mfa_verification_failed());
    }
    Ok(code)
}

async fn find_matching_recovery_code(
    transaction: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    recovery_code: &str,
) -> Result<Option<Uuid>, ApiError> {
    let rows = sqlx::query(
        "
        SELECT id, code_salt, code_hash
        FROM recovery_codes
        WHERE account_id = $1
          AND used_at IS NULL
        FOR UPDATE
        ",
    )
    .bind(account_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let mut matched_id = None;
    for row in rows {
        let id = row
            .try_get::<Uuid, _>("id")
            .map_err(|_| ApiError::service_unavailable())?;
        let salt = recovery_code_salt_from_vec(
            row.try_get::<Vec<u8>, _>("code_salt")
                .map_err(|_| ApiError::service_unavailable())?,
        )?;
        let stored_hash = row
            .try_get::<Vec<u8>, _>("code_hash")
            .map_err(|_| ApiError::service_unavailable())?;
        let candidate_hash = recovery_code_hash(account_id, &salt, recovery_code);
        if stored_hash.len() == candidate_hash.len()
            && stored_hash
                .as_slice()
                .ct_eq(candidate_hash.as_slice())
                .into()
        {
            matched_id = Some(id);
        }
    }

    Ok(matched_id)
}

async fn consume_recovery_code(
    transaction: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    recovery_code_id: Uuid,
    now: OffsetDateTime,
) -> Result<(), ApiError> {
    let result = sqlx::query(
        "
        UPDATE recovery_codes
        SET used_at = $1
        WHERE id = $2
          AND account_id = $3
          AND used_at IS NULL
        ",
    )
    .bind(now)
    .bind(recovery_code_id)
    .bind(account_id)
    .execute(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(ApiError::mfa_verification_failed())
    }
}

fn recovery_code_salt_from_vec(value: Vec<u8>) -> Result<[u8; RECOVERY_CODE_SALT_BYTES], ApiError> {
    value
        .try_into()
        .map_err(|_| ApiError::service_unavailable())
}

pub(crate) async fn insert_audit_event(
    transaction: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    actor_device_id: Option<Uuid>,
    event_type: &'static str,
    event_metadata: Value,
) -> Result<(), ApiError> {
    sqlx::query(
        "
        INSERT INTO audit_events (
            account_id,
            actor_device_id,
            event_type,
            event_metadata
        ) VALUES ($1, $2, $3, $4)
        ",
    )
    .bind(account_id)
    .bind(actor_device_id)
    .bind(event_type)
    .bind(SqlJson(event_metadata))
    .execute(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    Ok(())
}

async fn create_session_with_device(
    transaction: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    device: &ValidatedDevice,
    session_state: &'static str,
    now: OffsetDateTime,
) -> Result<
    (
        Uuid,
        [u8; tokens::TOKEN_LEN],
        OffsetDateTime,
        OffsetDateTime,
    ),
    ApiError,
> {
    let device_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let session_token = tokens::random_token();
    let session_token_hash = tokens::sha256_verifier(&session_token);
    let idle_expires_at = now + SESSION_IDLE_TTL;
    let absolute_expires_at = now + SESSION_ABSOLUTE_TTL;

    sqlx::query(
        "
        INSERT INTO devices (
            id,
            account_id,
            display_name,
            user_agent_hash,
            client_type,
            public_metadata,
            last_seen_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        ",
    )
    .bind(device_id)
    .bind(account_id)
    .bind(&device.label)
    .bind(Option::<Vec<u8>>::None)
    .bind(&device.client_type)
    .bind(SqlJson(device.public_metadata.clone()))
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    sqlx::query(
        "
        INSERT INTO sessions (
            id,
            account_id,
            device_id,
            session_token_hash,
            csrf_token_hash,
            session_state,
            expires_at,
            idle_expires_at,
            absolute_expires_at,
            last_seen_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $7, $8, $9)
        ",
    )
    .bind(session_id)
    .bind(account_id)
    .bind(device_id)
    .bind(session_token_hash.as_slice())
    .bind(Option::<&[u8]>::None)
    .bind(session_state)
    .bind(idle_expires_at)
    .bind(absolute_expires_at)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    Ok((
        device_id,
        session_token,
        idle_expires_at,
        absolute_expires_at,
    ))
}

fn auth_key_from_vec(value: Vec<u8>) -> Result<[u8; AUTH_KEY_BYTES], ApiError> {
    value
        .try_into()
        .map_err(|_| ApiError::service_unavailable())
}

fn validate_short_text(value: &str, max_bytes: usize) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max_bytes || trimmed.chars().any(char::is_control) {
        return Err(ApiError::bad_request());
    }
    Ok(trimmed.to_string())
}

fn genesis_head_hash(vault_id: Uuid) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"password-vault/vault-genesis/v1");
    hasher.update([0]);
    hasher.update(vault_id.as_bytes());
    hasher.finalize().into()
}

fn session_cookie(session_token: &[u8; tokens::TOKEN_LEN]) -> String {
    format!(
        "{SESSION_COOKIE_NAME}={}; Path=/; Secure; HttpOnly; SameSite=Strict",
        encode_base64url(session_token)
    )
}

const fn clear_session_cookie() -> &'static str {
    "__Host-pv_session=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
}

fn session_token_from_headers(headers: &HeaderMap) -> Option<[u8; tokens::TOKEN_LEN]> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    let mut seen_session_cookie = false;
    let mut session_token = None;
    for part in cookie_header.split(';') {
        if let Some((name, value)) = part.trim().split_once('=')
            && name == SESSION_COOKIE_NAME
        {
            if seen_session_cookie {
                return None;
            }
            seen_session_cookie = true;
            session_token = Some(decode_base64url_array::<{ tokens::TOKEN_LEN }>(value).ok()?);
        }
    }
    session_token
}

fn csrf_token_from_headers(headers: &HeaderMap) -> Option<[u8; tokens::TOKEN_LEN]> {
    let value = headers.get("x-pv-csrf")?.to_str().ok()?;
    decode_base64url_array::<{ tokens::TOKEN_LEN }>(value).ok()
}

pub(crate) async fn ensure_csrf_token(
    pool: &PgPool,
    headers: &HeaderMap,
    session_id: Uuid,
) -> Result<(), ApiError> {
    let csrf_token = csrf_token_from_headers(headers).ok_or_else(ApiError::csrf_required)?;
    let csrf_token_hash = tokens::sha256_verifier(&csrf_token);
    let stored_hash = sqlx::query_scalar::<_, Option<Vec<u8>>>(
        "
        SELECT csrf_token_hash
        FROM sessions
        WHERE id = $1
          AND revoked_at IS NULL
        ",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?
    .flatten()
    .ok_or_else(ApiError::csrf_required)?;

    if stored_hash.len() == tokens::TOKEN_LEN
        && stored_hash
            .as_slice()
            .ct_eq(csrf_token_hash.as_slice())
            .into()
    {
        Ok(())
    } else {
        Err(ApiError::csrf_required())
    }
}

pub(crate) fn ensure_unsafe_request_context(headers: &HeaderMap) -> Result<(), ApiError> {
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("cross-site"))
    {
        return Err(ApiError::csrf_required());
    }

    if let Some(origin) = headers.get("origin").and_then(|value| value.to_str().ok()) {
        let host = headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(ApiError::csrf_required)?;
        let origin_host = origin_host(origin).ok_or_else(ApiError::csrf_required)?;
        if origin_host != host {
            return Err(ApiError::csrf_required());
        }
    }

    Ok(())
}

fn origin_host(origin: &str) -> Option<&str> {
    let without_scheme = origin.strip_prefix("https://")?;
    without_scheme
        .split('/')
        .next()
        .filter(|host| !host.is_empty())
}

pub(crate) async fn load_current_session(
    pool: &PgPool,
    headers: &HeaderMap,
    now: OffsetDateTime,
) -> Result<Option<CurrentSession>, ApiError> {
    let Some(session_token) = session_token_from_headers(headers) else {
        return Ok(None);
    };
    let session_token_hash = tokens::sha256_verifier(&session_token);

    let row = sqlx::query(
        "
        SELECT
            s.id,
            s.account_id,
            s.device_id,
            s.session_state,
            s.idle_expires_at,
            s.absolute_expires_at,
            d.revoked_at AS device_revoked_at
        FROM sessions s
        LEFT JOIN devices d
          ON d.account_id = s.account_id
         AND d.id = s.device_id
        WHERE s.session_token_hash = $1
          AND s.revoked_at IS NULL
        ",
    )
    .bind(session_token_hash.as_slice())
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let Some(row) = row else {
        return Ok(None);
    };

    let id = row
        .try_get::<Uuid, _>("id")
        .map_err(|_| ApiError::service_unavailable())?;
    let account_id = row
        .try_get::<Uuid, _>("account_id")
        .map_err(|_| ApiError::service_unavailable())?;
    let device_id = row
        .try_get::<Option<Uuid>, _>("device_id")
        .map_err(|_| ApiError::service_unavailable())?;
    let session_state = row
        .try_get::<String, _>("session_state")
        .map_err(|_| ApiError::service_unavailable())?;
    let idle_expires_at = row
        .try_get::<OffsetDateTime, _>("idle_expires_at")
        .map_err(|_| ApiError::service_unavailable())?;
    let absolute_expires_at = row
        .try_get::<OffsetDateTime, _>("absolute_expires_at")
        .map_err(|_| ApiError::service_unavailable())?;
    let device_revoked_at = row
        .try_get::<Option<OffsetDateTime>, _>("device_revoked_at")
        .map_err(|_| ApiError::service_unavailable())?;

    if device_revoked_at.is_some() || idle_expires_at <= now || absolute_expires_at <= now {
        revoke_session(pool, id, now).await?;
        return Ok(None);
    }

    Ok(Some(CurrentSession {
        id,
        account_id,
        device_id,
        session_state,
        idle_expires_at,
        absolute_expires_at,
    }))
}

pub(crate) async fn refresh_session_activity(
    pool: &PgPool,
    mut session: CurrentSession,
    now: OffsetDateTime,
) -> Result<CurrentSession, ApiError> {
    let refreshed_idle_expires_at = min_time(now + SESSION_IDLE_TTL, session.absolute_expires_at);
    sqlx::query(
        "
        UPDATE sessions
        SET last_seen_at = $1,
            idle_expires_at = $2,
            expires_at = $2
        WHERE id = $3
        ",
    )
    .bind(now)
    .bind(refreshed_idle_expires_at)
    .bind(session.id)
    .execute(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    session.idle_expires_at = refreshed_idle_expires_at;
    Ok(session)
}

async fn revoke_session(
    pool: &PgPool,
    session_id: Uuid,
    now: OffsetDateTime,
) -> Result<(), ApiError> {
    sqlx::query("UPDATE sessions SET revoked_at = $1 WHERE id = $2")
        .bind(now)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|_| ApiError::service_unavailable())?;
    Ok(())
}

fn min_time(left: OffsetDateTime, right: OffsetDateTime) -> OffsetDateTime {
    if left <= right { left } else { right }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => database_error
            .code()
            .as_deref()
            .is_some_and(|code| code == "23505"),
        _ => false,
    }
}

async fn enforce_challenge_rate_limit(
    pool: &PgPool,
    login_handle_normalized: &str,
    challenge_type: &'static str,
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
        telemetry::rate_limited_request("auth_challenge", challenge_type);
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
    let mut mac = <HmacSha256 as Mac>::new_from_slice(synthetic_metadata_key)
        .expect("HMAC accepts any key length");
    mac.update(domain.as_bytes());
    mac.update(&[0]);
    mac.update(login_handle_normalized.as_bytes());
    mac.finalize().into_bytes().into()
}

fn default_kdf_profile() -> Value {
    json!({
        "id": PBKDF2_BROWSER_PROFILE_ID,
        "algorithm": "PBKDF2-HMAC-SHA-256",
        "iterations": PBKDF2_BROWSER_ITERATIONS,
        "hash": "SHA-256",
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

pub(crate) fn database_pool(state: &AppState) -> Result<&PgPool, ApiError> {
    state
        .database
        .as_ref()
        .ok_or_else(ApiError::service_unavailable)
}

pub(crate) fn no_store_json<T: Serialize>(status: StatusCode, body: T) -> Response {
    let mut response = (status, Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub(crate) fn format_rfc3339(value: OffsetDateTime) -> Result<String, ApiError> {
    value
        .format(&Rfc3339)
        .map_err(|_| ApiError::service_unavailable())
}

pub(crate) fn now_utc_second() -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .map_err(|_| ApiError::service_unavailable())
}

fn unix_time_seconds(value: OffsetDateTime) -> Result<u64, ApiError> {
    value
        .unix_timestamp()
        .try_into()
        .map_err(|_| ApiError::service_unavailable())
}

pub(crate) struct StrictJson<T>(pub(crate) T);

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

pub(crate) async fn add_no_store_header(
    request: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};

    use uuid::Uuid;

    use crate::auth::{encoding::encode_base64url, tokens};

    use super::{
        SESSION_COOKIE_NAME, csrf_token_from_headers, normalize_login_handle, recovery_code_hash,
        recovery_code_salt_from_vec, session_token_from_headers, synthetic_login_metadata,
        validate_recovery_code,
    };

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

    #[test]
    fn session_cookie_parser_accepts_one_host_cookie_among_others() {
        let token = [0x42; tokens::TOKEN_LEN];
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!(
                "foo=1; {SESSION_COOKIE_NAME}={}; bar=2",
                encode_base64url(&token)
            ))
            .expect("test cookie header is valid"),
        );

        assert_eq!(
            session_token_from_headers(&headers).expect("session token parses"),
            token
        );
    }

    #[test]
    fn session_cookie_parser_rejects_duplicate_or_wrong_length_tokens() {
        let token = encode_base64url(&[0x42; tokens::TOKEN_LEN]);
        let mut duplicate = HeaderMap::new();
        duplicate.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!(
                "{SESSION_COOKIE_NAME}={token}; {SESSION_COOKIE_NAME}={token}"
            ))
            .expect("test cookie header is valid"),
        );
        assert!(session_token_from_headers(&duplicate).is_none());

        let mut wrong_length = HeaderMap::new();
        wrong_length.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!(
                "{SESSION_COOKIE_NAME}={}",
                encode_base64url(&[0x42; 31])
            ))
            .expect("test cookie header is valid"),
        );
        assert!(session_token_from_headers(&wrong_length).is_none());
    }

    #[test]
    fn csrf_header_parser_accepts_only_base64url_32_byte_tokens() {
        let token = [0x24; tokens::TOKEN_LEN];
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-pv-csrf",
            HeaderValue::from_str(&encode_base64url(&token)).expect("test csrf header is valid"),
        );
        assert_eq!(
            csrf_token_from_headers(&headers).expect("csrf token parses"),
            token
        );

        headers.insert(
            "x-pv-csrf",
            HeaderValue::from_str(&encode_base64url(&[0x24; 31]))
                .expect("test csrf header is valid"),
        );
        assert!(csrf_token_from_headers(&headers).is_none());
    }

    #[test]
    fn recovery_code_validation_normalizes_only_expected_shape() {
        assert_eq!(
            validate_recovery_code(" PVRC-abcd-2345-efgh ").expect("valid code normalizes"),
            "pvrc-abcd-2345-efgh"
        );
        assert!(validate_recovery_code("abcd-2345").is_err());
        assert!(validate_recovery_code("pvrc-abcd 2345").is_err());
        assert!(validate_recovery_code("pvrc-abcd_2345").is_err());
        assert!(validate_recovery_code(&format!("pvrc-{}", "a".repeat(129))).is_err());
    }

    #[test]
    fn recovery_code_hash_is_normalized_and_account_bound() {
        let account_id =
            Uuid::parse_str("00000000-0000-4000-8000-000000000123").expect("uuid fixture parses");
        let other_account_id =
            Uuid::parse_str("00000000-0000-4000-8000-000000000124").expect("uuid fixture parses");
        let salt = [0x42; 16];

        assert_eq!(
            recovery_code_hash(account_id, &salt, "PVRC-ABCD-2345"),
            recovery_code_hash(account_id, &salt, " pvrc-abcd-2345 ")
        );
        assert_ne!(
            recovery_code_hash(account_id, &salt, "pvrc-abcd-2345"),
            recovery_code_hash(other_account_id, &salt, "pvrc-abcd-2345")
        );
        assert_eq!(
            recovery_code_salt_from_vec(salt.to_vec()).expect("salt length is valid"),
            salt
        );
        assert!(recovery_code_salt_from_vec(vec![0x42; 15]).is_err());
    }
}
