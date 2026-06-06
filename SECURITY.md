# Security Policy

Status: bootstrap draft.

`password-vault` is not production-ready. No security guarantees are made at this stage.

## Reporting Security Issues

For now, do not publish real vulnerabilities, secrets, private infrastructure details, or exploit
details in public issues.

Use a private communication channel with the repository owner until GitHub private vulnerability
reporting is enabled.

## Security Goals

- zero-knowledge vault contents
- end-to-end encryption for vault item data
- no server-side plaintext vault item storage
- no admin recovery path that can decrypt user vault data
- TOTP MFA for login
- WebAuthn/passkeys planned after TOTP
- secure CI/CD with no production secrets in public PR workflows
- GitOps-compatible deployment with human approval gates

## Non-Goals At Bootstrap

- production SaaS readiness
- compliance certification
- enterprise organization sharing
- browser extension security model
- mobile/desktop client security model

## Public Repository Safety

Do not include:

- real passwords or vault data
- TOTP seeds
- recovery codes
- master passwords
- private keys
- tokens
- kubeconfigs
- `.env` values
- private hostnames, IPs, or home-network details
- sensitive logs
