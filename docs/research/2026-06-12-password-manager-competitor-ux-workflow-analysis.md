# Research Note: Password Manager Competitor UX And Workflow Analysis

Status: draft product/design research.

Date: 2026-06-12

## Why This Matters

Password Vault already has a strong technical direction: API-first, Kubernetes-native, client-side
encryption, MFA, sync, and observability. The current product risk is different: the browser
experience can still feel like an engineering console instead of a tool normal people trust with
their private life.

This note reviews mature password managers to identify the product workflows and human-centered UX
patterns that Password Vault should adopt, adapt, or explicitly defer.

## Scope

Reviewed products:

- 1Password
- Bitwarden
- Keeper
- Proton Pass
- Enpass
- RoboForm
- Apple Passwords / iCloud Keychain
- Google Password Manager
- LastPass
- Dashlane
- NordPass
- KeePassXC

Primary focus:

- first-run experience
- vault/item workflows
- browser extension and autofill workflows
- password generation and password-change workflows
- security health dashboards
- MFA, passkeys, and recovery UX
- sharing and organization models
- import/export and migration
- product copy and information architecture

Non-goals for this note:

- selecting a final brand identity
- copying competitor trade dress, layouts, copy, color systems, or icons
- changing cryptographic protocol decisions
- adding organization sharing to the MVP scope

## Source Method

The evidence below is based on official product documentation, official product pages, official
security pages, and official help-center content available on 2026-06-12.

Source limitations:

- Some Dashlane and NordPass support-center pages returned a Cloudflare challenge from this
  environment. For those products, this note uses accessible official security/product pages and
  avoids unsupported detailed claims about support workflows.
- Proton support URLs were inconsistent from this environment, so Proton-specific workflow evidence
  uses official Proton Pass product/security/alias/passkey pages.
- This is product/UX research, not a security audit of the competitors.

## Executive Findings

1. Mature password managers do not lead with cryptography in the main product path. They lead with
   concrete user jobs: save a login, fill a login, generate a better password, find unsafe
   accounts, recover safely, and share intentionally.
2. Browser extension and autofill are not side features. For mainstream users they are the product.
   A web-only MVP can ship first, but the architecture and UX must treat browser-extension flows as
   first-class.
3. Security dashboards work best when they translate risk into action. 1Password Watchtower,
   Bitwarden Vault Health, Keeper Security Audit, Enpass Audit, Google Password Checkup, Proton Pass
   Monitor, and RoboForm Security Center all frame issues as fixable user work.
4. Recovery UX must be explicit and calm. Recovery codes, emergency kits, account secret keys, and
   trusted contacts are easy to misunderstand. Password Vault must distinguish "recover MFA access"
   from "recover vault decryption" in every relevant screen.
5. Item creation should feel like creating a real-world record, not editing JSON. Popular managers
   support item categories, templates, custom fields, notes, cards, identities, SSH keys, passkeys,
   attachments, expiration dates, favorites, tags, and search.
6. Sharing and organizations should remain post-MVP, but the model must be designed early enough to
   avoid repainting the core vault/key architecture later.
7. Import is not required for the first working MVP, but it is essential to adoption. Users moving
   from KeePassXC, Bitwarden, 1Password, browsers, and CSV exports need a safe migration path.
8. The strongest design opportunity for Password Vault is a calmer, more human experience: fewer
   protocol details, clearer security language, guided first steps, better empty states, and
   security work presented as a short checklist instead of a technical dashboard.

## Priority Alignment

This research is a UX/product north star, not a replacement for the current stabilization queue.

Before Password Vault is safe for real secrets, the project still needs the operational gates already
tracked in the MVP plan: backup/PITR/restore evidence, failover evidence, Alertmanager delivery,
trusted TLS or documented trust path, and client-side reachability proof. The UX backlog below must
not compete with those gates. New issues should be opened only when they harden the existing MVP,
reduce user-facing security risk, or clarify docs that would otherwise mislead users.

## Product Patterns To Borrow

### 1. First-Run Checklist

Most products quickly move the user from account creation to the first useful action. For Password
Vault, the post-registration flow should become:

