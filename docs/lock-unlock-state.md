# Lock And Unlock State Model

Status: draft.

This document separates server authorization from vault decryption.

Implementation status, 2026-06-08:

- Browser-side in-memory unlock is implemented locally in the current branch.
- Unlock derives from the master password plus account secret key during registration/login.
- After MFA verification, the browser decrypts the encrypted vault key wrap and keeps the unwrapped
  vault key only in tab memory.
- The `Lock` action clears in-memory unlock material and hides decrypted item fields without
  revoking the server session.
- Refreshing the page still requires sign-in/unlock again because no persisted local wrapped key is
  implemented.

## States

```text
Logged out
  no server session
  no vault unlock material

Logged in, locked
  valid server session
  API access allowed by session and authorization
  vault item payloads cannot be decrypted locally

Logged in, unlocked
  valid server session
  local browser unlock material exists
  client can decrypt vault item payloads

Logged out after lock
  session revoked or expired
  unlock material cleared
```

## Rules

- Server session and unlock state are separate.
- Server must not store vault unlock material.
- Unlock material should prefer in-memory storage for the MVP.
- Auto-lock should clear local decrypt capability.
- Re-login should require MFA according to policy.
- Re-unlock during a valid session should not necessarily require TOTP.

## UX Implications

- Search is available only after unlock because searchable fields are encrypted.
- Refreshing the page may require unlock again if keys are memory-only.
- Users must understand that losing the master password/unlock secret can lose vault access.
- Recovery codes should be shown as account MFA recovery, not vault recovery.

## Open Decisions

- In-memory only vs persisted wrapped local key.
- Auto-lock timeout.
- Behavior on browser close, tab close, and reload.
- Whether "remember this device" is allowed in MVP.
- Whether multi-device enrollment is in MVP.
