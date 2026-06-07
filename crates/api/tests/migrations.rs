use password_vault_api::db;

const EXPECTED_TABLES: &[&str] = &[
    "accounts",
    "devices",
    "auth_challenges",
    "totp_factors",
    "recovery_codes",
    "sessions",
    "account_keysets",
    "vaults",
    "vault_key_wraps",
    "vault_items",
    "vault_item_revisions",
    "audit_events",
];

const EXPECTED_CONSTRAINTS: &[&str] = &[
    "accounts_auth_protocol_check",
    "accounts_kdf_profile_check",
    "accounts_auth_verifier_profile_check",
    "accounts_auth_verifier_salt_len",
    "accounts_auth_verifier_iterations_exact",
    "accounts_auth_stored_key_len",
    "accounts_auth_server_key_len",
    "sessions_account_id_fkey",
    "sessions_session_token_hash_len",
    "sessions_session_state_check",
    "sessions_account_device_fk",
    "sessions_idle_before_absolute_check",
    "devices_client_type_check",
    "devices_public_metadata_object_check",
    "account_keysets_nonce_len",
    "account_keysets_ciphertext_nonempty",
    "account_keysets_crypto_version_check",
    "account_keysets_account_key_id_uq",
    "vault_key_wraps_nonce_len",
    "vault_key_wraps_ciphertext_nonempty",
    "vault_key_wraps_crypto_version_check",
    "vault_key_wraps_vault_account_fk",
    "vault_key_wraps_account_key_fk",
    "vault_key_wraps_vault_account_key_uq",
    "totp_factors_account_id_uq",
    "totp_factors_seed_ciphertext_nonempty",
    "totp_factors_seed_aead_nonempty",
    "recovery_codes_code_salt_len",
    "vaults_head_hash_len",
    "vault_item_revisions_item_fk",
    "vault_item_revisions_operation_check",
    "vault_item_revisions_revision_shape_check",
    "vault_item_revisions_head_seq_advances_check",
    "vault_item_revisions_base_hash_matches_previous_check",
    "vault_item_revisions_crypto_version_nonempty",
    "vault_item_revisions_vault_item_id_uq",
    "vault_item_revisions_vault_head_seq_uq",
    "vault_items_latest_revision_fk",
];

#[tokio::test]
async fn migrations_apply_and_create_expected_schema() {
    let Some(database_url) = std::env::var("PV_TEST_DATABASE_URL").ok() else {
        eprintln!("skipping migration test because PV_TEST_DATABASE_URL is not set");
        return;
    };

    let pool = db::connect(&database_url)
        .await
        .expect("test database must be reachable");

    db::run_migrations(&pool)
        .await
        .expect("migrations must apply cleanly");
    reset_test_data(&pool).await;

    let table_count = count_matching_names(
        &pool,
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name = ANY($1)",
        EXPECTED_TABLES,
    )
    .await;

    assert_eq!(table_count, EXPECTED_TABLES.len() as i64);

    let constraint_count = count_matching_names(
        &pool,
        "SELECT COUNT(*) FROM pg_constraint WHERE conname = ANY($1)",
        EXPECTED_CONSTRAINTS,
    )
    .await;

    assert_eq!(constraint_count, EXPECTED_CONSTRAINTS.len() as i64);

    assert_no_plaintext_item_columns(&pool).await;
    assert_duplicate_login_handle_is_rejected(&pool).await;
    assert_cross_account_session_device_link_is_rejected(&pool).await;
    assert_session_without_device_requires_existing_account(&pool).await;
    assert_registration_key_material_guards(&pool).await;
    assert_invalid_revision_shape_is_rejected(&pool).await;
    assert_cross_vault_revision_link_is_rejected(&pool).await;
}

async fn count_matching_names(pool: &sqlx::PgPool, query: &str, names: &[&str]) -> i64 {
    sqlx::query_scalar::<sqlx::Postgres, i64>(query)
        .bind(names)
        .fetch_one(pool)
        .await
        .expect("schema validation query must succeed")
}

