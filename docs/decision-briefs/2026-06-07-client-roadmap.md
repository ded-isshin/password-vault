# Decision Brief: Client And Multi-Device Roadmap

Status: draft.

## Question

What does "multi-device" mean for the MVP if the first usable client is only the browser web app?

## Short Answer

The MVP client is the browser web app, but the protocol must be multi-device from day one.

That means the server data model, sync protocol, key wrapping, sessions, and audit model must allow
the same user to access the same encrypted vault from multiple browsers/devices using the same
approved login and unlock flow.

The Chrome extension and iOS client are post-MVP clients, not separate product lines.

The product is API-first: the browser web app is the first client of the versioned product API, not a
reason to bake browser-only assumptions into backend behavior.

## MVP Client Scope

MVP includes:

- browser web app;
- personal account;
- one personal vault;
- login and TOTP MFA;
- local vault unlock in browser;
- encrypted item create/update/delete;
- delta sync by cursor;
- optimistic conflict rejection;
- documented `/v1` API contracts for security-sensitive flows;
- multiple browser sessions/devices using the same account and vault.

MVP does not include:

- Chrome extension autofill;
- iOS app;
- offline-first conflict UI;
- device-to-device key transfer;
- organization sharing;
- passkeys/WebAuthn;
- browser extension native messaging;
- plugin marketplace.

## Why Multi-Device From Day One

If the first data model assumes a single device, later browser extension and mobile clients will
force a redesign. The MVP should therefore treat devices as future first-class actors even if it
ships only a browser client.

Recommended first-device model:

- user can log in from more than one browser;
- each browser derives the required unlock material from the user's password, account secret key,
  and KDF metadata;
- server stores encrypted vault key wraps, never plaintext vault keys;
- server stores sessions and audit events separately from vault unlock state;
- sync uses `change_seq` cursors and immutable item revisions.

## Future Chrome Extension

The Chrome extension should reuse:

- the same auth protocol;
- the same local unlock model;
- the same versioned sync API;
- the same encrypted payload format;
- the same item revision model.

The canonical initial API surface and contract-strength rule are documented in
[API Contract Draft](../api-contract.md).

Extension-specific work should be added later:

- autofill threat model;
- extension permissions review;
- content script isolation;
- phishing and origin-matching rules;
- browser store release/signing process;
- UX/design review with Claude Code.

## Future iOS Client

The iOS client should reuse the same crypto and sync protocol, but it will need a separate client
security design:

- local secure storage;
- biometric unlock policy;
- app-store release process;
- device loss/revocation behavior;
- offline cache behavior;
- crash/log redaction.

## Device Records

The MVP should include a soft `devices` table for audit and future extension. It is not a strong
authenticator by itself. It can track:

- device ID;
- client type;
- first seen and last seen timestamps;
- revoked timestamp;
- public device metadata safe for audit display;
- future device-specific key wraps if approved.

Strong device enrollment, trusted-device unlock, and device-specific key wraps are post-MVP unless
the auth/key hierarchy ADR explicitly pulls them into the first implementation.

## Design Constraint

Do not add a feature that requires the server to decrypt user vault items. Browser, extension, and
mobile clients must all preserve the same zero-knowledge user-vault boundary.
