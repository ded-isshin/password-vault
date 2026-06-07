# Research Note: Auth Login Protocol Options

Status: draft research note for ADR 0003. Do not implement product code from this note alone.

## Why This Matters

`password-vault` is intended to be a zero-knowledge password manager. Login must authenticate a user
to the server, but login must not give the server the material needed to decrypt vault items.

For this product, there are two separate states:

- account login: the server knows that a request is from an authenticated account;
- vault unlock: the client has local key material that can decrypt encrypted vault payloads.

Mixing these states is one of the easiest ways to break the product's security model.

## Options Compared

### Traditional Password Over TLS

The browser sends the user's password to the backend over TLS. The backend hashes the password and
checks it against a stored password hash.

This is normal for many web applications, but it is a poor fit for this product.

Benefits:

- simplest implementation;
- easy to test with common web auth patterns;
- mature backend libraries exist in every language.

Problems:

- the backend sees the user's unlock password;
- backend logs, traces, panic dumps, or debug tooling can accidentally expose password-equivalent
  material;
- the design weakens the zero-knowledge story even if item payload encryption is client-side;
- after server compromise, stored password hashes can become an offline guessing target.

Recommendation: reject for the public MVP. It is acceptable only for a throwaway internal prototype,
and even then it should not be merged into the public product direction.

### Derived-Auth-Key Flow

The client derives multiple values from the user's password locally. One derived value is used for
server authentication. Another derived value is used for vault unlock or key wrapping. The server
never receives the raw password.

A safe design must treat the client-derived auth value as password-equivalent. It must not be stored
raw, and it must not be replayable if the database is copied.

Benefits:

- practical MVP path;
- keeps the raw password off the backend;
- matches the broad direction used by mature password managers that separate authentication from
  vault encryption;
- can be designed so the auth layer can later be replaced by OPAQUE without replacing the vault
  payload format.

Problems:

- protocol details are subtle;
- a bad design can create a cheap offline guessing oracle after database compromise;
- browser Argon2id requires a reviewed WASM dependency because WebCrypto does not provide Argon2id;
- phishing resistance still depends on MFA/WebAuthn, not on derived-auth alone.

MVP recommendation: use this as the first implementation path, with strict ADR coverage and tests.

Minimum guardrails:

- one expensive password KDF per unlock/login attempt;
- keep high-entropy account secret key as an optional future hardening path, similar to 1Password's
  two-secret key derivation model;
- HKDF domain separation for auth and vault-unlock material;
- server stores only a slow server-side hash of the client-derived auth secret;
- server never stores raw client-derived auth material;
- vault keys are wrapped client-side;
- login success does not mean vault unlock success;
- every protocol field is versioned.

### OPAQUE / PAKE

OPAQUE is an augmented password-authenticated key exchange protocol published as RFC 9807. It lets
the client prove knowledge of a password without sending the password to the server and is designed
to reduce pre-computation risk after server compromise.

Benefits:

- strongest password-login direction among the compared options;
- hides the password from the server, including during registration;
- provides mutual authentication and forward secrecy in the protocol design;
- good long-term fit for a public zero-knowledge password manager.

Problems:

- implementation complexity is much higher;
- Rust and browser library maturity must be reviewed carefully;
- interoperability and test-vector coverage must be established before production use;
- it still does not solve malicious browser-delivered JavaScript;
- it authenticates the user but does not automatically define vault key wrapping, recovery, or
  multi-device key enrollment.

Long-term recommendation: keep OPAQUE as the preferred future login protocol, but do not block the
web MVP on OPAQUE unless library review shows a clearly mature browser and Rust path.

### WebAuthn / Passkeys

WebAuthn is a W3C browser API for public-key credentials. Passkeys are WebAuthn/FIDO2 credentials
that can be device-bound or synchronized across devices depending on authenticator and platform.

Benefits:

- phishing-resistant authentication when used correctly;
- no shared password secret is sent to the server;
- strong fit for second factor and future passwordless login;
- good UX for future mobile and browser-extension flows.

