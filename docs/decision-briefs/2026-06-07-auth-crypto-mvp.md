# Decision Brief: Auth And Crypto MVP

Status: draft.

Audience: Human architect, Codex orchestrator, implementation agents, reviewers.

## Question

Which authentication and cryptography direction should the MVP use for a public, Kubernetes-native,
zero-knowledge password manager?

## Short Answer

Use a derived-auth-key MVP protocol first. Keep OPAQUE as the preferred long-term authentication
upgrade after library maturity and browser interoperability are reviewed.

The MVP must not send the raw master password to the backend. The backend should receive only
authentication material derived on the client, and must store that value only behind a slow
server-side password hash. Vault item payloads remain encrypted client-side.

A 1Password-style account secret key is the recommended MVP baseline. It reduces the value of a
copied authentication database for normal password-only offline guessing. It also adds onboarding
and recovery complexity, so the implementation ADR must define that UX before code.

## Industry Baseline

Normal web applications commonly send a password over TLS and hash it on the server. That is not the
right target for this product because the server would see a password-equivalent secret and the
zero-knowledge boundary would be weaker.

Password-manager-style systems separate:

- account login;
- server session;
- vault unlock;
- vault key wrapping;
- item encryption;
- MFA.

OPAQUE is a strong protocol direction because it is an augmented password-authenticated key exchange
that hides the password from the server, including during registration. It is also more complex than
the MVP needs and requires careful library selection, protocol test vectors, and browser-client
integration.

## Recommended MVP Direction

Use this direction for the implementation design:

```text
user password
  + account secret key
  -> Argon2id(combined input, salt, params) -> master secret

master secret
  -> HKDF("password-vault/auth/v1") -> client auth secret
  -> HKDF("password-vault/unlock/v1") -> account unlock key

client auth secret
  -> sent to backend as password-equivalent auth material
  -> stored only as a slow server-side hash

account unlock key
  -> unwraps user key material

user key material
  -> unwraps vault key / root data key

vault key / root data key
  -> HKDF(vault_id, item_id, revision_id, key_epoch) -> item-revision content key

item-revision content key
  -> encrypts/decrypts exactly one item revision payload on the client
```

Important boundaries:

- `login` creates a server session;
- `unlock` enables local decryption in the browser;
- `TOTP` is login MFA only;
- `TOTP` is not a vault encryption key;
- server-side recovery codes recover login-factor access only, not vault contents.

## Why Not OPAQUE In MVP

OPAQUE remains attractive, but it is a protocol-level dependency with non-trivial implementation
risk. For the MVP, the project should avoid being blocked by:

- Rust and browser library maturity review;
- interoperability testing;
- protocol implementation mistakes;
- a more complex registration/login state machine.

The MVP key hierarchy should keep OPAQUE migration possible by keeping authentication separate from
vault key wrapping.

## Account Secret Key Direction

Use a high-entropy account secret key as the recommended second input to the browser KDF for the
MVP. The server must not store this secret in plaintext.

Reason: a copied authentication database should not be enough for normal password-only offline
guessing. This is a deliberate UX/security tradeoff similar to the two-secret direction used by
mature password managers.

Cost: new-device login requires the account secret key or a future approved recovery/enrollment
flow. The implementation ADR must define emergency-kit display, local storage policy, recovery
limits, and lost-secret behavior before code.

## Browser Crypto Constraints

WebCrypto does not provide Argon2id. If Argon2id is used in the browser, the project needs a pinned
WASM dependency, known-answer tests, dependency review, and supply-chain controls.

Initial Argon2id parameters should start from the OWASP minimum recommendation of 19 MiB memory, 2
iterations, and parallelism 1, then be tuned on representative browsers and devices.

PBKDF2 must not be a silent runtime fallback. It is allowed only as an explicitly approved prototype
or degraded-mode decision with a migration plan.

AES-GCM is the practical browser MVP AEAD because it is available in WebCrypto. The final crypto
spec should use per-revision content keys or define nonce generation, per-key encryption budget,
rekey triggers, associated data, and payload versioning before item encryption is implemented.

## MVP Hard Gates

Do not implement product auth/crypto code until the following artifacts exist:

- auth protocol message shapes;
- pre-login KDF metadata flow without account enumeration;
- crypto payload format;
- KDF parameters and calibration target;
- server-side hash algorithm and parameters;
- rate limits before expensive server-side auth-secret verification;
- TOTP window, replay protection, and throttling design;
- recovery-code lifecycle;
- browser Argon2id dependency review or explicit PBKDF2 degraded-mode decision;
- test vectors for KDF, HKDF separation, TOTP, and AEAD tamper rejection.

## Required Tests

- backend never receives the raw master password;
- raw client auth secret is not stored;
- login metadata lookup does not expose account existence through response shape;
- expensive server-side auth verification is protected by rate limits;
- wrong password cannot unwrap vault key material;
- backend cannot decrypt item ciphertext;
- cross-user and cross-vault access is denied;
- AES-GCM nonce uniqueness is enforced by tests;
- associated-data tampering fails decryption;
- TOTP RFC vectors pass;
- reused TOTP step is rejected;
- MFA recovery code cannot decrypt vault data.

## Accepted Residual Risk

A web MVP has a structural browser-delivered JavaScript risk: a compromised server or build pipeline
could ship malicious JavaScript. The MVP must document this honestly. Future browser extension,
desktop, and mobile clients can reduce this risk through reviewed release artifacts and app-store or
extension-store distribution, but they do not remove all supply-chain risk.

## Sources

- https://www.rfc-editor.org/rfc/rfc9807.html
- https://www.rfc-editor.org/rfc/rfc9106.html
- https://www.w3.org/TR/webcrypto/
- https://www.rfc-editor.org/info/rfc6238/
- https://agilebits.github.io/security-design/key-security-features.html
- https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html
