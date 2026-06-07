CREATE TABLE accounts (
    id uuid PRIMARY KEY,
    login_handle_normalized text NOT NULL,
    display_name text,
    auth_protocol text NOT NULL,
    auth_migration_status text NOT NULL DEFAULT 'current',
    kdf_profile jsonb NOT NULL,
    account_salt bytea NOT NULL CONSTRAINT accounts_account_salt_len CHECK (octet_length(account_salt) = 32),
    auth_verifier_profile text NOT NULL,
    auth_verifier_salt bytea NOT NULL CONSTRAINT accounts_auth_verifier_salt_len CHECK (octet_length(auth_verifier_salt) = 32),
    auth_verifier_iterations integer NOT NULL CONSTRAINT accounts_auth_verifier_iterations_exact CHECK (auth_verifier_iterations = 150000),
    auth_stored_key bytea NOT NULL CONSTRAINT accounts_auth_stored_key_len CHECK (octet_length(auth_stored_key) = 32),
    auth_server_key bytea NOT NULL CONSTRAINT accounts_auth_server_key_len CHECK (octet_length(auth_server_key) = 32),
    opaque_credential_record bytea,
    failed_auth_count integer NOT NULL DEFAULT 0 CONSTRAINT accounts_failed_auth_count_nonnegative CHECK (failed_auth_count >= 0),
    locked_until timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT accounts_login_handle_nonempty CHECK (length(btrim(login_handle_normalized)) > 0),
    CONSTRAINT accounts_auth_protocol_check CHECK (auth_protocol IN ('derived-auth-v1', 'opaque-rfc9807-v1')),
    CONSTRAINT accounts_auth_verifier_profile_check CHECK (auth_verifier_profile IN ('pv-scram-sha-256-v1')),
    CONSTRAINT accounts_kdf_profile_check CHECK (
        kdf_profile = jsonb_build_object(
            'id', 'argon2id-browser-v1',
            'algorithm', 'argon2id',
            'memory_kib', 19456,
            'iterations', 2,
            'parallelism', 1
        )
    ),
    CONSTRAINT accounts_auth_migration_status_check CHECK (auth_migration_status IN ('current', 'migration_required', 'migration_in_progress'))
);

CREATE UNIQUE INDEX accounts_login_handle_normalized_uq
    ON accounts (login_handle_normalized);

CREATE TABLE devices (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    display_name text NOT NULL,
    user_agent_hash bytea CONSTRAINT devices_user_agent_hash_len CHECK (user_agent_hash IS NULL OR octet_length(user_agent_hash) = 32),
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz,
    revoked_at timestamptz,
    CONSTRAINT devices_display_name_nonempty CHECK (length(btrim(display_name)) > 0),
    CONSTRAINT devices_account_id_id_uq UNIQUE (account_id, id)
);

CREATE INDEX devices_account_id_idx
    ON devices (account_id);

CREATE TABLE auth_challenges (
    id uuid PRIMARY KEY,
    account_id uuid REFERENCES accounts (id) ON DELETE CASCADE,
    login_handle_normalized text NOT NULL,
    challenge_type text NOT NULL,
    auth_protocol text NOT NULL,
    server_nonce bytea NOT NULL CONSTRAINT auth_challenges_server_nonce_len CHECK (octet_length(server_nonce) = 32),
    public_metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    attempts integer NOT NULL DEFAULT 0 CONSTRAINT auth_challenges_attempts_nonnegative CHECK (attempts >= 0),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT auth_challenges_login_handle_nonempty CHECK (length(btrim(login_handle_normalized)) > 0),
    CONSTRAINT auth_challenges_type_check CHECK (challenge_type IN ('register', 'login', 'pre_mfa')),
    CONSTRAINT auth_challenges_auth_protocol_check CHECK (auth_protocol IN ('derived-auth-v1', 'opaque-rfc9807-v1'))
);

CREATE INDEX auth_challenges_account_id_idx
    ON auth_challenges (account_id);

CREATE INDEX auth_challenges_expires_at_idx
    ON auth_challenges (expires_at);

CREATE INDEX auth_challenges_handle_type_created_at_idx
    ON auth_challenges (login_handle_normalized, challenge_type, created_at DESC);