Problems:

- WebAuthn authenticates account access; it does not automatically decrypt vault item payloads;
- passkey login still needs a vault-unlock or key-unwrapping story;
- account recovery and device enrollment become more complex;
- attestation, resident keys, user verification, discoverable credentials, and synced passkeys need
  careful policy decisions.

Recommendation: not required for the first MVP, but design the account/device model so WebAuthn can
be added as MFA first and possibly as a login method later.

## Recommended Product Direction

MVP:

- derived-auth-key login;
- account secret key deferred as an optional hardening path pending final UX and recovery decision;
- TOTP as required MFA for the first public web MVP;
- opaque server sessions stored server-side;
- client-side vault unlock and encryption;
- multi-device-capable key wrapping and sync metadata from the beginning;
- no password-over-TLS login path.

Post-MVP:

- WebAuthn/passkeys as phishing-resistant MFA;
- OPAQUE as the preferred password-login replacement after library review;
- browser extension using the same auth/session/sync API;
- mobile clients using the same account, device, key-wrap, and sync model.

Long-term:

- WebAuthn/passkeys for passwordless account login where possible;
- device enrollment and recovery flows that never give the server user-vault decrypt capability;
- organization key hierarchy and member key wraps.

## Account Secret Key Direction

An account secret key is a high-entropy random secret created during registration and held by the
user's clients. It is not stored by the server in plaintext and is not a TOTP seed.

Why it helps:

- a stolen database is not enough to run normal password-only offline guessing;
- weak or reused user passwords are less damaging when the account secret key is still private;
- it matches a proven pattern used by 1Password-style two-secret key derivation.

Costs:

- new-device setup needs the account secret key;
- users must save an emergency kit or recovery material;
- web storage of the account secret key is a local-device risk;
- losing both existing devices and the account secret key can make vault data unrecoverable.

Current recommendation: do not require the account secret key for the first MVP. Keep it documented
as a future hardening option. If accepted later, the UX can start simple: generate it at
registration, show it once, require it for first login on a new browser, and allow a user-controlled
"remember this device" option only after local-storage risk is documented.

## Tests Required Before Implementation Is Trusted

- KDF known-answer tests.
- HKDF domain separation tests.
- Login protocol registration and authentication tests.
- Server storage test proving the raw client-derived auth secret is not persisted.
- Negative test that a copied database value is not directly replayable as a login secret.
- Wrong-password failure tests that do not leak which protocol step failed.
- Rate-limit and lockout/backoff tests.
- Session cookie tests: HttpOnly, Secure, SameSite, expiration, rotation.
- TOTP RFC 6238 test vectors.
- TOTP replay rejection for an already accepted time step.
- WebAuthn/passkey tests later: challenge freshness, origin/RP ID checks, signature counter or
  clone-detection policy where applicable.
- Backend negative test that stored item payloads cannot be decrypted by backend-only code.

## ADR Decisions To Record

- Which MVP auth protocol is accepted.
- Whether OPAQUE is deferred and what must be true before adopting it.
- Exact KDF algorithm, parameters, memory/time targets, and versioning.
- Exact HKDF labels and key hierarchy.
- Exact server auth verifier storage format.
- Whether TOTP is mandatory for MVP accounts.
- Whether account secret key / two-secret key derivation is accepted later as optional hardening.
- Whether WebAuthn is MFA-only first or also a future login method.
- Device enrollment model.
- Account recovery versus vault recovery boundary.
- Residual risk of browser-delivered JavaScript.

## Sources

- https://www.rfc-editor.org/info/rfc9807/
- https://pages.nist.gov/800-63-4/sp800-63b.html
- https://owasp.org/www-project-application-security-verification-standard/
- https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html
- https://www.w3.org/TR/webauthn-3/
- https://agilebits.github.io/security-design/key-security-features.html
- https://bitwarden.com/help/bitwarden-security-white-paper/
