# Data Model Draft

Status: draft. This is not a migration plan.

This document records the first product data boundaries. The final schema depends on the auth,
key-derivation, recovery, and sync ADRs.

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

## Draft Tables

```text
users
  id
  login_handle
  auth_protocol_version
  auth_public_metadata
  created_at
  disabled_at

sessions
  id
  user_id
  created_at
  expires_at
  revoked_at
  user_agent_hash
  ip_hash

mfa_totp
  id
  user_id
  encrypted_seed
  seed_protection_version
  verified_at
  last_used_step
  created_at
  disabled_at

recovery_codes
  id
  user_id
  code_hash
  used_at
  created_at

vaults
  id
  owner_user_id
  vault_type
  created_at
  archived_at

vault_members
  id
  vault_id
  user_id
  role
  wrapped_vault_key
  wrapping_version
  created_at

items
  id
  vault_id
  latest_revision_id
  deleted_at
  created_at
  updated_at

item_revisions
  id
  item_id
  vault_id
  revision_seq
  change_seq
  crypto_version
  ciphertext
  associated_data_hash
  created_by_user_id
  created_at

audit_events
  id
  user_id
  vault_id
  event_type
  event_metadata
  created_at
```

## Authorization Rules

- Every item operation must prove membership in the target vault.
- Every revision must belong to the same vault as its item.
- Cross-user and cross-vault access denial must be tested.
- A user can only use MFA and recovery records tied to the same account.
- Audit events must never include item plaintext, TOTP seeds, recovery codes, or vault keys.

## Open Decisions

- Whether MVP supports more than one device.
- Whether `login_handle` is email, username, or both.
- Whether email verification is required before TOTP enrollment.
- Whether `vault_members.wrapped_vault_key` exists in MVP or only after device/sharing design.
- Whether recovery-key wrapping is included from day one.
- TOTP seed protection: app-level encryption, Vault/OpenBao Transit, or another KMS path.
