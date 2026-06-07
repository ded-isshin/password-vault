ALTER TABLE devices
    ADD COLUMN client_type text NOT NULL DEFAULT 'browser',
    ADD COLUMN public_metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    ADD CONSTRAINT devices_client_type_check CHECK (client_type IN ('browser', 'browser-extension', 'ios', 'android', 'cli')),
    ADD CONSTRAINT devices_public_metadata_object_check CHECK (jsonb_typeof(public_metadata) = 'object');

ALTER TABLE sessions
    ADD COLUMN idle_expires_at timestamptz NOT NULL DEFAULT (now() + interval '30 minutes'),
    ADD COLUMN absolute_expires_at timestamptz NOT NULL DEFAULT (now() + interval '12 hours'),
    ADD CONSTRAINT sessions_idle_before_absolute_check CHECK (idle_expires_at <= absolute_expires_at);

CREATE TABLE account_keysets (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    crypto_version text NOT NULL,
    key_id text NOT NULL,
    nonce bytea NOT NULL CONSTRAINT account_keysets_nonce_len CHECK (octet_length(nonce) = 12),
    ciphertext bytea NOT NULL CONSTRAINT account_keysets_ciphertext_nonempty CHECK (octet_length(ciphertext) > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT account_keysets_crypto_version_check CHECK (crypto_version IN ('account-keyset-v1')),
    CONSTRAINT account_keysets_key_id_nonempty CHECK (length(btrim(key_id)) > 0),
    CONSTRAINT account_keysets_account_key_id_uq UNIQUE (account_id, key_id)
);

CREATE INDEX account_keysets_account_id_idx
    ON account_keysets (account_id);

CREATE TABLE vault_key_wraps (
    id uuid PRIMARY KEY,
    vault_id uuid NOT NULL,
    account_id uuid NOT NULL,
    key_id text NOT NULL,
    crypto_version text NOT NULL,
    nonce bytea NOT NULL CONSTRAINT vault_key_wraps_nonce_len CHECK (octet_length(nonce) = 12),
    ciphertext bytea NOT NULL CONSTRAINT vault_key_wraps_ciphertext_nonempty CHECK (octet_length(ciphertext) > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT vault_key_wraps_vault_account_fk FOREIGN KEY (account_id, vault_id) REFERENCES vaults (account_id, id) ON DELETE CASCADE,
    CONSTRAINT vault_key_wraps_account_key_fk FOREIGN KEY (account_id, key_id) REFERENCES account_keysets (account_id, key_id) ON DELETE CASCADE,
    CONSTRAINT vault_key_wraps_crypto_version_check CHECK (crypto_version IN ('vault-key-wrap-v1')),
    CONSTRAINT vault_key_wraps_key_id_nonempty CHECK (length(btrim(key_id)) > 0),
    CONSTRAINT vault_key_wraps_vault_account_key_uq UNIQUE (vault_id, account_id, key_id)
);

CREATE INDEX vault_key_wraps_account_id_idx
    ON vault_key_wraps (account_id);

CREATE INDEX vault_key_wraps_vault_id_idx
    ON vault_key_wraps (vault_id);
