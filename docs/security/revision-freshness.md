# Vault Revision Freshness And Rollback Resistance

Status: draft. Related issue: #25.

## Problem

AEAD associated data can prove that ciphertext belongs to a specific vault, item, revision, algorithm,
and key epoch. It does not prove that the server returned the latest known vault state.

A malicious or stale backend can replay an older valid encrypted state. A restored database can also
serve an older valid state after disaster recovery. The client needs a way to detect rollback against
state it has already observed.

## MVP Decision

Use a client-keyed per-vault hash chain, client-MACed changes, and local client checkpoints.

The server stores the current `vault_head` and ordered change log, but it cannot create valid client
changes because it does not know the `vault_integrity_key`.

The client stores the last accepted per-vault checkpoint locally:

```text
vault_id
head_seq
head_hash
```

If a later sync returns a lower `head_seq`, a different `head_hash` for the same `head_seq`, or a
chain that does not extend the local checkpoint, the client must refuse to trust the returned state
until the user explicitly resolves the condition.

## Current Browser Implementation

Status: implemented for the static browser MVP.

The browser stores the per-vault checkpoint in origin-scoped `localStorage` under versioned
Password Vault keys. The implementation writes append-only per-head records plus a convenience
latest-pointer record; loading scans the append-only records so a stale tab cannot erase evidence of
a newer checkpoint by overwriting the pointer. Stored records contain only:

```text
version
vault_id
head_seq
head_hash
```

It does not store vault keys, account secret keys, TOTP seeds, recovery codes, cookies, item IDs,
item titles, URLs, usernames, passwords, notes, ciphertext, or decrypted item data.

On unlock, the client:

1. Requires local persistent storage to be available.
2. Loads and validates the stored checkpoint for the vault, if one exists.
3. Rejects immediately if the stored checkpoint is newer than the server-reported head.
4. Rejects immediately if the stored checkpoint has the same sequence but a different head hash.
5. Replays sync from the genesis head so item contents can be reconstructed after browser reload.
6. Verifies that the replayed chain reaches the stored checkpoint before trusting any newer suffix.
7. Persists the latest verified head only after sync succeeds.

For local writes, the client persists the new checkpoint after the server accepts the write, the
returned head matches the locally proposed head, and the client verifies its locally generated
change MAC and head hash. Checkpoint writes are monotonic for all tabs sharing the same browser
origin: the client refuses to overwrite a newer checkpoint or a different hash at the same sequence.
This is detection-and-fail-closed behavior for multi-tab races, not a cross-tab distributed lock.

This is intentionally fail-closed for the MVP. If the browser blocks `localStorage`, if the stored
checkpoint is malformed, or if the checkpoint cannot be written after a verified sync/write, the
client surfaces an error instead of silently accepting a downgrade to memory-only freshness.

Origin-scoped storage means a checkpoint written for one browser origin is not visible to another
origin. Moving between local preview URLs, LAN edge URLs, or future production domains starts as a
new-device/no-checkpoint case unless a future trusted checkpoint-transfer mechanism exists.

The checkpoint intentionally survives lock/logout for the browser origin. Removing it would remove
the cross-session rollback anchor.

## Keys

Derive a per-vault integrity key from unlocked vault key material:

```text
vault_integrity_key = HKDF(vault_key, "password-vault/vault-integrity/v1", vault_id)
```

The backend never receives this key.

## Identifiers

The client should generate `item_id` and `item_revision_id` before encryption.

This lets the client bind identifiers into:

- AEAD associated data;
- `change_mac`;
- the per-vault hash-chain link.

The server may validate uniqueness and authorization, but it should not need to assign IDs after the
client has encrypted the item payload.

## Genesis

Every vault has a genesis state:

```text
head_seq = 0
head_hash = HMAC-SHA-256(
  vault_integrity_key,
  canonical("password-vault/state/genesis/v1", vault_id, crypto_profile_id)
)
```

The client records this checkpoint when a vault is created or first trusted.

## Chain Link

Every write produces exactly one vault change and one new chain head:

```text
head_seq = previous_head_seq + 1

envelope_hash = SHA-256(canonical(encrypted_item_envelope))

client_change = canonical(
  "password-vault/client-change/v1",
  vault_id,
  operation,
  item_id,
  item_revision_id,
  item_revision_seq,
  base_item_revision_seq,
  base_head_seq,
  base_head_hash,
  key_id,
  crypto_version,
  envelope_hash,
  deleted
)

change_mac = HMAC-SHA-256(vault_integrity_key, client_change)

head_hash = SHA-256(
  canonical(
    "password-vault/state/head/v1",
    vault_id,
    head_seq,
    previous_head_hash,
    operation,
    item_id,
    item_revision_id,
    item_revision_seq,
    key_id,
    crypto_version,
    envelope_hash,
    change_mac,
    deleted
  )
)
```

The canonical encoding must be defined before implementation. JSON is acceptable only if canonical
serialization is specified and tested.

Implementation prerequisite: the first code PR for this design must define a typed canonical
encoding with field order, type tags, string/byte length-prefixing, integer encoding, and test
vectors. Ad hoc object serialization is not acceptable for `envelope_hash`, `change_mac`, or
`head_hash`.

