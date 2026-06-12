# Web UI Design Direction

Date: 2026-06-07

Status: draft direction for the next frontend implementation slices.

## Goal

Define a practical browser UI direction for Password Vault that feels premium and trustworthy while
remaining original, security-first, API-first, and honest about the current MVP state.

The target quality bar is comparable to mature password-manager products, but the UI must not copy
1Password branding, protected trade dress, exact layouts, copy, colors, icons, or visual identity.

## Current State

The deployed browser preview currently serves static HTML, CSS, and JavaScript from the Rust API.
It can check health/readiness and start registration/login challenges. The backend also implements
registration finish, session inspection, CSRF token issuance, and logout.

The preview is useful, but it still behaves like a technical status page:

- It exposes raw protocol metadata such as salts, nonces, challenge IDs, and KDF profile values.
- It does not yet show a real account creation, MFA, unlock, or vault workflow.
- It has no authenticated app shell, vault list, item detail view, settings area, or lock state.
- It is light-only and has no durable component system.
- It cannot prove browser cookie persistence while the live preview remains plain HTTP, because the
  session cookie is intentionally `Secure`.

## Design Principles

- Trust is the product. Explain the zero-knowledge boundary at the moments where users make trust
  decisions, especially master password entry, unlock, MFA, and secret display.
- Do not expose protocol internals in product UI. Salts, nonces, verifier metadata, and challenge
  IDs belong in logs/tests/dev tools, not in the user surface.
- Keep each step clear: one primary action, one visible state, no dead ends.
- Treat locked, session-only, MFA-pending, and vault-unlocked states as distinct UI states.
- Use generic auth errors to avoid account enumeration and MFA-state leakage.
- Prefer restrained, dense, accessible UI over marketing-page composition.
- Keep the app API-first. Browser UI, future extension, mobile apps, and CLI clients must share the
  same `/v1` contracts.

## Visual System Direction

Use a dark-first interface with a first-class light theme. Keep the existing teal direction as a
starting accent family, but refine it into an original Password Vault palette instead of adopting
category-blue or copying another product.

Recommended system:

- Deep neutral background and calm elevated surfaces.
- One primary accent family, with success/warning/danger colors reserved for state.
- 6px, 10px, and 16px radius scale.
- 44px minimum interactive targets.
- Strong visible focus ring.
- Monospace only for codes, one-time secrets, and technical values that truly need it.
- Reduced-motion support from the first implementation slice.
- Original product mark; avoid generic shield/checkmark as the permanent identity.

## MVP Screens In Order

1. App shell and theme foundation.
   Build a responsive app shell, theme tokens, focus states, reduced-motion handling, and a small
   route/state model in static HTML/CSS/JS.

2. Register flow.
   Replace the raw challenge dump with product steps: login handle, master password, confirm
   password, strength/quality guidance, registration start, local crypto seam, registration finish,
   and transition to `mfa_enrollment_required`.

3. TOTP enrollment.
   Show QR/manual secret once, confirm a TOTP code, and explain that the secret is not re-shown.
   Add recovery-code UX when the backend endpoint exists.

4. Login and MFA.
   Implement handle/password challenge, proof submission, MFA code entry, generic errors, and
   `session_state` handling.

5. Vault unlock.
   Separate server session from local vault unlock. A session alone is not vault access until the
   browser unwraps the vault key locally.

6. Vault list and item browser.
   Add client-side search over decrypted in-memory items, loading/empty/error states, and encrypted
   sync boundaries.

7. Item editor.
   Add title, URL, username, password, notes, custom fields, copy/show/hide controls, and conflict
   handling for future revision conflicts.

8. Account and security settings.
   Add devices, sessions, TOTP/recovery settings, audit events, and logout.

## Static-First Implementation Plan

Stay with static HTML/CSS/JS for the next frontend slice. Use it to prove the design system and real
endpoint wiring before introducing a heavier framework.

Next static steps:

- Replace the current preview layout with an app shell and auth layout.
- Add design tokens for dark/light themes, spacing, radius, focus, and reduced motion.
- Add small reusable primitives: button, field, password field, status toast, stepper, modal/sheet,
  secret reveal, and lock badge.