async fn reset_test_data(pool: &sqlx::PgPool) {
    execute(
        pool,
        "
        TRUNCATE
            audit_events,
            sessions,
            recovery_codes,
            totp_factors,
            auth_challenges,
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
    .await;
}

async fn assert_no_plaintext_item_columns(pool: &sqlx::PgPool) {
    let forbidden_columns = ["title", "url", "username", "password", "notes", "tags"];
    let count = count_matching_names(
        pool,
        "
        SELECT COUNT(*)
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name IN ('vault_items', 'vault_item_revisions')
          AND column_name = ANY($1)
        ",
        &forbidden_columns,
    )
    .await;

    assert_eq!(count, 0);
}

async fn assert_duplicate_login_handle_is_rejected(pool: &sqlx::PgPool) {
    insert_account(pool, "00000000-0000-0000-0000-000000000001", "alice").await;

    assert_rejected(
        pool,
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
            '00000000-0000-0000-0000-000000000002',
            'alice',
            'derived-auth-v1',
            jsonb_build_object(
                'id', 'pbkdf2-sha256-browser-v1',
                'algorithm', 'PBKDF2-HMAC-SHA-256',
                'iterations', 600000,
                'hash', 'SHA-256'
            ),
            decode(repeat('aa', 32), 'hex'),
            'pv-scram-sha-256-v1',
            decode(repeat('bb', 32), 'hex'),
            150000,
            decode(repeat('cc', 32), 'hex'),
            decode(repeat('dd', 32), 'hex')
        )
        ",
    )
    .await;
}

async fn assert_cross_account_session_device_link_is_rejected(pool: &sqlx::PgPool) {
    insert_account(pool, "00000000-0000-0000-0000-000000000003", "bob").await;
    execute(
        pool,
        "
        INSERT INTO devices (id, account_id, display_name)
        VALUES (
            '00000000-0000-0000-0000-000000000101',
            '00000000-0000-0000-0000-000000000003',
            'Bob browser'
        )
        ",
    )
    .await;

    assert_rejected(
        pool,
        "
        INSERT INTO sessions (
            id,
            account_id,
            device_id,
            session_token_hash,
            session_state,
            expires_at
        ) VALUES (
            '00000000-0000-0000-0000-000000000201',
            '00000000-0000-0000-0000-000000000001',
            '00000000-0000-0000-0000-000000000101',
            decode(repeat('11', 32), 'hex'),
            'mfa_verified',
            now() + interval '1 hour'
        )
        ",
    )
    .await;
}

async fn assert_session_without_device_requires_existing_account(pool: &sqlx::PgPool) {
    assert_rejected(
        pool,
        "
        INSERT INTO sessions (
            id,
            account_id,
            device_id,
            session_token_hash,
            session_state,
            expires_at
        ) VALUES (
            '00000000-0000-0000-0000-000000000202',
            '00000000-0000-0000-0000-000000999999',
            NULL,
            decode(repeat('12', 32), 'hex'),
            'mfa_verified',
            now() + interval '1 hour'
        )
        ",
    )
    .await;
}

