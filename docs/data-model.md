# Data Model Draft

Status: bootstrap implementation draft. The first PostgreSQL migration exists in
`migrations/202606070001_initial_schema.sql`, but auth, sync, and vault-item runtime flows are not
implemented yet.

This document records the product data boundaries and the first implemented schema direction. The
schema can still change while the auth, key-derivation, recovery, and sync implementation is built.

## Security Boundary

The data model must separate server-visible synchronization metadata from user-secret content.

Recommended MVP boundary:

| Data | Server visibility | Reason |
| --- | --- | --- |
| User ID and login handle | Plaintext | Needed for account lookup and auth. |
| Vault ID and membership IDs | Plaintext | Needed for authorization. |
| Item ID | Plaintext | Needed for sync and updates. |
| Item revision sequence | Plaintext | Needed for conflict detection and delta sync. |
| Per-vault change cursor | Plaintext | Needed for delta sync. |
| Timestamps | Plaintext | Needed for sync and audit. |
| Deleted/tombstone flag | Plaintext | Needed for delete sync. |
| Crypto version | Plaintext | Needed for migration handling. |
| Ciphertext size | Observable | Hard to hide in MVP. |
| Title, URL, username, password, notes, tags | Ciphertext | Avoid metadata leakage. |
| Custom fields | Ciphertext | Secret-bearing by default. |

Consequence: the server cannot search or sort by title, URL, username, or tags. Search must be
client-side after unlock.

## Implemented MVP Migration Tables

```text
accounts
  id
  login_handle_normalized
  auth_protocol
  auth_migration_status
  kdf_profile
  account_salt
  auth_verifier_profile
  auth_verifier_salt
  auth_verifier_iterations
  auth_stored_key
  auth_server_key
  opaque_credential_record
  failed_auth_count
  locked_until
  created_at
  updated_at

devices
  id
  account_id
  display_name
  user_agent_hash
  created_at
  last_seen_at
  revoked_at

auth_challenges
  id
  account_id
  login_handle_normalized
  challenge_type
  auth_protocol
  server_nonce
  public_metadata
  attempts
  expires_at
  consumed_at
  created_at

totp_factors
  id
  account_id
  seed_ciphertext
  seed_nonce
  seed_key_id
  seed_aead
  algorithm
  digits
  period_seconds
  last_accepted_step
  verified_at
  created_at
  updated_at

recovery_codes
  id
  account_id
  code_salt
  code_hash
  created_at
  used_at

sessions
  id
  account_id
  device_id
  session_token_hash
  csrf_token_hash
  session_state
  created_at
  last_seen_at
  expires_at
  revoked_at

vaults
  id
  account_id
  crypto_profile_id
  head_seq
  genesis_head_hash
  head_hash
  created_at
  updated_at

vault_items
  id
  vault_id
  latest_revision_id
  latest_revision_seq
  deleted_at
  created_at
  updated_at

vault_item_revisions
  id
  vault_id
  item_id
  operation
  revision_seq
  base_revision_seq
  head_seq
  base_head_seq
  base_head_hash
  previous_head_hash
  head_hash
  change_mac
  key_id
  crypto_version
  envelope_hash
  encrypted_item_envelope
  created_at

audit_events
  id
  account_id
  actor_device_id
  event_type
  event_metadata
  created_at
```

The migration deliberately does not include plaintext item columns such as `title`, `url`,
`username`, `password`, `notes`, or `tags`.

## Schema Guardrails Implemented

- `accounts.login_handle_normalized` is unique.
- `sessions.account_id` references `accounts(id)` with cascade delete so account removal revokes
  sessions.
- `sessions(account_id, device_id)` references `devices(account_id, id)` so a session cannot attach
  another account's device when a device is present.
- `sessions.session_state` stores whether the session can access vault APIs.
- `vault_items(vault_id, id)` is unique and item revisions reference that composite key.
- `vault_item_revisions` is append-only data with `operation` limited to `create`, `update`, and
  `delete`.
- `vault_item_revisions(vault_id, head_seq)` is unique for one ordered vault change stream.
- `vault_items.latest_revision_id` references a revision for the same `(vault_id, item_id)`.
- Hashes and MACs are constrained to expected byte lengths where the current protocol already
  defines those lengths.
- TOTP seeds are stored as ciphertext plus protection metadata, and recovery codes are stored as
  one-way salted verifiers.

The database still cannot validate client-side cryptographic correctness for `head_hash` or
`change_mac`; unlocked clients must verify those values, and backend transaction code must enforce
authorization and optimistic concurrency.

## Authorization Rules

- Every item operation must prove membership in the target vault.
- Every revision must belong to the same vault as its item.
- Cross-user and cross-vault access denial must be tested.
- A user can only use MFA and recovery records tied to the same account.
- Audit events must never include item plaintext, TOTP seeds, recovery codes, or vault keys.

## Open Decisions

- Whether `login_handle` is email, username, or both.
- Whether email verification is required before TOTP enrollment.
- Whether device records are soft audit records in MVP or full cryptographic enrollments.
- Whether `vault_members.wrapped_vault_key` exists in MVP or only after device/sharing design.
- Whether recovery-key wrapping is included from day one.
- TOTP seed protection: app-level encryption, Vault/OpenBao Transit, or another KMS path.
- Exact encrypted item envelope JSON shape and canonical encoding.
- Exact migration-job versus startup-migration deployment pattern.

## Device Direction

The MVP browser client must support multiple user devices at the protocol level. A user should be
able to log in and unlock the same personal vault from more than one browser session using the
approved auth and unlock flow.

The initial `devices` table is a soft device/audit record, not a phishing-resistant authenticator and
not a cryptographic enrollment by itself. It gives the product a place to attach sessions, user-facing
device labels, revocation state, future WebAuthn credentials, and future per-device key material.

Strong device enrollment, trusted-device unlock, and device-specific key wraps are post-MVP unless
the auth/key hierarchy ADR explicitly pulls them into the first implementation.
