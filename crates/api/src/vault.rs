use axum::{
    Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction, types::Json as SqlJson};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{
        encoding::{decode_base64url, decode_base64url_array, encode_base64url},
        routes::{
            ApiError, CurrentSession, StrictJson, add_no_store_header, database_pool,
            ensure_csrf_token, ensure_unsafe_request_context, format_rfc3339, insert_audit_event,
            load_current_session, no_store_json, now_utc_second, refresh_session_activity,
        },
    },
    telemetry,
};

const VAULT_BODY_LIMIT_BYTES: usize = 128 * 1024;
const ITEM_ENVELOPE_CRYPTO_VERSION: &str = "item-envelope-v1";
const ITEM_ENVELOPE_AEAD: &str = "AES-256-GCM";
const ITEM_ENVELOPE_NONCE_BYTES: usize = 12;
const HASH_BYTES: usize = 32;
const MAX_KEY_ID_BYTES: usize = 128;
const MAX_ITEM_ENVELOPE_CIPHERTEXT_BYTES: usize = 64 * 1024;
const MAX_ITEM_ENVELOPE_JSON_BYTES: usize = 96 * 1024;
const MAX_SYNC_CHANGES_PER_RESPONSE: usize = 500;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/vaults", get(list_vaults))
        .route("/v1/vaults/{vault_id}/sync", get(sync_vault))
        .route("/v1/vaults/{vault_id}/items", post(create_item))
        .route(
            "/v1/vaults/{vault_id}/items/{item_id}/revisions",
            post(create_item_revision),
        )
        .layer(DefaultBodyLimit::max(VAULT_BODY_LIMIT_BYTES))
        .layer(middleware::from_fn(add_no_store_header))
}

#[derive(Deserialize)]
struct VaultPath {
    vault_id: Uuid,
}