1. Create account.
2. Enroll TOTP.
3. Save recovery codes with explicit "MFA only, not vault decryption" language.
4. Unlock the first vault.
5. Create or save the first login.
6. Generate a stronger password.
7. See a small vault health baseline.

The screen should not expose salts, nonces, challenge IDs, raw KDF values, or server protocol
details. Those belong in developer diagnostics and tests.

### 2. Browser-First Mental Model

1Password, Bitwarden, Enpass, RoboForm, Apple, and Google all make save/fill/autofill central. The
extension is how users discover value while signing in to real websites.

Password Vault should keep the current browser web app as the MVP, but the next product architecture
documents should define a future extension boundary:

- extension uses the same `/v1` API contracts;
- extension never sends plaintext vault contents to the server;
- extension has an explicit threat model for content scripts, page DOM access, phishing, and
  autofill decisions;
- extension supports save, fill, search, generate, and password-change update flows;
- web app and extension share item schema and local crypto behavior.

### 3. Human Item Model

The MVP can start with login and secure note, but the product language should already use an item
model that can grow:

- Login
- Secure note
- Card
- Identity
- Recovery code
- Server/API credential
- Passkey
- SSH key
- Wi-Fi/network credential

Not every type needs MVP implementation. The important design decision is to avoid an interface that
feels like "encrypted blobs" to the user.

### 4. Security Health As Action Queue

Security health should be actionable and local where possible:

- weak passwords
- reused passwords
- compromised passwords
- logins missing TOTP where the website supports TOTP
- websites that support passkeys
- insecure `http://` login URLs
- duplicate items
- old or never-used items
- soon-expiring cards, documents, API keys, or custom date fields
- recovery setup incomplete

For the MVP, only implement checks that are truthful with the available data. It is better to show
three reliable checks than a fake enterprise-grade risk score.

### 5. Recovery Language

The product must repeatedly clarify:

- TOTP recovery codes restore the second login factor only.
- Recovery codes do not decrypt the vault.
- Losing the vault decryption secret may be unrecoverable unless a reviewed zero-knowledge recovery
  design exists.
- Future account recovery, emergency access, or trusted contacts require a separate key-wrapping
  design and threat model.

This should appear in account setup, recovery-code download/print flow, settings, and help docs.

### 6. Friendly Security Copy

Competitors use security pages for depth, but everyday screens stay concrete. Password Vault should
use short, human copy:

- "Your vault is locked on this device."
- "Save these recovery codes. They can restore MFA access, but they cannot decrypt your vault."
- "This item was saved locally, encrypted, and synced."
- "This password is reused in 3 saved logins."
- "This website supports two-factor authentication."

Avoid making users read a security whitepaper to understand ordinary actions.

### 7. Security UX Hygiene

The first browser MVP already has copy/reveal workflows. Competitor review and independent Claude
review both point to low-cost hardening that should be treated as security work, not polish:

- clear copied secrets from the clipboard after a short timeout where the browser allows it;
- clear clipboard and in-memory decrypted state on lock/logout where feasible;
- require local unlock, and later local reauthentication, before reveal/copy/fill actions;
- expose password/item history from existing immutable item revisions instead of adding a new
  storage model;
- define auto-lock, lock-on-idle, and "remember this device" behavior before real-user onboarding.

These are smaller and more immediately useful than speculative post-MVP feature research.

### 8. Master Password Change And Key Re-Wrap

Most mature password managers support changing the master password. In Password Vault, this is not a
simple account setting: it requires re-deriving the account unlock key and re-wrapping encrypted key
material without exposing plaintext vault keys to the server.

This is a real-user workflow that should be designed before production use. It should be handled as
an architecture/security design task before implementation, with test vectors and rollback behavior.

### 9. Site TOTP Versus Account MFA TOTP

Competitors often store verification codes inside vault items for third-party websites. Password
Vault already has account-login TOTP for MFA, where the seed is server-owned and protected by runtime
server-side keying.

Those are different systems:

- account MFA TOTP protects login to Password Vault;
- in-vault site TOTP would protect third-party accounts and must be stored only inside the
  client-encrypted vault item payload.

