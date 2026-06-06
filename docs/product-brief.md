# Product Brief

Status: bootstrap draft.

## Goal

Build a Kubernetes-native password manager for personal use first, with a future path to
organizations and sharing.

## Product Principles

- Zero-knowledge vault data.
- Client-side encryption by default.
- Server stores ciphertext and synchronization metadata.
- Kubernetes-native deployment.
- Public-facing deployment path, with private routing details kept out of the public product repo.
- GitHub is the control plane for issues, PRs, CI, docs, and release evidence.
- Argo CD is the future deployment controller.
- Documentation and threat modeling come before product code.

## Initial User Story

As a personal user, I can register, enable TOTP MFA, unlock my vault, create encrypted password
items, update them, and sync them through the service without the server being able to read my vault
item contents.

## MVP Acceptance Direction

- user can register
- user can log in
- user can enroll and verify TOTP
- user can recover from lost TOTP using recovery codes
- user can create encrypted vault items
- user can update encrypted vault items
- user can list item metadata allowed by the security model
- server never stores plaintext vault item contents
- cross-user and cross-vault access is denied and tested
- CI runs on GitHub-hosted runners
- deployment changes go through GitOps review

## Current Design Blockers

- login and key-derivation protocol
- client-side Argon2id via reviewed WASM vs WebCrypto-native KDF
- cryptographic item format
- browser-delivered JavaScript residual risk
- TOTP seed key custody
- off-node backup target
- GitHub Project creation is blocked until the active token has the full `project` scope
- single-device vs multi-device MVP
- plaintext metadata boundary
- recovery key vs account recovery codes
- item revision and delta-sync protocol

## Planned Post-MVP

- WebAuthn/passkeys
- zero-knowledge-compatible vault recovery key if not included in MVP
- KeePass/KDBX import
- organizations
- shared vaults
- browser extension
- mobile/desktop clients
- stronger platform secret management
- backup and restore automation