#[derive(Deserialize)]
struct VaultItemPath {
    vault_id: Uuid,
    item_id: Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncQuery {
    from_head_seq: i64,
    from_head_hash: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateItemRequest {
    item_id: Uuid,
    revision_id: Uuid,
    base_head_seq: i64,
    base_head_hash: String,
    new_head_hash: String,
    change_mac: String,
    envelope_hash: String,
    encrypted_item_envelope: EncryptedItemEnvelopeRequest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRevisionRequest {
    revision_id: Uuid,
    operation: String,
    base_revision_seq: i64,
    base_head_seq: i64,
    base_head_hash: String,
    new_head_hash: String,
    change_mac: String,
    envelope_hash: String,
    encrypted_item_envelope: EncryptedItemEnvelopeRequest,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EncryptedItemEnvelopeRequest {
    crypto_version: String,
    key_id: String,
    aead: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Clone)]
struct ValidatedItemEnvelope {
    value: Value,
    crypto_version: String,
    key_id: String,
}

#[derive(Serialize)]
struct VaultListResponse {
    vaults: Vec<VaultResponse>,
}

#[derive(Serialize)]
struct VaultResponse {
    vault_id: Uuid,
    head_seq: i64,
    head_hash: String,
    encrypted_vault_key: EncryptedVaultKeyResponse,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct EncryptedVaultKeyResponse {
    crypto_version: String,
    key_id: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Serialize)]
struct HeadResponse {
    seq: i64,
    hash: String,
}

#[derive(Serialize)]
struct SyncResponse {
    from_head: HeadResponse,
    to_head: HeadResponse,
    has_more: bool,
    changes: Vec<SyncChangeResponse>,
}

#[derive(Serialize)]
struct SyncChangeResponse {
    item_id: Uuid,
    revision_id: Uuid,
    operation: String,
    revision_seq: i64,
    head_seq: i64,
    previous_head_hash: String,
    head_hash: String,
    base_revision_seq: i64,
    base_head_seq: i64,
    base_head_hash: String,
    change_mac: String,
    envelope_hash: String,
    encrypted_item_envelope: Value,
}

#[derive(Serialize)]
struct ItemRevisionWriteResponse {
    item_id: Uuid,
    revision_id: Uuid,
    revision_seq: i64,
    head_seq: i64,
    head_hash: String,
}

#[derive(Serialize)]
struct VaultConflictResponse {
    error: VaultConflictError,
    current_head: HeadResponse,
}

#[derive(Serialize)]
struct VaultConflictError {
    code: &'static str,
    message: &'static str,
}

struct VaultHead {
    head_seq: i64,
    head_hash: Vec<u8>,
    genesis_head_hash: Vec<u8>,
}

struct ValidatedCreateItem {
    item_id: Uuid,
    revision_id: Uuid,
    base_head_seq: i64,
    base_head_hash: [u8; HASH_BYTES],
    new_head_hash: [u8; HASH_BYTES],
    change_mac: [u8; HASH_BYTES],
    envelope_hash: [u8; HASH_BYTES],
    encrypted_item_envelope: ValidatedItemEnvelope,
}

struct ValidatedCreateRevision {
    revision_id: Uuid,
    operation: &'static str,
    base_revision_seq: i64,
    base_head_seq: i64,
    base_head_hash: [u8; HASH_BYTES],
    new_head_hash: [u8; HASH_BYTES],
    change_mac: [u8; HASH_BYTES],
    envelope_hash: [u8; HASH_BYTES],
    encrypted_item_envelope: ValidatedItemEnvelope,
}

async fn list_vaults(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let pool = database_pool(&state)?;
    let session = require_vault_session(pool, &headers).await?;

    let rows = sqlx::query(
        "
        SELECT
            v.id AS vault_id,
            v.head_seq,
            v.head_hash,
            v.created_at,
            v.updated_at,
            w.crypto_version AS wrap_crypto_version,
            w.key_id AS wrap_key_id,
            w.nonce AS wrap_nonce,
            w.ciphertext AS wrap_ciphertext
        FROM vaults v
        JOIN vault_key_wraps w
          ON w.vault_id = v.id
         AND w.account_id = v.account_id
        WHERE v.account_id = $1
        ORDER BY v.created_at, v.id
        ",
    )
    .bind(session.account_id)
    .fetch_all(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let mut vaults = Vec::with_capacity(rows.len());
    for row in rows {
        let head_hash = row
            .try_get::<Vec<u8>, _>("head_hash")
            .map_err(|_| ApiError::service_unavailable())?;
        let nonce = row
            .try_get::<Vec<u8>, _>("wrap_nonce")
            .map_err(|_| ApiError::service_unavailable())?;
        let ciphertext = row
            .try_get::<Vec<u8>, _>("wrap_ciphertext")
            .map_err(|_| ApiError::service_unavailable())?;
        vaults.push(VaultResponse {
            vault_id: row
                .try_get::<Uuid, _>("vault_id")
                .map_err(|_| ApiError::service_unavailable())?,
            head_seq: row
                .try_get::<i64, _>("head_seq")
                .map_err(|_| ApiError::service_unavailable())?,
            head_hash: encode_base64url(&head_hash),
            encrypted_vault_key: EncryptedVaultKeyResponse {
                crypto_version: row
                    .try_get::<String, _>("wrap_crypto_version")
                    .map_err(|_| ApiError::service_unavailable())?,
                key_id: row
                    .try_get::<String, _>("wrap_key_id")
                    .map_err(|_| ApiError::service_unavailable())?,
                nonce: encode_base64url(&nonce),
                ciphertext: encode_base64url(&ciphertext),
            },
            created_at: format_rfc3339(
                row.try_get("created_at")
                    .map_err(|_| ApiError::service_unavailable())?,
            )?,
            updated_at: format_rfc3339(
                row.try_get("updated_at")
                    .map_err(|_| ApiError::service_unavailable())?,
            )?,
        });
    }

    Ok(no_store_json(StatusCode::OK, VaultListResponse { vaults }))
}

async fn sync_vault(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<VaultPath>,
    Query(query): Query<SyncQuery>,
) -> Result<Response, ApiError> {
    let pool = database_pool(&state)?;
    let session = require_vault_session(pool, &headers).await?;
    let from_head_seq = validate_nonnegative_seq(query.from_head_seq)?;
    let from_head_hash = decode_base64url_array::<HASH_BYTES>(&query.from_head_hash)
        .map_err(|_| ApiError::bad_request())?;

    let Some(vault) = load_vault_head(pool, session.account_id, path.vault_id).await? else {
        return Err(ApiError::not_found());
    };

    if !sync_cursor_matches(pool, path.vault_id, &vault, from_head_seq, &from_head_hash).await? {
        telemetry::sync_request("conflict", "none");
        return Ok(vault_conflict_response(&vault));
    }

    let rows = sqlx::query(
        "
        SELECT
            id,
            item_id,
            operation,
            revision_seq,
            base_revision_seq,
            head_seq,
            base_head_seq,
            base_head_hash,
            previous_head_hash,
            head_hash,
            change_mac,
            envelope_hash,
            encrypted_item_envelope
        FROM vault_item_revisions
        WHERE vault_id = $1
          AND head_seq > $2
          AND head_seq <= $3
        ORDER BY head_seq ASC
        LIMIT $4
        ",
    )
    .bind(path.vault_id)
    .bind(from_head_seq)
    .bind(vault.head_seq)
    .bind((MAX_SYNC_CHANGES_PER_RESPONSE + 1) as i64)
    .fetch_all(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let has_more = rows.len() > MAX_SYNC_CHANGES_PER_RESPONSE;
    let mut rows = rows;
    if has_more {
        rows.truncate(MAX_SYNC_CHANGES_PER_RESPONSE);
    }
    let (to_head_seq, to_head_hash) = if let Some(last_row) = rows.last() {
        (
            last_row
                .try_get::<i64, _>("head_seq")
                .map_err(|_| ApiError::service_unavailable())?,
            last_row
                .try_get::<Vec<u8>, _>("head_hash")
                .map_err(|_| ApiError::service_unavailable())?,
        )
    } else {
        (vault.head_seq, vault.head_hash.clone())
    };

    let mut changes = Vec::with_capacity(rows.len());
    for row in rows {
        changes.push(SyncChangeResponse {
            item_id: row
                .try_get::<Uuid, _>("item_id")
                .map_err(|_| ApiError::service_unavailable())?,
            revision_id: row
                .try_get::<Uuid, _>("id")
                .map_err(|_| ApiError::service_unavailable())?,
            operation: row
                .try_get::<String, _>("operation")
                .map_err(|_| ApiError::service_unavailable())?,
            revision_seq: row
                .try_get::<i64, _>("revision_seq")
                .map_err(|_| ApiError::service_unavailable())?,
            head_seq: row
                .try_get::<i64, _>("head_seq")
                .map_err(|_| ApiError::service_unavailable())?,
            previous_head_hash: encode_base64url(
                &row.try_get::<Vec<u8>, _>("previous_head_hash")
                    .map_err(|_| ApiError::service_unavailable())?,
            ),
            head_hash: encode_base64url(
                &row.try_get::<Vec<u8>, _>("head_hash")
                    .map_err(|_| ApiError::service_unavailable())?,
            ),
            base_revision_seq: row
                .try_get::<i64, _>("base_revision_seq")
                .map_err(|_| ApiError::service_unavailable())?,
            base_head_seq: row
                .try_get::<i64, _>("base_head_seq")
                .map_err(|_| ApiError::service_unavailable())?,
            base_head_hash: encode_base64url(
                &row.try_get::<Vec<u8>, _>("base_head_hash")
                    .map_err(|_| ApiError::service_unavailable())?,
            ),
            change_mac: encode_base64url(
                &row.try_get::<Vec<u8>, _>("change_mac")
                    .map_err(|_| ApiError::service_unavailable())?,
            ),
            envelope_hash: encode_base64url(
                &row.try_get::<Vec<u8>, _>("envelope_hash")
                    .map_err(|_| ApiError::service_unavailable())?,
            ),
            encrypted_item_envelope: row
                .try_get::<SqlJson<Value>, _>("encrypted_item_envelope")
                .map_err(|_| ApiError::service_unavailable())?
                .0,
        });
    }

    telemetry::sync_request("success", if has_more { "partial" } else { "complete" });

    Ok(no_store_json(
        StatusCode::OK,
        SyncResponse {
            from_head: HeadResponse {
                seq: from_head_seq,
                hash: encode_base64url(&from_head_hash),
            },
            to_head: HeadResponse {
                seq: to_head_seq,
                hash: encode_base64url(&to_head_hash),
            },
            has_more,
            changes,
        },
    ))
}

async fn create_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<VaultPath>,
    StrictJson(request): StrictJson<CreateItemRequest>,
) -> Result<Response, ApiError> {
    ensure_unsafe_request_context(&headers)?;
    let pool = database_pool(&state)?;
    let session = require_vault_session(pool, &headers).await?;
    ensure_csrf_token(pool, &headers, session.id).await?;
    let request = ValidatedCreateItem::from_request(request)?;
    let now = now_utc_second()?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    let Some(vault) = lock_vault_head(&mut transaction, session.account_id, path.vault_id).await?
    else {
        telemetry::vault_item_change("create", "not_found");
        return Err(ApiError::not_found());
    };
    if !head_matches(&vault, request.base_head_seq, &request.base_head_hash) {
        telemetry::vault_item_change("create", "conflict");
        return Ok(vault_conflict_response(&vault));
    }

    let head_seq = vault.head_seq + 1;
    if let Err(error) = insert_item_row(&mut transaction, path.vault_id, request.item_id).await {
        return match error {
            VaultWriteError::Conflict => Ok(vault_conflict_response(&vault)),
            VaultWriteError::Api(error) => Err(error),
        };
    }
    if let Err(error) = insert_revision_row(
        &mut transaction,
        InsertRevisionRow {
            revision_id: request.revision_id,
            vault_id: path.vault_id,
            item_id: request.item_id,
            operation: "create",
            revision_seq: 1,
            base_revision_seq: 0,
            head_seq,
            base_head_seq: request.base_head_seq,
            base_head_hash: request.base_head_hash.as_slice(),
            previous_head_hash: request.base_head_hash.as_slice(),
            head_hash: request.new_head_hash.as_slice(),
            change_mac: request.change_mac.as_slice(),
            envelope_hash: request.envelope_hash.as_slice(),
            envelope: &request.encrypted_item_envelope,
        },
    )
    .await
    {
        return match error {
            VaultWriteError::Conflict => Ok(vault_conflict_response(&vault)),
            VaultWriteError::Api(error) => Err(error),
        };
    }
    update_item_latest(
        &mut transaction,
        path.vault_id,
        request.item_id,
        request.revision_id,
        1,
        false,
        now,
    )
    .await?;
    update_vault_head(
        &mut transaction,
        session.account_id,
        path.vault_id,
        head_seq,
        request.new_head_hash.as_slice(),
        now,
    )
    .await?;
    insert_audit_event(
        &mut transaction,
        session.account_id,
        session.device_id,
        "vault_item_created",
        json!({
            "vault_id": path.vault_id,
            "item_id": request.item_id,
            "revision_id": request.revision_id,
            "head_seq": head_seq,
            "revision_seq": 1
        }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    telemetry::vault_item_change("create", "success");

    Ok(no_store_json(
        StatusCode::CREATED,
        ItemRevisionWriteResponse {
            item_id: request.item_id,
            revision_id: request.revision_id,
            revision_seq: 1,
            head_seq,
            head_hash: encode_base64url(&request.new_head_hash),
        },
    ))
}

async fn create_item_revision(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<VaultItemPath>,
    StrictJson(request): StrictJson<CreateRevisionRequest>,
) -> Result<Response, ApiError> {
    ensure_unsafe_request_context(&headers)?;
    let pool = database_pool(&state)?;
    let session = require_vault_session(pool, &headers).await?;
    ensure_csrf_token(pool, &headers, session.id).await?;
    let request = ValidatedCreateRevision::from_request(request)?;
    let now = now_utc_second()?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    let Some(vault) = lock_vault_head(&mut transaction, session.account_id, path.vault_id).await?
    else {
        telemetry::vault_item_change(request.operation, "vault_not_found");
        return Err(ApiError::not_found());
    };
    let Some(item) = lock_item(&mut transaction, path.vault_id, path.item_id).await? else {
        telemetry::vault_item_change(request.operation, "item_not_found");
        return Err(ApiError::not_found());
    };

    if !head_matches(&vault, request.base_head_seq, &request.base_head_hash)
        || request.base_revision_seq != item.latest_revision_seq
        || item.deleted
    {
        telemetry::vault_item_change(request.operation, "conflict");
        return Ok(vault_conflict_response(&vault));
    }

    let revision_seq = item.latest_revision_seq + 1;
    let head_seq = vault.head_seq + 1;
    if let Err(error) = insert_revision_row(
        &mut transaction,
        InsertRevisionRow {
            revision_id: request.revision_id,
            vault_id: path.vault_id,
            item_id: path.item_id,
            operation: request.operation,
            revision_seq,
            base_revision_seq: request.base_revision_seq,
            head_seq,
            base_head_seq: request.base_head_seq,
            base_head_hash: request.base_head_hash.as_slice(),
            previous_head_hash: request.base_head_hash.as_slice(),
            head_hash: request.new_head_hash.as_slice(),
            change_mac: request.change_mac.as_slice(),
            envelope_hash: request.envelope_hash.as_slice(),
            envelope: &request.encrypted_item_envelope,
        },
    )
    .await
    {
        return match error {
            VaultWriteError::Conflict => Ok(vault_conflict_response(&vault)),
            VaultWriteError::Api(error) => Err(error),
        };
    }
    update_item_latest(
        &mut transaction,
        path.vault_id,
        path.item_id,
        request.revision_id,
        revision_seq,
        request.operation == "delete",
        now,
    )
    .await?;
    update_vault_head(
        &mut transaction,
        session.account_id,
        path.vault_id,
        head_seq,
        request.new_head_hash.as_slice(),
        now,
    )
    .await?;
    insert_audit_event(
        &mut transaction,
        session.account_id,
        session.device_id,
        match request.operation {
            "update" => "vault_item_updated",
            "delete" => "vault_item_deleted",
            _ => return Err(ApiError::bad_request()),
        },
        json!({
            "vault_id": path.vault_id,
            "item_id": path.item_id,
            "revision_id": request.revision_id,
            "head_seq": head_seq,
            "revision_seq": revision_seq
        }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| ApiError::service_unavailable())?;

    telemetry::vault_item_change(request.operation, "success");

    Ok(no_store_json(
        StatusCode::CREATED,
        ItemRevisionWriteResponse {
            item_id: path.item_id,
            revision_id: request.revision_id,
            revision_seq,
            head_seq,
            head_hash: encode_base64url(&request.new_head_hash),
        },
    ))
}

impl ValidatedCreateItem {
    fn from_request(request: CreateItemRequest) -> Result<Self, ApiError> {
        Ok(Self {
            item_id: request.item_id,
            revision_id: request.revision_id,
            base_head_seq: validate_nonnegative_seq(request.base_head_seq)?,
            base_head_hash: decode_hash(&request.base_head_hash)?,
            new_head_hash: decode_hash(&request.new_head_hash)?,
            change_mac: decode_hash(&request.change_mac)?,
            envelope_hash: decode_hash(&request.envelope_hash)?,
            encrypted_item_envelope: ValidatedItemEnvelope::from_request(
                request.encrypted_item_envelope,
            )?,
        })
    }
}

impl ValidatedCreateRevision {
    fn from_request(request: CreateRevisionRequest) -> Result<Self, ApiError> {
        let operation = match request.operation.as_str() {
            "update" => "update",
            "delete" => "delete",
            _ => return Err(ApiError::bad_request()),
        };
        let base_revision_seq = validate_positive_seq(request.base_revision_seq)?;
        Ok(Self {
            revision_id: request.revision_id,
            operation,
            base_revision_seq,
            base_head_seq: validate_nonnegative_seq(request.base_head_seq)?,
            base_head_hash: decode_hash(&request.base_head_hash)?,
            new_head_hash: decode_hash(&request.new_head_hash)?,
            change_mac: decode_hash(&request.change_mac)?,
            envelope_hash: decode_hash(&request.envelope_hash)?,
            encrypted_item_envelope: ValidatedItemEnvelope::from_request(
                request.encrypted_item_envelope,
            )?,
        })
    }
}

impl ValidatedItemEnvelope {
    fn from_request(request: EncryptedItemEnvelopeRequest) -> Result<Self, ApiError> {
        if request.crypto_version != ITEM_ENVELOPE_CRYPTO_VERSION
            || request.aead != ITEM_ENVELOPE_AEAD
        {
            return Err(ApiError::bad_request());
        }
        let key_id = validate_short_text(&request.key_id, MAX_KEY_ID_BYTES)?;
        let _nonce = decode_base64url_array::<ITEM_ENVELOPE_NONCE_BYTES>(&request.nonce)
            .map_err(|_| ApiError::bad_request())?;
        let ciphertext =
            decode_base64url(&request.ciphertext).map_err(|_| ApiError::bad_request())?;
        if ciphertext.is_empty() || ciphertext.len() > MAX_ITEM_ENVELOPE_CIPHERTEXT_BYTES {
            return Err(ApiError::bad_request());
        }

        let value = serde_json::to_value(&request).map_err(|_| ApiError::bad_request())?;
        let value_len = serde_json::to_vec(&value)
            .map_err(|_| ApiError::bad_request())?
            .len();
        if value_len > MAX_ITEM_ENVELOPE_JSON_BYTES {
            return Err(ApiError::bad_request());
        }

        Ok(Self {
            value,
            crypto_version: request.crypto_version,
            key_id,
        })
    }
}

async fn require_vault_session(
    pool: &sqlx::PgPool,
    headers: &HeaderMap,
) -> Result<CurrentSession, ApiError> {
    let now = now_utc_second()?;
    let session = load_current_session(pool, headers, now)
        .await?
        .ok_or_else(ApiError::session_required)?;
    if !session.vault_access() {
        return Err(ApiError::mfa_required());
    }
    refresh_session_activity(pool, session, now).await
}

fn validate_nonnegative_seq(value: i64) -> Result<i64, ApiError> {
    if value >= 0 {
        Ok(value)
    } else {
        Err(ApiError::bad_request())
    }
}

fn validate_positive_seq(value: i64) -> Result<i64, ApiError> {
    if value > 0 {
        Ok(value)
    } else {
        Err(ApiError::bad_request())
    }
}

fn decode_hash(value: &str) -> Result<[u8; HASH_BYTES], ApiError> {
    decode_base64url_array::<HASH_BYTES>(value).map_err(|_| ApiError::bad_request())
}

fn validate_short_text(value: &str, max_bytes: usize) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max_bytes || trimmed.chars().any(char::is_control) {
        return Err(ApiError::bad_request());
    }
    Ok(trimmed.to_string())
}

async fn load_vault_head(
    pool: &sqlx::PgPool,
    account_id: Uuid,
    vault_id: Uuid,
) -> Result<Option<VaultHead>, ApiError> {
    let row = sqlx::query(
        "
        SELECT head_seq, head_hash, genesis_head_hash
        FROM vaults
        WHERE id = $1
          AND account_id = $2
        ",
    )
    .bind(vault_id)
    .bind(account_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    row.map(vault_head_from_row).transpose()
}

async fn lock_vault_head(
    transaction: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    vault_id: Uuid,
) -> Result<Option<VaultHead>, ApiError> {
    let row = sqlx::query(
        "
        SELECT head_seq, head_hash, genesis_head_hash
        FROM vaults
        WHERE id = $1
          AND account_id = $2
        FOR UPDATE
        ",
    )
    .bind(vault_id)
    .bind(account_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    row.map(vault_head_from_row).transpose()
}

fn vault_head_from_row(row: sqlx::postgres::PgRow) -> Result<VaultHead, ApiError> {
    Ok(VaultHead {
        head_seq: row
            .try_get::<i64, _>("head_seq")
            .map_err(|_| ApiError::service_unavailable())?,
        head_hash: row
            .try_get::<Vec<u8>, _>("head_hash")
            .map_err(|_| ApiError::service_unavailable())?,
        genesis_head_hash: row
            .try_get::<Vec<u8>, _>("genesis_head_hash")
            .map_err(|_| ApiError::service_unavailable())?,
    })
}

async fn sync_cursor_matches(
    pool: &sqlx::PgPool,
    vault_id: Uuid,
    vault: &VaultHead,
    from_head_seq: i64,
    from_head_hash: &[u8; HASH_BYTES],
) -> Result<bool, ApiError> {
    if from_head_seq > vault.head_seq {
        return Ok(false);
    }
    if from_head_seq == vault.head_seq {
        return Ok(vault.head_hash.as_slice() == from_head_hash);
    }
    if from_head_seq == 0 {
        return Ok(vault.genesis_head_hash.as_slice() == from_head_hash);
    }

    let stored_head_hash = sqlx::query_scalar::<_, Vec<u8>>(
        "
        SELECT head_hash
        FROM vault_item_revisions
        WHERE vault_id = $1
          AND head_seq = $2
        ",
    )
    .bind(vault_id)
    .bind(from_head_seq)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    Ok(stored_head_hash
        .as_deref()
        .is_some_and(|hash| hash == from_head_hash))
}

fn head_matches(vault: &VaultHead, base_head_seq: i64, base_head_hash: &[u8; HASH_BYTES]) -> bool {
    vault.head_seq == base_head_seq && vault.head_hash.as_slice() == base_head_hash
}

fn vault_conflict_response(vault: &VaultHead) -> Response {
    no_store_json(
        StatusCode::CONFLICT,
        VaultConflictResponse {
            error: VaultConflictError {
                code: "vault_conflict",
                message: "Vault state conflict.",
            },
            current_head: HeadResponse {
                seq: vault.head_seq,
                hash: encode_base64url(&vault.head_hash),
            },
        },
    )
}

enum VaultWriteError {
    Conflict,
    Api(ApiError),
}

async fn insert_item_row(
    transaction: &mut Transaction<'_, Postgres>,
    vault_id: Uuid,
    item_id: Uuid,
) -> Result<(), VaultWriteError> {
    sqlx::query(
        "
        INSERT INTO vault_items (id, vault_id)
        VALUES ($1, $2)
        ",
    )
    .bind(item_id)
    .bind(vault_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            VaultWriteError::Conflict
        } else {
            VaultWriteError::Api(ApiError::service_unavailable())
        }
    })?;
    Ok(())
}

struct InsertRevisionRow<'a> {
    revision_id: Uuid,
    vault_id: Uuid,
    item_id: Uuid,
    operation: &'static str,
    revision_seq: i64,
    base_revision_seq: i64,
    head_seq: i64,
    base_head_seq: i64,
    base_head_hash: &'a [u8],
    previous_head_hash: &'a [u8],
    head_hash: &'a [u8],
    change_mac: &'a [u8],
    envelope_hash: &'a [u8],
    envelope: &'a ValidatedItemEnvelope,
}

async fn insert_revision_row(
    transaction: &mut Transaction<'_, Postgres>,
    input: InsertRevisionRow<'_>,
) -> Result<(), VaultWriteError> {
    sqlx::query(
        "
        INSERT INTO vault_item_revisions (
            id,
            vault_id,
            item_id,
            operation,
            revision_seq,
            base_revision_seq,
            head_seq,
            base_head_seq,
            base_head_hash,
            previous_head_hash,
            head_hash,
            change_mac,
            key_id,
            crypto_version,
            envelope_hash,
            encrypted_item_envelope
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8,
            $9, $10, $11, $12, $13, $14, $15, $16
        )
        ",
    )
    .bind(input.revision_id)
    .bind(input.vault_id)
    .bind(input.item_id)
    .bind(input.operation)
    .bind(input.revision_seq)
    .bind(input.base_revision_seq)
    .bind(input.head_seq)
    .bind(input.base_head_seq)
    .bind(input.base_head_hash)
    .bind(input.previous_head_hash)
    .bind(input.head_hash)
    .bind(input.change_mac)
    .bind(&input.envelope.key_id)
    .bind(&input.envelope.crypto_version)
    .bind(input.envelope_hash)
    .bind(SqlJson(input.envelope.value.clone()))
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            VaultWriteError::Conflict
        } else {
            VaultWriteError::Api(ApiError::service_unavailable())
        }
    })?;
    Ok(())
}

async fn update_item_latest(
    transaction: &mut Transaction<'_, Postgres>,
    vault_id: Uuid,
    item_id: Uuid,
    revision_id: Uuid,
    revision_seq: i64,
    deleted: bool,
    now: time::OffsetDateTime,
) -> Result<(), ApiError> {
    sqlx::query(
        "
        UPDATE vault_items
        SET latest_revision_id = $3,
            latest_revision_seq = $4,
            deleted_at = CASE WHEN $5 THEN $6 ELSE deleted_at END,
            updated_at = $6
        WHERE vault_id = $1
          AND id = $2
        ",
    )
    .bind(vault_id)
    .bind(item_id)
    .bind(revision_id)
    .bind(revision_seq)
    .bind(deleted)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;
    Ok(())
}

async fn update_vault_head(
    transaction: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    vault_id: Uuid,
    head_seq: i64,
    head_hash: &[u8],
    now: time::OffsetDateTime,
) -> Result<(), ApiError> {
    sqlx::query(
        "
        UPDATE vaults
        SET head_seq = $2,
            head_hash = $3,
            updated_at = $4
        WHERE id = $1
          AND account_id = $5
        ",
    )
    .bind(vault_id)
    .bind(head_seq)
    .bind(head_hash)
    .bind(now)
    .bind(account_id)
    .execute(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;
    Ok(())
}

struct LockedItem {
    latest_revision_seq: i64,
    deleted: bool,
}

async fn lock_item(
    transaction: &mut Transaction<'_, Postgres>,
    vault_id: Uuid,
    item_id: Uuid,
) -> Result<Option<LockedItem>, ApiError> {
    let row = sqlx::query(
        "
        SELECT latest_revision_seq, deleted_at
        FROM vault_items
        WHERE vault_id = $1
          AND id = $2
        FOR UPDATE
        ",
    )
    .bind(vault_id)
    .bind(item_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    row.map(|row| {
        Ok(LockedItem {
            latest_revision_seq: row
                .try_get::<i64, _>("latest_revision_seq")
                .map_err(|_| ApiError::service_unavailable())?,
            deleted: row
                .try_get::<Option<time::OffsetDateTime>, _>("deleted_at")
                .map_err(|_| ApiError::service_unavailable())?
                .is_some(),
        })
    })
    .transpose()
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