Future issues must not route site TOTP seeds through account-MFA custody or server-readable paths.

## Product-By-Product Findings

### 1Password

Observed official patterns:

- Browser extension onboarding emphasizes saving logins, filling logins, suggested passwords,
  passkeys, and provider sign-in without leaving the browser.
- Platform app guides expose item categories, search, custom fields, pinned fields, archive/delete,
  lock/unlock, biometrics, and sharing.
- Watchtower groups security problems into actionable categories: breaches, weak/reused passwords,
  unsecured websites, missing 2FA, passkeys available, expiring items, duplicates, and wrong-account
  placement.
- Secret Key and Emergency Kit UX is a strong example of explicit account safety language.

What to borrow:

- First-run should quickly reach save/fill/create actions.
- Security health should be an action queue, not a raw telemetry view.
- Recovery and account-secret concepts should be explained with calm warnings and durable artifacts.

What not to copy:

- 1Password visual identity, trade dress, copy, iconography, and exact layouts.

### Bitwarden

Observed official patterns:

- Web app getting-started docs guide users through creating folders, adding a login, generating a
  password, enabling two-step login, saving a recovery code, and later using premium health reports.
- Security documentation is explicit about end-to-end encryption, zero-knowledge posture, KDFs,
  recovery codes, emergency access, device login, passkeys, and organization sharing.
- Item model includes logins, cards, identities, secure notes, SSH keys, custom fields, folders,
  favorites, search, filters, import/export, and health reports.
- Collections are an organization sharing model, not just visual folders.

What to borrow:

- Simple web-vault onboarding.
- Separate "login" from "unlock" in product language.
- Keep import/export and organization sharing in the product roadmap even if deferred.

### Keeper

Observed official patterns:

- Sharing docs are detailed about record sharing, shared folders, one-time shares, time-limited
  access, self-destructing records, and permission levels.
- Security Audit presents a score and clear risk buckets: weak, reused, compromised, expired, and
  breach-related items.
- Import docs support migration from many password managers and file formats.
- Security guidance explains master password requirements, KDF posture, 2FA, security keys, and
  device/browser-extension risks.

What to borrow:

- Permission language for future sharing must be precise and visible.
- Security audit should be easy to scan and should link each issue to an item list.
- Import roadmap should include KeePass, Bitwarden, 1Password, browser CSV, and generic CSV.

### Proton Pass

Observed official patterns:

- Proton emphasizes privacy, end-to-end encryption, aliases, passkeys, integrated 2FA, and breach
  monitoring.
- Hide-my-email aliases are framed as identity protection, not merely an email feature.
- Pass Monitor combines password health, dark-web monitoring, inactive 2FA, and recommendations.

What to borrow:

- Privacy features can become a differentiator after the core vault is stable.
- Masked email/alias support should be researched as a post-MVP feature.
- Security health should include "privacy hygiene" later, not only password strength.

### Enpass

Observed official patterns:

- Browser extension docs make autofill, autosave, password generation, passkeys, and updating
  changed passwords core flows.
- Security audit home screen shows score, risk, compromised/identical/weak/expired passwords,
  breach monitoring, 2FA opportunities, and passkey opportunities.
- Security docs emphasize local encryption, SQLCipher, KDF parameters, device compromise warnings,
  and business recovery architecture.

What to borrow:

- Password-change flow should update the vault item as part of the browser workflow.
- Security dashboard should prioritize "what can I improve next?"

### RoboForm

Observed official patterns:

- Add-password docs frame three entry paths: save automatically after login/account creation, add
  manually, or import.
- Security Center checks compromised passwords against Have I Been Pwned.
- Business docs include dashboard, policy, master password, 2FA, hardware key/passkey, and user
  management flows.
- Password generator docs treat site-specific password requirements as a real UX problem.

What to borrow:

- Support multiple ways to add the first item.
- Password generator must handle site constraints without making the user fight the UI.

### Apple Passwords / iCloud Keychain

Observed official patterns:

- Apple presents passwords, passkeys, Wi-Fi passwords, and verification codes in one place.
- iCloud Keychain and AutoFill make cross-device usage central.
- Shared password groups are simple: trusted contacts, owner/member roles, shared credentials update
  for everyone.