- Add a route/state model for anonymous, challenge, deriving, finishing, MFA required, setup, and
  unlocked states.
- Wire implemented endpoints: `/v1/auth/register/start`, `/v1/auth/register/finish`,
  `/v1/auth/login/start`, `/v1/session`, `/v1/csrf`, and `/v1/auth/logout`.
- Add a small CSRF helper that fetches a token immediately before unsafe authenticated requests,
  because the current CSRF contract is single-slot rotate-on-fetch.
- Create a `crypto.js` seam with real function names and placeholder implementations until browser
  crypto is implemented.

Move to React/Vite only after auth, MFA, unlock, and vault CRUD state start to make vanilla DOM
coordination expensive. The design tokens and visual system should port without redesign.

## Security And Privacy UX Rules

- Never send master passwords, unwrapped account keys, unwrapped vault keys, or plaintext vault
  items to the server.
- Never put secrets in URLs, logs, local storage, screenshots, or telemetry.
- Keep decrypted vault data in memory only and clear it on lock, logout, and idle expiry.
- Clear copied secrets from the clipboard after a short timeout where browser APIs allow it, and
  clear in-memory decrypted state on lock and logout.
- Gate reveal, copy, fill, and export actions on local vault unlock; later production hardening
  should add local reauthentication for high-risk reveal/export actions.
- Use generic auth failure copy for login, MFA, and rate-limit cases.
- Display account-MFA TOTP seeds and recovery codes as one-time secrets.
- Keep account-MFA TOTP separate from any future in-vault site-TOTP feature. Site-TOTP seeds for
  third-party accounts must live only inside the client-encrypted vault item payload.
- Do not weaken the `Secure`, `HttpOnly`, `SameSite=Strict` session cookie contract for HTTP
  preview convenience.
- Surface the HTTP preview limitation clearly when a browser flow depends on `Secure` cookie
  persistence.

## Accessibility Requirements

- Full keyboard flow for every screen.
- Visible focus state on all controls.
- Real labels and `aria-describedby` for field errors.
- `aria-live` status region for async state changes.
- Focus trap and focus restoration for modals/sheets.
- `aria-pressed` on password show/hide controls.
- `autocomplete` values for username, current password, new password, and one-time code.
- Inputs at least 16px on mobile.
- Contrast targets: WCAG 2.2 AA for text and UI components.

## Claude Code Usage

Purpose: independent frontend/product design advisor.

Prompt/task given: review the current static preview, API contract, MVP plan, and repository rules;
propose a modern premium password-manager UI direction inspired by mature products but not copying
1Password branding, layout, copy, colors, or protected trade dress.

Summary of output:

- The current preview is useful but behaves like a technical status page.
- Raw protocol fields should be removed from the product UI.
- The next frontend slice should be static-first, with a real app shell, dark/light tokens, route
  state, component primitives, session/CSRF/logout wiring, and a browser crypto seam.
- The UI should distinguish server session, MFA state, and local vault unlock.
- React/Vite should wait until auth, MFA, unlock, and vault CRUD state justify a framework.

Accepted suggestions:

- Use a premium but original visual direction.
- Build dark/light theme tokens and accessibility requirements early.
- Remove protocol metadata dumps from the user-facing flow.
- Treat CSRF as a helper because tokens rotate on fetch.
- Document the HTTP preview limitation instead of weakening cookie security.

Deferred suggestions:

- Full React/Vite migration is deferred until browser crypto, MFA, unlock, and vault CRUD make
  vanilla DOM state management too expensive.
- Full brand identity work is deferred, but the placeholder shield/check mark should not become the
  permanent identity.

## Open Risks

- Browser crypto is the critical path for a real zero-knowledge register/login/unlock UX.
- HTTPS ingress is needed before browser session persistence can be fully validated.
- Vault conflict UX must be designed before item sync can be treated as production-ready.
- Master password change requires a reviewed key re-wrap flow before it can be offered safely.
- Compromised-password checks require a privacy review because external breach lookups can leak
  metadata unless they are opt-in, client-side, and privacy-preserving.
- Public screenshots and examples must use placeholder handles and no private host details.
