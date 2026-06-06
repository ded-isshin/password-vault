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
item
  id
  latest_revision_id
  deleted_at

item_revision
  id
  item_id
  revision_seq
  change_seq
  crypto_version
  ciphertext
  created_at
```

`revision_seq` is per item. `change_seq` is per vault and powers delta sync.

## Create Flow

```text
client encrypts item payload
client POST /v1/vaults/{vault_id}/items
server checks vault membership
server assigns item_id, revision_seq=1, change_seq=N
server stores ciphertext
server returns item_id and cursor
```

## Update Flow

```text
client reads latest known revision_seq
client encrypts new payload
client PUT /v1/vaults/{vault_id}/items/{item_id}
  base_revision_seq=<known revision>
server checks vault membership
server rejects stale base revision with 409
server stores new immutable revision
server advances latest_revision_id and change_seq
```

## Delete Flow

Deletes should produce a tombstone so other devices can learn that an item was deleted.

```text
client DELETE /v1/vaults/{vault_id}/items/{item_id}
server checks vault membership
server sets deleted_at or writes encrypted deletion revision
server advances change_seq
```

## Delta Pull

```text
GET /v1/vaults/{vault_id}/changes?since=<cursor>

returns:
  cursor
  changes[]
    item_id
    revision_id
    revision_seq
    change_seq
    crypto_version
    ciphertext
    deleted
```

The client decrypts payloads locally after vault unlock.

## Conflict Handling

The MVP should use optimistic concurrency:

- client submits `base_revision_seq`
- server rejects stale writes with `409 Conflict`
- client can show both local draft and latest remote revision
- no server-side merge of ciphertext

## Tests Required

- User cannot sync another user's vault.
- User cannot update item outside their vault.
- Stale revision update returns conflict.
- Delete tombstone appears in delta stream.
- Wrong crypto version is rejected or preserved for migration handling.
- Audit log records operation type without secret values.