- Verification-code setup is integrated into the password workflow.

What to borrow:

- Very simple language and minimal surfaces.
- Treat verification codes as part of the item, not as a separate technical subsystem.
- Shared-group UX can be simple even when the underlying key model is complex.

### Google Password Manager

Observed official patterns:

- Password Manager is embedded in Chrome and Google Account flows.
- Password Checkup exposes unsafe, compromised, and weak passwords and links to change flows.
- Import flow warns users to delete CSV exports after import.
- Share is constrained to family group members.
- Biometric authentication and OS screen-lock integration are used to protect reveal/fill actions.

What to borrow:

- Security checkup should be small and understandable.
- Import/export UX must warn about plaintext CSV risk.
- Reveal/copy/fill actions should be gated by local unlock/reauth where possible.

### LastPass

Observed official patterns:

- Official pages emphasize zero-knowledge encryption, browser extension usage, password health,
  breaches, MFA, emergency access, device sync, sharing, and secure notes.
- LastPass is also a cautionary product because the industry now treats breach history as part of
  user trust evaluation.

What to borrow:

- Emergency access and family sharing are valuable later.
- Security communication must be transparent and humble.

What not to copy:

- Do not make broad zero-knowledge or durability claims before Password Vault has implemented and
  tested the full browser crypto, backup, restore, and incident response posture.

### Dashlane

Observed official patterns:

- Accessible official security page emphasizes zero-knowledge password management, strong
  encryption, privacy, and a Trust Center.
- Detailed support-center workflow pages were not accessible from this environment due to a
  Cloudflare challenge.

What to borrow:

- Maintain a clear public security/trust page once the implementation is ready.
- Keep security claims tied to evidence, audits, and current behavior.

### NordPass

Observed official patterns:

- Accessible official security page emphasizes XChaCha20, OAuth 2.0 based authentication,
  asymmetric encryption for organization/recovery features, privacy-sensitive logging, and
  cross-device availability.
- Detailed support-center workflow pages were not accessible from this environment due to a
  Cloudflare challenge.

What to borrow:

- Be explicit about logs and privacy boundaries.
- Future organization recovery should use public-key based design rather than admin plaintext
  access.

### KeePassXC

Observed official patterns:

- KeePassXC is a local-first password database, not a SaaS vault, but its user guide is valuable for
  entry/group organization, browser integration, Auto-Type, TOTP, attachments, custom fields, and
  password generation.
- KeePass/KDBX import matters because many privacy-conscious users already trust local password
  databases.

What to borrow:

- Respect local-first mental models: export, backup, and user control matter.
- KDBX import should be a high-priority post-MVP migration feature.

## Recommended Password Vault UX Direction

### MVP Must Become More Human

Current technical MVP work is necessary, but the browser app should now shift toward:

- a polished locked/unlocked app shell;
- a first-run checklist;
- a non-technical register/login/MFA flow;
- one personal vault;
- create-first-login workflow;
- password generator;
- encrypted item list/detail/editor;
- local vault health starter;
- clear recovery-code UX;
- clear "session vs vault unlock" states;
- no raw protocol metadata in normal screens.

### API-First Product Boundary

Every UX feature should be mapped to stable API contracts:

- Web app: first implementation surface.
- Browser extension: next client surface, same API and item model.
- Mobile apps: later, same sync/key model.
- CLI: optional later for power users and automation.

The API should remain the product backbone. UI work should not invent hidden behavior outside `/v1`
contracts.

### Human-Centered Information Architecture

Recommended navigation:

- Vault
- Generator
- Health
- Settings
- Help

Recommended vault item sections:

- Favorites
- Logins
- Secure Notes
- Recently Updated
- Needs Attention
- Archive / Trash

Recommended settings sections:

- Account
- Security
- MFA and recovery
- Devices and sessions
- Import and export
- Privacy
- About and diagnostics

### Visual Direction

Password Vault should be inspired by the trust and polish of top-tier password managers but remain
original:

- avoid raw technical panels as the primary UI;
- use quiet, high-contrast typography;
- make primary actions obvious;
- use icon buttons for copy/reveal/generate/favorite/archive;
- make empty states useful and action-oriented;
- preserve full keyboard accessibility;
- support dark and light modes;
- avoid copying 1Password's color palette, layouts, lock imagery, or exact wording.

## Recommended GitHub Tickets

Create only the issues that pass the current execution-hygiene filter. Do not open a broad
"competitor parity" epic; it invites unbounded scope.

Open now:

1. `[Security]: Add clipboard auto-clear and reveal/copy reauth UX`:
   https://github.com/ded-isshin/password-vault/issues/130
2. `[ADR]: Design master password change and key re-wrap flow`:
   https://github.com/ded-isshin/password-vault/issues/131
3. `[UX]: Clarify MFA recovery versus vault decryption recovery in user-facing copy`:
   https://github.com/ded-isshin/password-vault/issues/132
4. `[Feature]: Add local Vault Health starter checks for implemented item data`:
   https://github.com/ded-isshin/password-vault/issues/133

Fold into existing/canonical docs rather than opening new issues:

- Guided first-run onboarding and create-first-login UX belong in the web UI direction and browser
  MVP follow-up work.
- Password generator UX belongs with the existing item editor/browser app work.
- Browser extension research belongs in the client roadmap and should be security-led.
- Import, sharing, organizations, aliases, mobile clients, passkeys, and expanded item templates
  remain post-MVP until stabilization gates close.

Revisit later:

- `[Research]: Browser extension autofill and autosave architecture`
- `[Research]: Import roadmap for KeePassXC, Bitwarden, 1Password, browsers, and CSV`
- `[Research]: Sharing, collections, trusted contacts, and organization key model`
- `[Research]: Masked email aliases and privacy-hygiene roadmap`
- `[Research]: Passkeys, integrated site TOTP, and security-key roadmap`

## Acceptance Criteria For Follow-Up Work

- Product screens stop exposing raw protocol fields to normal users.
- New UI issues reference concrete competitor patterns and official sources.
- MVP scope remains personal-use-first.
- Browser extension, import, sharing, aliases, organizations, and mobile clients stay out of MVP
  implementation unless explicitly re-prioritized.
- Any security health claim is backed by implemented checks.
- Any recovery claim distinguishes MFA recovery from vault decryption recovery.
- Design improvements remain original and avoid copying competitor protected expression.

## Claude Code Review

Independent review performed:

- Model: `claude-opus-4-8`
- Role: independent product architect and UX/security reviewer
- Scope: this research note, current product brief, feature map, MVP implementation plan, API-first
  direction, and existing web UI design direction
- Command mode: read-only Claude Code invocation with `Read`, `Grep`, `Glob`, and `LS` allowed;
  edit/write/bash tools disallowed

Summary of output:

- The research is useful, honest, and well-sourced, but it must be treated as a UX north star
  subordinate to the stabilization queue.
- The original 12-ticket list was too broad for the current execution-hygiene rules.
- Several proposed items duplicated existing docs or closed browser MVP work.
- The review identified three high-value gaps: clipboard/reveal security hygiene, master password
  change with key re-wrap, and password/item history based on existing immutable revisions.
- The review warned that in-vault site TOTP must not be confused with account-login MFA TOTP.
- The review warned that compromised-password checks imply third-party breach lookups and must be
  opt-in, privacy-preserving, and client-side if implemented.

Accepted suggestions:

- Add a priority-alignment warning so UX research does not compete with backup/TLS/alerting gates.
- Replace broad "competitor parity" issue creation with a short risk-reducing issue list.
- Add clipboard auto-clear, reveal/copy reauth, local unlock gating, auto-lock, and item history as
  security-UX patterns.
- Add master password change/key re-wrap as a missing architecture task.
- Add explicit separation between account MFA TOTP and future in-vault site TOTP.
- Keep import parsing/encryption client-side and treat plaintext CSV as dangerous.

Rejected or deferred suggestions:

- Do not create a large competitor-parity epic now.
- Do not open speculative research tickets for extension, sharing, aliases, passkeys, mobile, or
  import until stabilization gates are closed or the Human Architect re-prioritizes them.
