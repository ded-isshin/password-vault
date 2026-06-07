# Research Note: OPAQUE Browser And Rust Compatibility

Status: completed research spike. Date: 2026-06-07. Related issue: #24.

## Why This Matters

The MVP should avoid sending the user's master password to the backend. OPAQUE is the strongest
current candidate because it is an augmented password-authenticated key exchange protocol designed
for client-server authentication without exposing the password to the server.

The risk is practical rather than theoretical: if the Rust/browser libraries are immature, hard to
bundle, or hard to test, a rushed OPAQUE implementation could be worse than a simpler fallback with
honest limitations.

## Official Documentation Checked

- RFC 9807 OPAQUE: <https://www.rfc-editor.org/rfc/rfc9807.html>
- RFC 9106 Argon2: <https://www.ietf.org/rfc/rfc9106.html>
- `opaque-ke` docs.rs: <https://docs.rs/opaque-ke/latest/opaque_ke/>
- `@serenity-kit/opaque` npm metadata:
  <https://www.npmjs.com/package/@serenity-kit/opaque>
- `opaque-wasm` npm metadata: <https://www.npmjs.com/package/opaque-wasm>

## Current Behavior Relevant To Us

OPAQUE:

- RFC 9807 defines OPAQUE as an aPAKE with registration and online authenticated key exchange.
- The protocol lets the client authenticate without disclosing the password to the server.
- OPAQUE can derive an export key that can be used by an application to protect additional
  password-derived client material.

Rust server library candidate:

- `opaque-ke` current docs show version `4.0.1`.
- The crate states it is based on RFC 9807.
- The crate documents an `argon2` feature and warns that identity key stretching is only for quick
  tests/examples.
- The crate requires Rust 1.85 or higher.
- The crate documentation records a 2021 NCC Group audit of an older release, with fixes later
  incorporated.

Browser/client library candidates:

- `@serenity-kit/opaque` npm metadata reports version `1.1.0`, MIT license, repository
  `github.com/serenity-kit/opaque`, and latest publication metadata from 2026-02-01.
- Search results describe it as a JavaScript implementation based on `opaque-ke`.
- `opaque-wasm` npm metadata reports version `2.1.0`, last modified in 2023, and repository
  `github.com/marucjmar/opaque-wasm`.

Additional package evidence:

- `@serenity-kit/opaque` README states it is based on `opaque-ke`, links RFC 9807, documents browser
  and Vite examples, and reports a 7ASecurity penetration test / whitebox security review.
- `@serenity-kit/opaque` exposes an `exportKey` after registration/login and documents Argon2id key
  stretching profiles.
- Its README says the RFC-recommended Argon2 memory setting had to be reduced by one unit because
  the exact `2^21` memory value crashed in browser environments. This is a practical browser risk
  for MVP.
- `opaque-wasm` appears older and less compelling for the MVP primary path.

Local environment finding:

- `rustc` is not installed.
- `cargo` is not installed.
- Node and npm are installed.
- Docker is installed.
- Helm and Argo CD CLIs are not installed locally.

Container build finding:

- `rust:1.85-bookworm` and `rust:1.96-bookworm` work when `/usr/local/cargo/bin` is added to `PATH`.
- `cargo info opaque-ke@4.0.1` reports Rust `1.85`, MIT/Apache-2.0 license, default `ristretto255`
  and `serde` features, optional `argon2`, and repository `github.com/facebook/opaque-ke`.
- `cargo info opaque-ke` currently resolves to a pre-release latest line (`4.1.0-pre.*`). MVP code
  should pin the stable `4.0.1` line unless an ADR explicitly accepts a pre-release.

## Best Practices

- Treat RFC 9807 as the source of truth for protocol shape.
- Use a maintained library rather than hand-rolling OPAQUE.
- Require client and server test vectors or deterministic round-trip tests before accepting an
  implementation.
- Keep OPAQUE messages separate from vault-item encryption messages.
- Do not use OPAQUE alone as the vault encryption story. The browser still needs a key hierarchy for
  wrapping the vault data key and encrypting item payloads.