Operation values for MVP:

- `create`
- `update`
- `delete`

Create uses `base_item_revision_seq = 0`. Update and delete use the latest known item revision
sequence as `base_item_revision_seq`.

`change_mac` authenticates client-controlled change data. `head_hash` binds that authenticated
change to the server-ordered append position.

## Write Flow

The client sends:

```text
base_head_seq
base_head_hash
new_head_hash
change_mac
encrypted_item_envelope
```

The server:

- verifies account and vault authorization;
- verifies `base_head_seq` and `base_head_hash` match the current stored vault head;
- stores the new immutable item revision and the new vault chain head;
- rejects stale writes with `409 Conflict`.

The server cannot verify that `new_head_hash` or `change_mac` are cryptographically correct, but it
can enforce single-head optimistic concurrency. Clients verify the MAC and hash chain during sync.

## Sync Flow

The client requests changes since its last known cursor:

```text
GET /v1/vaults/{vault_id}/sync?since_seq=<head_seq>&since_head_hash=<head_hash>
```

The response includes:

```text
vault_id
from_head { head_seq, head_hash }
to_head { head_seq, head_hash }
changes[]
```

Each change includes:

```text
head_seq
previous_head_hash
head_hash
operation
item_id
item_revision_id
item_revision_seq
base_item_revision_seq
base_head_seq
base_head_hash
key_id
crypto_version
envelope_hash
change_mac
deleted
encrypted_item_envelope
```

The client:

- checks `from_head` matches its local checkpoint when it has one;
- verifies every `change_mac` and chain link using `vault_integrity_key`;
- decrypts item envelopes only after chain validation;
- updates its local checkpoint only after successful validation.

## Delete Flow

Deletes must be authenticated changes.

The MVP deletion API is not a bare `DELETE` request. Deletion is a client-signed revision submitted
through the same revision-write path with `operation = delete`, `deleted = true`, and a valid
`change_mac`.

The server may expose `deleted=true` in sync metadata, but the client must trust deletion only when
the deletion change MAC and chain link verify. A server-only `deleted_at` flag is not enough for
client trust.

## What This Detects

Detected:

- stale sync response for a client that has a newer local checkpoint;
- server rewrite of encrypted envelopes or revision metadata;
- ciphertext swapped between items, revisions, vaults, or key epochs;
- server-forged create, update, or delete changes without the vault integrity key;
- server-accepted write based on a stale vault head;
- restore to an older database snapshot when the client has a newer checkpoint.

Not fully detected:

- a brand-new device with no local checkpoint can be shown an older but internally valid chain;
- a server can hide a suffix of changes from a client that never saw that suffix;
- a malicious server can fork different devices unless heads are compared out of band;
- a compromised browser bundle can erase or rewrite local checkpoints;
- a browser profile, user setting, private mode, or storage cleanup can remove the local checkpoint;
- a malicious server can deny service by refusing to return data;
- cross-device fork detection requires device gossip, signed checkpoints, or an external transparency
  mechanism, which is post-MVP.

## API Requirements

The API contract must include:

- `vault_head`
- `head_seq`
- `head_hash`
- `base_head_seq`
- `base_head_hash`
- `base_item_revision_seq`
- `new_head_hash`
- `change_mac`
- `409 stale_vault_head` for writes based on an old vault head;
- `409 stale_item_revision` for item updates based on an old item revision;
- `409 sync_fork_or_restore` when a supplied sync cursor does not match the stored chain;
- an explicit rollback/freshness error surfaced by the client.

## Test Requirements

- Client rejects a lower `head_seq` than its local checkpoint.
- Client rejects a different `head_hash` for a previously accepted `head_seq`.
- Client rejects a chain link whose `previous_head_hash` does not match.
- Client rejects a change whose `change_mac` is invalid.
- Client rejects reordered, omitted, or duplicated changes.
- Client rejects modified envelope metadata because `envelope_hash` changes.
- Backend returns `409 stale_vault_head` for stale `base_head_seq` or `base_head_hash`.
- Backend returns `409 stale_item_revision` for stale item revision updates.
- Sync with mismatched `{since_seq, since_head_hash}` returns `409 sync_fork_or_restore`.
- Delete tombstones advance `head_seq` and require a valid authenticated deletion change.
- Restored older fixture state is treated as rollback by a client with a newer checkpoint.

## Sources

- NIST SP 800-38D, GCM and GMAC:
  <https://csrc.nist.gov/pubs/sp/800/38/d/final>
- RFC 5116, authenticated encryption with associated data:
  <https://datatracker.ietf.org/doc/html/rfc5116>
- RFC 6962, append-only Merkle tree background:
  <https://www.ietf.org/rfc/rfc6962>
- WHATWG HTML Standard, Web Storage:
  <https://html.spec.whatwg.org/multipage/webstorage.html>
- MDN `Window.localStorage`:
  <https://developer.mozilla.org/en-US/docs/Web/API/Window/localStorage>
- MDN "Using the Web Storage API":
  <https://developer.mozilla.org/en-US/docs/Web/API/Web_Storage_API/Using_the_Web_Storage_API>
- OWASP Cryptographic Storage Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Cryptographic_Storage_Cheat_Sheet.html>
- OWASP Key Management Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html>