async fn assert_registration_key_material_guards(pool: &sqlx::PgPool) {
    insert_account(pool, "00000000-0000-0000-0000-000000000006", "carol").await;
    insert_account(pool, "00000000-0000-0000-0000-000000000007", "dave").await;
    insert_vault(
        pool,
        "00000000-0000-0000-0000-000000000306",
        "00000000-0000-0000-0000-000000000006",
    )
    .await;
    insert_account_keyset(
        pool,
        "00000000-0000-0000-0000-000000000701",
        "00000000-0000-0000-0000-000000000006",
        "carol-key-v1",
    )
    .await;
    insert_account_keyset(
        pool,
        "00000000-0000-0000-0000-000000000702",
        "00000000-0000-0000-0000-000000000007",
        "dave-key-v1",
    )
    .await;

    assert_rejected(
        pool,
        "
        INSERT INTO account_keysets (
            id,
            account_id,
            crypto_version,
            key_id,
            nonce,
            ciphertext
        ) VALUES (
            '00000000-0000-0000-0000-000000000703',
            '00000000-0000-0000-0000-000000000006',
            'account-keyset-v1',
            'bad-nonce',
            decode(repeat('01', 11), 'hex'),
            decode('02', 'hex')
        )
        ",
    )
    .await;

    assert_rejected(
        pool,
        "
        INSERT INTO account_keysets (
            id,
            account_id,
            crypto_version,
            key_id,
            nonce,
            ciphertext
        ) VALUES (
            '00000000-0000-0000-0000-000000000704',
            '00000000-0000-0000-0000-000000000006',
            'account-keyset-v1',
            'empty-ciphertext',
            decode(repeat('01', 12), 'hex'),
            decode('', 'hex')
        )
        ",
    )
    .await;

    assert_rejected(
        pool,
        "
        INSERT INTO vault_key_wraps (
            id,
            vault_id,
            account_id,
            key_id,
            crypto_version,
            nonce,
            ciphertext
        ) VALUES (
            '00000000-0000-0000-0000-000000000801',
            '00000000-0000-0000-0000-000000000306',
            '00000000-0000-0000-0000-000000000006',
            'carol-key-v1',
            'unsupported-wrap-v1',
            decode(repeat('03', 12), 'hex'),
            decode('04', 'hex')
        )
        ",
    )
    .await;

    assert_rejected(
        pool,
        "
        INSERT INTO vault_key_wraps (
            id,
            vault_id,
            account_id,
            key_id,
            crypto_version,
            nonce,
            ciphertext
        ) VALUES (
            '00000000-0000-0000-0000-000000000802',
            '00000000-0000-0000-0000-000000000306',
            '00000000-0000-0000-0000-000000000007',
            'dave-key-v1',
            'vault-key-wrap-v1',
            decode(repeat('03', 12), 'hex'),
            decode('04', 'hex')
        )
        ",
    )
    .await;

    assert_rejected(
        pool,
        "
        INSERT INTO devices (
            id,
            account_id,
            display_name,
            client_type,
            public_metadata
        ) VALUES (
            '00000000-0000-0000-0000-000000000901',
            '00000000-0000-0000-0000-000000000006',
            'Carol browser',
            'browser',
            '[]'::jsonb
        )
        ",
    )
    .await;

    execute(
        pool,
        "
        INSERT INTO devices (
            id,
            account_id,
            display_name,
            client_type,
            public_metadata
        ) VALUES (
            '00000000-0000-0000-0000-000000000902',
            '00000000-0000-0000-0000-000000000006',
            'Carol browser',
            'browser',
            '{}'::jsonb
        )
        ",
    )
    .await;

    assert_rejected(
        pool,
        "
        INSERT INTO sessions (
            id,
            account_id,
            device_id,
            session_token_hash,
            session_state,
            expires_at,
            idle_expires_at,
            absolute_expires_at
        ) VALUES (
            '00000000-0000-0000-0000-000000000903',
            '00000000-0000-0000-0000-000000000006',
            '00000000-0000-0000-0000-000000000902',
            decode(repeat('13', 32), 'hex'),
            'mfa_enrollment_required',
            now() + interval '30 minutes',
            now() + interval '12 hours',
            now() + interval '30 minutes'
        )
        ",
    )
    .await;
}

async fn assert_invalid_revision_shape_is_rejected(pool: &sqlx::PgPool) {
    insert_vault(
        pool,
        "00000000-0000-0000-0000-000000000301",
        "00000000-0000-0000-0000-000000000001",
    )
    .await;
    insert_item(
        pool,
        "00000000-0000-0000-0000-000000000401",
        "00000000-0000-0000-0000-000000000301",
    )
    .await;

    assert_rejected(
        pool,
        revision_insert_sql(
            "00000000-0000-0000-0000-000000000501",
            "00000000-0000-0000-0000-000000000301",
            "00000000-0000-0000-0000-000000000401",
            "rename",
        ),
    )
    .await;
}

