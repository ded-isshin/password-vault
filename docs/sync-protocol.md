# Sync Protocol Draft

Status: draft.

This document defines the intended shape of encrypted item synchronization.

## Goals

- Keep item contents encrypted from the server.
- Let the client pull changes incrementally.
- Preserve item history through immutable revisions.
- Prevent silent overwrite of concurrent edits.
- Support future browser extension and desktop/mobile clients.

## Non-Goals

- No real-time collaborative editing in MVP.
- No server-side merge of encrypted content.
- No server-side search over encrypted payloads.
- No offline-first conflict UI in MVP.

## Revision Model

Each item has immutable revisions.

```text
vault
  id
  head_seq
  head_hash
  crypto_profile_id

item
  id
  latest_revision_id
  deleted_at

item_revision
  id
  item_id
  revision_seq
  head_seq
  previous_head_hash
  head_hash
  change_mac
  crypto_version
  ciphertext
  created_at
```

`revision_seq` is per item. `head_seq` is per vault and powers delta sync.

`head_hash` is a client-keyed hash-chain head. It lets an unlocked client detect rollback against
state it has already observed. See [Vault Revision Freshness And Rollback Resistance](security/revision-freshness.md).

The sync API returns a bounded number of changes per response. If `has_more` is true, the client
must continue from the returned `to_head` cursor instead of assuming it has reached the latest vault
head.

The client should generate `item_id` and `revision_id` before encryption so those identifiers can be
bound into AEAD associated data and `change_mac`.

## Create Flow

```text
client encrypts item payload
client POST /v1/vaults/{vault_id}/items
  base_head_seq=<known vault head seq>
  base_head_hash=<known vault head hash>
  new_head_hash=<client-computed head hash>
  change_mac=<client-computed change MAC>
server checks vault membership
server checks base vault head
server stores client-generated item_id, revision_seq=1, head_seq=N
server stores ciphertext
server returns item_id and cursor
```

## Update Flow

```text
client reads latest known revision_seq and vault head
client encrypts new payload
client POST /v1/vaults/{vault_id}/items/{item_id}/revisions
  base_revision_seq=<known revision>
  base_head_seq=<known vault head seq>
  base_head_hash=<known vault head hash>
  new_head_hash=<client-computed head hash>
  change_mac=<client-computed change MAC>
server checks vault membership
server rejects stale base revision with 409
server rejects stale vault head with 409
server stores new immutable revision
server advances latest_revision_id and head_seq
```

## Delete Flow

Deletes should produce a tombstone so other devices can learn that an item was deleted.

```text
client POST /v1/vaults/{vault_id}/items/{item_id}/revisions
  operation=delete
  base_revision_seq=<known revision>
  base_head_seq=<known vault head seq>
  base_head_hash=<known vault head hash>
  new_head_hash=<client-computed head hash>
  change_mac=<client-computed deletion MAC>
server checks vault membership
server rejects stale base revision with 409
server rejects stale vault head with 409
server stores authenticated deletion revision/tombstone
server advances head_seq
```

## Delta Pull

```text
GET /v1/vaults/{vault_id}/sync?since_seq=<head_seq>&since_head_hash=<head_hash>

returns:
  from_head
    head_seq
    head_hash
  to_head
    head_seq
    head_hash
  changes[]
    item_id
    revision_id
    revision_seq
    base_revision_seq
    head_seq
    base_head_seq
    base_head_hash
    previous_head_hash
    head_hash
    operation
    key_id
    envelope_hash
    change_mac
    crypto_version
    encrypted_item_envelope
    deleted
```

The client verifies the vault hash chain before trusting or decrypting payloads. The client decrypts
payloads locally after vault unlock.

## Conflict Handling

The MVP should use optimistic concurrency:

- client submits `base_revision_seq`
- client submits `base_head_seq` and `base_head_hash`
- server rejects stale writes with `409 Conflict`
- server rejects sync cursors that do not match the stored chain at `since_seq`
- client can show both local draft and latest remote revision
- no server-side merge of ciphertext

## Freshness And Rollback Handling

The server can store and enforce the current vault head, but only the unlocked client can verify
`head_hash` and `change_mac`.

The client must keep a local checkpoint for each vault it has trusted:

```text
vault_id
head_seq
head_hash
```

If a sync response does not extend the local checkpoint, the client treats it as possible rollback or
fork and refuses to trust the state without explicit user resolution.

MVP residual risk: a brand-new device with no trusted checkpoint can still be shown an older but
internally valid chain. Strong cross-device freshness, device gossip, or transparency logging is
post-MVP.

The implementation must define canonical encoding and test vectors before this protocol is coded.
The encoding must cover `envelope_hash`, `change_mac`, and `head_hash`. It must not rely on
unspecified JSON object key ordering or runtime-specific serialization behavior.

## Tests Required

- User cannot sync another user's vault.
- User cannot update item outside their vault.
- Stale revision update returns conflict.
- Delete tombstone appears in delta stream.
- Delete tombstone is submitted as an authenticated deletion revision, not trusted as server-only
  metadata.
- Client rejects a lower `head_seq` than its local checkpoint.
- Client rejects a mismatched `head_hash` at an already accepted `head_seq`.
- Client rejects a changed `change_mac`, reordered change, omitted change, or modified operation
  metadata.
- Client rejects modified encrypted envelope metadata because the hash-chain check fails.
- Wrong crypto version is rejected or preserved for migration handling.
- Audit log records operation type without secret values.
