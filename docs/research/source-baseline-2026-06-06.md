# Research Note: Source Baseline 2026-06-06

Status: draft.

## Why This Matters

`password-vault` is security-sensitive. Architecture and stack choices must be based on official or
primary sources before implementation begins.

## Sources Checked

### Password Manager Security Models

- 1Password Security Design.
- Bitwarden Security Whitepaper.
- KeePass/KDBX documentation.

### Authentication And Cryptography

- RFC 9807 OPAQUE.
- RFC 9106 Argon2.
- RFC 6238 TOTP.
- W3C WebCrypto.
- OWASP Cryptographic Storage Cheat Sheet.
- OWASP Key Management Cheat Sheet.
- OWASP Password Storage Cheat Sheet.

### Backend Stack

- Rust Book.
- Axum documentation.
- SQLx documentation.
- RustCrypto Argon2 documentation.
- `totp-rs` documentation as a candidate to review, not a selected dependency.

### Kubernetes And Data Platform

- CloudNativePG 1.29 architecture, scheduling, backup, recovery, and replication documentation.
- Kubernetes persistent volume and ingress documentation.
- Argo CD Application specification.

### GitHub Workflow

- GitHub Issues, Projects, labels, milestones.
- GitHub Actions secure use.
- GitHub Actions permissions.
- GitHub repository rulesets and branch protection.
- GitHub CODEOWNERS.

## Findings

- OPAQUE is now published as RFC 9807 and is a serious long-term auth candidate.
- WebCrypto does not settle Argon2id; browser Argon2id means reviewed WASM.
- PostgreSQL remains the recommended primary product database.
- CloudNativePG is the preferred first PostgreSQL operator candidate.
- CloudNativePG 1.29 supports quorum synchronous replication through
  `spec.postgresql.synchronous.method: any` and `number: 1`; `dataDurability` mode is a deployment
  decision.
- CloudNativePG backup design should prefer the CNPG-I/Barman Cloud Plugin direction for new work
  where the installed version supports it.
- Vault/OpenBao is useful for platform secrets but not as user-vault decrypt path.
- GitHub Project creation requires the GitHub CLI `project` scope, not only `read:project`; this was
  resolved on 2026-06-07 for the working public project.
- GitHub Projects are managed through the new Projects surface and ProjectV2 GraphQL model; the
  repository `projects` field exposed by `gh repo view` still maps to deprecated classic projects.

## Open Questions

- OPAQUE implementation maturity in Rust and browser clients.
- Whether the MVP should use derived-auth-key first and migrate auth later.
- Final KDF choice and parameters.
- Final AEAD and item payload format.
- Backup target.
- CloudNativePG `required` versus `preferred` synchronous data durability mode.
- Runtime secret-management path.

## Sources

- https://agilebits.github.io/security-design/
- https://bitwarden.com/help/bitwarden-security-white-paper/
- https://keepass.info/help/base/security.html
- https://keepass.info/help/kb/kdbx.html
- https://www.rfc-editor.org/info/rfc9807/
- https://www.rfc-editor.org/rfc/rfc9106.html
- https://www.rfc-editor.org/info/rfc6238/
- https://www.w3.org/TR/webcrypto/
- https://cheatsheetseries.owasp.org/cheatsheets/Cryptographic_Storage_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
- https://docs.rs/axum/latest/axum/
- https://docs.rs/sqlx/latest/sqlx/
- https://docs.rs/argon2/latest/argon2/
- https://docs.rs/totp-rs/latest/totp_rs/
- https://cloudnative-pg.io/docs/1.29/architecture/
- https://cloudnative-pg.io/docs/1.29/replication/
- https://cloudnative-pg.io/docs/1.29/scheduling/
- https://cloudnative-pg.io/docs/1.29/backup/
- https://cloudnative-pg.io/docs/1.29/recovery/
- https://kubernetes.io/docs/concepts/storage/persistent-volumes/
- https://kubernetes.io/docs/concepts/services-networking/ingress/
- https://argo-cd.readthedocs.io/en/release-3.0/user-guide/application-specification/
- https://argo-cd.readthedocs.io/en/stable/user-guide/auto_sync/
- https://docs.github.com/en/issues/planning-and-tracking-with-projects/learning-about-projects/about-projects
- https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects
- https://cli.github.com/manual/gh_project_create