async fn assert_cross_vault_revision_link_is_rejected(pool: &sqlx::PgPool) {
    insert_vault(
        pool,
        "00000000-0000-0000-0000-000000000302",
        "00000000-0000-0000-0000-000000000003",
    )
    .await;
    insert_item(
        pool,
        "00000000-0000-0000-0000-000000000402",
        "00000000-0000-0000-0000-000000000302",
    )
    .await;

    assert_rejected(
        pool,
        revision_insert_sql(
            "00000000-0000-0000-0000-000000000502",
            "00000000-0000-0000-0000-000000000301",
            "00000000-0000-0000-0000-000000000402",
            "create",
        ),
    )
    .await;
}

async fn insert_account(pool: &sqlx::PgPool, id: &str, login_handle: &str) {
    let sql = format!(
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
            '{id}',
            '{login_handle}',
            'derived-auth-v1',
            jsonb_build_object(
                'id', 'pbkdf2-sha256-browser-v1',
                'algorithm', 'PBKDF2-HMAC-SHA-256',
                'iterations', 600000,
                'hash', 'SHA-256'
            ),
            decode(repeat('aa', 32), 'hex'),
            'pv-scram-sha-256-v1',
            decode(repeat('bb', 32), 'hex'),
            150000,
            decode(repeat('cc', 32), 'hex'),
            decode(repeat('dd', 32), 'hex')
        )
        "
    );
    execute(pool, &sql).await;
}

async fn insert_vault(pool: &sqlx::PgPool, vault_id: &str, account_id: &str) {
    let sql = format!(
        "
        INSERT INTO vaults (
            id,
            account_id,
            crypto_profile_id,
            genesis_head_hash,
            head_hash
        ) VALUES (
            '{vault_id}',
            '{account_id}',
            'crypto-v1',
            decode(repeat('00', 32), 'hex'),
            decode(repeat('00', 32), 'hex')
        )
        "
    );
    execute(pool, &sql).await;
}

async fn insert_item(pool: &sqlx::PgPool, item_id: &str, vault_id: &str) {
    let sql = format!(
        "
        INSERT INTO vault_items (id, vault_id)
        VALUES ('{item_id}', '{vault_id}')
        "
    );
    execute(pool, &sql).await;
}

async fn insert_account_keyset(pool: &sqlx::PgPool, id: &str, account_id: &str, key_id: &str) {
    let sql = format!(
        "
        INSERT INTO account_keysets (
            id,
            account_id,
            crypto_version,
            key_id,
            nonce,
            ciphertext
        ) VALUES (
            '{id}',
            '{account_id}',
            'account-keyset-v1',
            '{key_id}',
            decode(repeat('01', 12), 'hex'),
            decode('02', 'hex')
        )
        "
    );
    execute(pool, &sql).await;
}

fn revision_insert_sql<'a>(
    revision_id: &'a str,
    vault_id: &'a str,
    item_id: &'a str,
    operation: &'a str,
) -> String {
    format!(
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
            '{revision_id}',
            '{vault_id}',
            '{item_id}',
            '{operation}',
            1,
            0,
            1,
            0,
            decode(repeat('00', 32), 'hex'),
            decode(repeat('00', 32), 'hex'),
            decode(repeat('01', 32), 'hex'),
            decode(repeat('02', 32), 'hex'),
            'key-v1',
            'item-envelope-v1',
            decode(repeat('03', 32), 'hex'),
            '{{\"ciphertext\":\"opaque\"}}'::jsonb
        )
        "
    )
}

async fn execute(pool: &sqlx::PgPool, sql: &str) {
    sqlx::query(sql)
        .execute(pool)
        .await
        .expect("test SQL must execute successfully");
}

async fn assert_rejected(pool: &sqlx::PgPool, sql: impl AsRef<str>) {
    assert!(
        sqlx::query(sql.as_ref()).execute(pool).await.is_err(),
        "test SQL should have been rejected"
    );
}