- Keep a fallback protocol behind the same `/v1` contract only if the OPAQUE spike fails.

## Security Considerations

- OPAQUE reduces server password exposure, but it does not solve browser-delivered JavaScript risk.
- A compromised web bundle can still steal the password or vault plaintext before cryptography runs.
- Browser WASM dependencies need supply-chain review and deterministic tests.
- Server-side OPAQUE credential records and OPRF/server key material are sensitive authentication
  assets.
- If the fallback path sends any password-derived auth secret to the backend, it must be documented
  as weaker than OPAQUE and designed for replacement.

## Recommendation

Keep OPAQUE as the preferred security direction, but do not make it the default MVP auth protocol.

The repository's earlier direction was derived-auth-key for MVP and OPAQUE later after review. This
spike does not provide enough evidence to reverse that default. It does provide enough evidence to
justify a time-boxed OPAQUE proof-of-concept before OPAQUE is selected for implementation.

Recommended #2 default:

- MVP default: `derived-auth-key + account secret key + server-side slow hash + TOTP`.
- OPAQUE: preferred future authentication protocol and optional pre-MVP upgrade only if a dedicated
  PoC passes.
- API shape: protocol-neutral `/v1/auth/register/start`, `/finish`, `/login/start`, `/finish`, with
  an explicit `auth_protocol` field.

The proof-of-concept should verify:

- Rust server library compiles with the selected MSRV.
- Browser package can run in Vite without unsafe bundler workarounds.
- Registration and login round-trip works between browser client and Rust server.
- Argon2 or selected key-stretching configuration is explicit.
- Export key or equivalent client material can be integrated with the vault key hierarchy.
- CI can run the tests without real secrets.
- Server setup secret handling and rotation/backup implications are understood.
- Browser registration/login performance and memory use are acceptable.

If the proof-of-concept fails, is not completed before implementation pressure rises, or remains
inconclusive, #2 should proceed with the derived-auth-key MVP default, label it weaker than OPAQUE,
and preserve a replacement path in the `/v1` contract.

## API Migration Advice

Keep auth protocol selection explicit:

```text
auth_protocol = "derived-auth-v1" | "opaque-rfc9807-v1"
```

Store:

- `auth_protocol_version`
- fallback verifier fields for `derived-auth-v1`
- nullable OPAQUE credential record for `opaque-rfc9807-v1`
- KDF profile
- migration status

Do not silently downgrade from OPAQUE to fallback. The client must see and verify the protocol version
being used.

Keep TOTP/session APIs stable across auth-protocol migration. Keep vault unlock and key wrapping
separate from auth so a future OPAQUE migration does not require re-encrypting vault items.

## What Not To Do

- Do not hand-roll OPAQUE.
- Do not silently downgrade to a derived-auth-secret protocol without an ADR.
- Do not implement auth before the local Rust toolchain/container build strategy is resolved.
- Do not commit any real test passwords, TOTP seeds, or key material.

## Open Questions

- Should the development environment install Rust locally, use a dev container, or run Rust builds
  only in Docker/GitHub Actions?
- Which browser OPAQUE package is acceptable after source review?
- Does the browser package expose enough API to control the KSF/Argon2 profile?
- Can `opaque-ke` and the browser package interoperate without patching?
- Should the OPAQUE export key wrap the vault data key directly, or should the vault key hierarchy
  use a separate Argon2id-derived wrapping key?

## Current Decision

Accepted for MVP planning: OPAQUE is not the MVP default. Derived-auth-key remains the MVP default
for #2 unless a separate PoC proves browser/Rust OPAQUE interop, performance, and operational
handling before auth implementation begins.

## Sources

- <https://www.rfc-editor.org/rfc/rfc9807.html>
- <https://www.ietf.org/rfc/rfc9106.html>
- <https://docs.rs/opaque-ke/latest/opaque_ke/>
- <https://www.npmjs.com/package/@serenity-kit/opaque>
- <https://www.npmjs.com/package/opaque-wasm>