CREATE TABLE totp_factors (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    seed_ciphertext bytea NOT NULL CONSTRAINT totp_factors_seed_ciphertext_nonempty CHECK (octet_length(seed_ciphertext) > 0),
    seed_nonce bytea NOT NULL CONSTRAINT totp_factors_seed_nonce_nonempty CHECK (octet_length(seed_nonce) >= 12),
    seed_key_id text NOT NULL,
    seed_aead text NOT NULL,
    algorithm text NOT NULL DEFAULT 'SHA1',
    digits integer NOT NULL DEFAULT 6,
    period_seconds integer NOT NULL DEFAULT 30,
    last_accepted_step bigint,
    verified_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT totp_factors_account_id_uq UNIQUE (account_id),
    CONSTRAINT totp_factors_seed_key_id_nonempty CHECK (length(btrim(seed_key_id)) > 0),
    CONSTRAINT totp_factors_seed_aead_nonempty CHECK (length(btrim(seed_aead)) > 0),
    CONSTRAINT totp_factors_algorithm_check CHECK (algorithm IN ('SHA1', 'SHA256', 'SHA512')),
    CONSTRAINT totp_factors_digits_check CHECK (digits IN (6, 8)),
    CONSTRAINT totp_factors_period_seconds_positive CHECK (period_seconds > 0),
    CONSTRAINT totp_factors_last_accepted_step_nonnegative CHECK (last_accepted_step IS NULL OR last_accepted_step >= 0)
);

CREATE TABLE recovery_codes (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    code_salt bytea NOT NULL CONSTRAINT recovery_codes_code_salt_len CHECK (octet_length(code_salt) >= 16),
    code_hash bytea NOT NULL CONSTRAINT recovery_codes_code_hash_len CHECK (octet_length(code_hash) >= 32),
    created_at timestamptz NOT NULL DEFAULT now(),
    used_at timestamptz,
    CONSTRAINT recovery_codes_account_hash_uq UNIQUE (account_id, code_hash)
);

CREATE INDEX recovery_codes_account_id_idx
    ON recovery_codes (account_id);

CREATE TABLE sessions (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    device_id uuid,
    session_token_hash bytea NOT NULL CONSTRAINT sessions_session_token_hash_len CHECK (octet_length(session_token_hash) = 32),
    csrf_token_hash bytea CONSTRAINT sessions_csrf_token_hash_len CHECK (csrf_token_hash IS NULL OR octet_length(csrf_token_hash) = 32),
    session_state text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz,
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    CONSTRAINT sessions_account_device_fk FOREIGN KEY (account_id, device_id) REFERENCES devices (account_id, id) ON DELETE SET NULL (device_id),
    CONSTRAINT sessions_session_state_check CHECK (session_state IN ('mfa_enrollment_required', 'mfa_recovery', 'mfa_verified'))
);

CREATE UNIQUE INDEX sessions_session_token_hash_uq
    ON sessions (session_token_hash);

CREATE INDEX sessions_account_id_idx
    ON sessions (account_id);

CREATE INDEX sessions_expires_at_idx
    ON sessions (expires_at);

CREATE TABLE vaults (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    crypto_profile_id text NOT NULL,
    head_seq bigint NOT NULL DEFAULT 0,
    genesis_head_hash bytea NOT NULL CONSTRAINT vaults_genesis_head_hash_len CHECK (octet_length(genesis_head_hash) = 32),
    head_hash bytea NOT NULL CONSTRAINT vaults_head_hash_len CHECK (octet_length(head_hash) = 32),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT vaults_account_id_id_uq UNIQUE (account_id, id),
    CONSTRAINT vaults_crypto_profile_id_nonempty CHECK (length(btrim(crypto_profile_id)) > 0),
    CONSTRAINT vaults_head_seq_nonnegative CHECK (head_seq >= 0)
);

CREATE INDEX vaults_account_id_idx
    ON vaults (account_id);

CREATE TABLE vault_items (
    id uuid PRIMARY KEY,
    vault_id uuid NOT NULL REFERENCES vaults (id) ON DELETE CASCADE,
    latest_revision_id uuid,
    latest_revision_seq bigint NOT NULL DEFAULT 0,
    deleted_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT vault_items_vault_id_id_uq UNIQUE (vault_id, id),
    CONSTRAINT vault_items_latest_revision_seq_nonnegative CHECK (latest_revision_seq >= 0)
);