- Do not ship 1Password-style Emergency Kit reassurance until Password Vault has its own reviewed
  recovery design.

Follow-up issue creation should follow the narrowed list above.

## Sources

- 1Password, Get to know 1Password in your browser:
  https://support.1password.com/getting-started-browser/
- 1Password, Watchtower:
  https://support.1password.com/watchtower/
- 1Password, Secret Key:
  https://support.1password.com/secret-key-security/
- Bitwarden, Security Whitepaper:
  https://bitwarden.com/help/bitwarden-security-white-paper/
- Bitwarden, Password Manager Web App:
  https://bitwarden.com/help/getting-started-webvault/
- Bitwarden, Managing Items:
  https://bitwarden.com/help/managing-items/
- Bitwarden, Collections:
  https://bitwarden.com/help/about-collections/
- Bitwarden, Emergency Access:
  https://bitwarden.com/help/emergency-access/
- Keeper, Sharing:
  https://docs.keeper.io/user-guides/sharing
- Keeper, Security Audit:
  https://docs.keeper.io/user-guides/security-audit
- Keeper, Import Overview:
  https://docs.keeper.io/user-guides/import-records-1/import-overview
- Keeper, Protecting Your Keeper Vault:
  https://docs.keeper.io/user-guides/tips-and-tricks/protecting-your-keeper-vault
- Proton Pass, Security:
  https://proton.me/pass/security
- Proton Pass, Aliases:
  https://proton.me/pass/aliases
- Proton Pass, Pass Monitor:
  https://proton.me/pass/pass-monitor
- Proton Pass, Passkeys:
  https://proton.me/pass/passkeys
- Enpass, Browser Extension:
  https://support.enpass.io/app/extension/using_enpass_browser_extension.htm
- Enpass, Security Audit Home:
  https://support.enpass.io/app/audit/enpass_home.htm
- Enpass, Changing Passwords With Browser Extension:
  https://support.enpass.io/app/generate/changing_passwords_using_browser_extension_in_desktop_app.htm
- Enpass, Data Security And Encryption:
  https://support.enpass.io/app/kb/data_security_and_encryption_in_enpass.htm
- RoboForm, Add Passwords:
  https://help.roboform.com/hc/en-us/articles/115005861708-How-to-add-passwords-to-RoboForm
- RoboForm, Compromised Passwords In Security Center:
  https://help.roboform.com/hc/en-us/articles/360060772192-Compromised-passwords-in-Security-Center
- RoboForm, Business Security Overview:
  https://help.roboform.com/hc/en-us/articles/115003926191-RoboForm-for-Business-Security-Overview
- RoboForm, Generate A Random Password:
  https://help.roboform.com/hc/en-us/articles/360043072811-How-to-generate-a-random-password
- Apple, Passwords app:
  https://support.apple.com/en-us/120758
- Apple, Passwords User Guide:
  https://support.apple.com/guide/passwords/welcome/mac
- Apple, Shared Password Groups:
  https://support.apple.com/guide/personal-safety/manage-shared-password-and-passkeys-ips3ce9f6e15/web
- Apple, Verification Codes:
  https://support.apple.com/guide/passwords/verification-codes-mchl873a6e72/mac
- Google, Manage Passwords In Chrome:
  https://support.google.com/chrome/answer/95606
- Google, Password Checkup:
  https://support.google.com/accounts/answer/9457609
- Google, Import Passwords:
  https://support.google.com/accounts/answer/10500247
- LastPass, Zero-Knowledge Security:
  https://www.lastpass.com/security/zero-knowledge-security
- LastPass, How LastPass Works:
  https://www.lastpass.com/how-lastpass-works
- LastPass, Security:
  https://www.lastpass.com/security
- Dashlane, Security:
  https://www.dashlane.com/security
- NordPass, Security:
  https://nordpass.com/security/
- KeePassXC, User Guide:
  https://keepassxc.org/docs/KeePassXC_UserGuide
- KeePassXC, Getting Started Guide:
  https://keepassxc.org/docs/KeePassXC_GettingStarted
