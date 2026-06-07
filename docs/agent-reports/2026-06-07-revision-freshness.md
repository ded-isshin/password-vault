# Session Report: Vault Revision Freshness

## Goal

Define the MVP mechanism for detecting stale or rolled-back encrypted vault state. Related issue:
#25.

## Active Context

- Active repository: `password-vault`
- Branch: `docs/25-revision-freshness`
- Out of scope: infrastructure changes, product code, runtime secrets, deployment

## Work Completed

- Added `docs/security/revision-freshness.md`.
- Updated `docs/sync-protocol.md` to use `head_seq`, `head_hash`, `change_mac`, and hash-chain
  verification.
- Updated `docs/security/crypto-design-draft.md` with `vault_integrity_key` and freshness rules.
- Updated `docs/api-contract.md` with required freshness fields and conflict behavior.
- Updated `README.md` with the new security note.

## Design Summary

The MVP uses a client-keyed per-vault hash chain, client-MACed changes, and local client
checkpoints.

- The backend stores the current vault head and ordered changes.
- The client derives `vault_integrity_key` after vault unlock.
- Each write includes the previous vault head, `change_mac`, and a new client-computed head hash.
- The backend enforces optimistic concurrency on `base_head_seq` and `base_head_hash`.
- The client verifies change MACs and the chain before trusting or decrypting sync results.

This detects rollback for clients that have already observed a newer checkpoint. It does not fully
solve first-sync or cross-device fork detection for a brand-new device; that is explicitly post-MVP.

## Subagents Used

Yes. A report-only security/crypto sync reviewer completed.

Accepted suggestions:

- Use a hash-bound sync cursor, not a sequence number alone.
- Add client-generated item and revision IDs before encryption.
- Add `change_mac` over client-controlled change fields.
- Treat deletion as an authenticated change, not only server metadata.
- Add tests for reordered, omitted, and metadata-modified changes.

Deferred suggestions:

- Device-signed vault head receipts.
- Cross-device head gossip.
- User-visible vault head fingerprint/export.
- Append-only transparency log.
- Signed server checkpoints.

## Claude Code Used?

Yes.

Purpose: independent security review of the revision freshness / rollback-resistance design.

Summary of output:

- The cryptographic core is sound and appropriate for an MVP.
- Blocking coherence issues remained: sync payload did not include all MAC inputs, sync docs and
  security note disagreed on payload fields, delete flow conflicted with authenticated deletion, and
  canonical encoding needed to be a hard prerequisite.

Accepted suggestions:

- Added all required `change_mac` verification inputs to sync payloads.
- Replaced bare `DELETE` with signed deletion revisions through the revision-write endpoint.
- Clarified authenticated deletion tombstones.
- Added canonical encoding as an implementation prerequisite.

Deferred suggestions:

- Full canonical binary format and test vectors. These are required before implementation, but not
  fully specified in this docs PR.

## Commands Run

- `sed -n ... docs/sync-protocol.md`
- `sed -n ... docs/security/crypto-design-draft.md`
- `sed -n ... docs/api-contract.md`
- `sed -n ... docs/threat-model.md`
- Web searches for NIST SP 800-38D, RFC 5116, RFC 6962, OWASP Cryptographic Storage, and OWASP Key
  Management
- `git switch -c docs/25-revision-freshness`
- `claude -p --permission-mode plan --tools "Read,Glob,Grep" --no-session-persistence --model opus --effort high ...`

## Files Changed

- `README.md`
- `docs/api-contract.md`
- `docs/security/crypto-design-draft.md`
- `docs/security/revision-freshness.md`
- `docs/sync-protocol.md`
- `docs/agent-reports/2026-06-07-revision-freshness.md`

## Validation

Tested:

- `git diff --check`
- Markdown files are non-empty.
- Public secret-pattern grep returned no findings.
- Consistency grep for old `vault_seq` / `state_hash` terms in updated normative files returned no
  findings after edits.

## Risks

- The design requires canonical encoding before implementation.
- New devices without trusted checkpoints can still be shown an older internally valid chain.
- Strong cross-device fork detection, gossip, and transparency logging are deferred.

## Approval Needed

No infrastructure approval is needed for this docs PR.