CREATE INDEX vault_items_vault_id_idx
    ON vault_items (vault_id);

CREATE TABLE vault_item_revisions (
    id uuid PRIMARY KEY,
    vault_id uuid NOT NULL,
    item_id uuid NOT NULL,
    operation text NOT NULL,
    revision_seq bigint NOT NULL,
    base_revision_seq bigint NOT NULL,
    head_seq bigint NOT NULL,
    base_head_seq bigint NOT NULL,
    base_head_hash bytea NOT NULL CONSTRAINT vault_item_revisions_base_head_hash_len CHECK (octet_length(base_head_hash) = 32),
    previous_head_hash bytea NOT NULL CONSTRAINT vault_item_revisions_previous_head_hash_len CHECK (octet_length(previous_head_hash) = 32),
    head_hash bytea NOT NULL CONSTRAINT vault_item_revisions_head_hash_len CHECK (octet_length(head_hash) = 32),
    change_mac bytea NOT NULL CONSTRAINT vault_item_revisions_change_mac_len CHECK (octet_length(change_mac) = 32),
    key_id text NOT NULL,
    crypto_version text NOT NULL,
    envelope_hash bytea NOT NULL CONSTRAINT vault_item_revisions_envelope_hash_len CHECK (octet_length(envelope_hash) = 32),
    encrypted_item_envelope jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT vault_item_revisions_item_fk FOREIGN KEY (vault_id, item_id) REFERENCES vault_items (vault_id, id) ON DELETE CASCADE,
    CONSTRAINT vault_item_revisions_vault_item_id_uq UNIQUE (vault_id, item_id, id),
    CONSTRAINT vault_item_revisions_vault_head_seq_uq UNIQUE (vault_id, head_seq),
    CONSTRAINT vault_item_revisions_item_revision_seq_uq UNIQUE (vault_id, item_id, revision_seq),
    CONSTRAINT vault_item_revisions_operation_check CHECK (operation IN ('create', 'update', 'delete')),
    CONSTRAINT vault_item_revisions_revision_seq_positive CHECK (revision_seq > 0),
    CONSTRAINT vault_item_revisions_base_revision_seq_nonnegative CHECK (base_revision_seq >= 0),
    CONSTRAINT vault_item_revisions_head_seq_positive CHECK (head_seq > 0),
    CONSTRAINT vault_item_revisions_base_head_seq_nonnegative CHECK (base_head_seq >= 0),
    CONSTRAINT vault_item_revisions_head_seq_advances_check CHECK (head_seq = base_head_seq + 1),
    CONSTRAINT vault_item_revisions_base_hash_matches_previous_check CHECK (base_head_hash = previous_head_hash),
    CONSTRAINT vault_item_revisions_key_id_nonempty CHECK (length(btrim(key_id)) > 0),
    CONSTRAINT vault_item_revisions_crypto_version_nonempty CHECK (length(btrim(crypto_version)) > 0),
    CONSTRAINT vault_item_revisions_revision_shape_check CHECK (
        (operation = 'create' AND revision_seq = 1 AND base_revision_seq = 0)
        OR
        (operation IN ('update', 'delete') AND revision_seq > base_revision_seq AND base_revision_seq > 0)
    )
);

CREATE INDEX vault_item_revisions_item_idx
    ON vault_item_revisions (vault_id, item_id);

ALTER TABLE vault_items
    ADD CONSTRAINT vault_items_latest_revision_fk
    FOREIGN KEY (vault_id, id, latest_revision_id) REFERENCES vault_item_revisions (vault_id, item_id, id)
    ON DELETE SET NULL (latest_revision_id);

CREATE TABLE audit_events (
    id bigserial PRIMARY KEY,
    account_id uuid REFERENCES accounts (id) ON DELETE SET NULL,
    actor_device_id uuid,
    event_type text NOT NULL,
    event_metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT audit_events_event_type_nonempty CHECK (length(btrim(event_type)) > 0)
);

CREATE INDEX audit_events_account_created_at_idx
    ON audit_events (account_id, created_at DESC);

CREATE INDEX audit_events_event_type_created_at_idx
    ON audit_events (event_type, created_at DESC);
