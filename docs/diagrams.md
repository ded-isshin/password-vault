# Architecture Diagrams

Status: draft.

## System Context

```mermaid
flowchart TB
  Human[Human architect] --> GitHub[GitHub issues, PRs, Actions, docs]
  GitHub --> Product[password-vault repository]
  Product --> GHCR[GHCR image registry]
  Product --> GitOps[GitOps PR to infrastructure repository]
  GitOps --> Argo[Argo CD]
  Argo --> Cluster[Kubernetes cluster]
  Cluster --> App[password-vault app]
  Cluster --> PG[(PostgreSQL)]
  Cluster -. future .-> Secrets[Vault/OpenBao or other secret manager]
```

## Trust Boundaries

```mermaid
flowchart LR
  subgraph ClientTrust[Client trust boundary]
    Browser[Browser UI]
    Crypto[Client crypto]
    Unlock[Unlock material in memory]
  end

  subgraph ServerTrust[Server trust boundary]
    API[API service]
    Session[Server sessions]
    MFA[TOTP verifier]
  end

  subgraph DataTrust[Persistent data]
    DB[(PostgreSQL ciphertext + metadata)]
    Backup[(Backups + WAL archive)]
  end

  Browser --> Crypto
  Unlock --> Crypto
  Crypto -->|ciphertext only| API
  API --> DB
  DB --> Backup
  API --> MFA
  API --> Session
```

## Vault Item Lifecycle

```mermaid
sequenceDiagram
  participant User
  participant Client
  participant API
  participant DB as PostgreSQL

  User->>Client: unlock vault
  Client->>Client: derive/recover local vault key
  User->>Client: create item
  Client->>Client: encrypt item payload
  Client->>API: submit ciphertext revision
  API->>API: authorize vault membership
  API->>DB: store item revision
  DB-->>API: revision id
  API-->>Client: sync ack
```

## Login And Unlock Separation

```mermaid
stateDiagram-v2
  [*] --> Anonymous
  Anonymous --> AuthenticatedLocked: login + TOTP
  AuthenticatedLocked --> AuthenticatedUnlocked: local unlock
  AuthenticatedUnlocked --> AuthenticatedLocked: auto-lock / manual lock
  AuthenticatedLocked --> Anonymous: logout / session expiry
  AuthenticatedUnlocked --> Anonymous: logout / session expiry
```

## Auth Direction For MVP And Future

```mermaid
flowchart LR
  P[User password] --> KDF[Client KDF]
  SK[Account secret key] --> KDF
  KDF --> AUTH[Client auth secret]
  KDF --> UNLOCK[Vault unlock material]
  AUTH --> HASH[Server-side slow hash]
  HASH --> S[(Auth verifier storage)]
  UNLOCK --> WRAP[Unwrap user or vault keys]
  WRAP --> ITEMS[Decrypt item payloads locally]
  TOTP[TOTP code] --> MFA[Login MFA verification]
  PASSKEY[Future WebAuthn/passkey] -. future .-> MFA
```

The server session authenticates API access. The unlock path stays local to the client.

## Multi-Device Key-Wrap Direction

```mermaid
flowchart TB
  Account[User account] --> D1[Browser device]
  Account --> D2[Future browser extension]
  Account --> D3[Future mobile app]
  D1 --> W1[Wrapped user/vault keys]
  D2 --> W2[Wrapped user/vault keys]
  D3 --> W3[Wrapped user/vault keys]
  W1 --> Vault[Vault key]
  W2 --> Vault
  W3 --> Vault
  Vault --> Payloads[Encrypted item revisions]
```

The first MVP client is the browser web app. The protocol and data model should still support
multiple enrolled devices from the beginning.

## Kubernetes Data Platform Direction

```mermaid
flowchart TB
  subgraph Workers[Worker nodes]
    P1[Postgres primary pod + local PV]
    R1[Postgres replica pod + local PV]
    R2[Postgres replica pod + local PV]
  end

  App1[API replica] --> RW[CloudNativePG read-write service]
  App2[API replica] --> RW
  RW --> P1
  P1 --> R1
  P1 --> R2
  P1 -. WAL archive .-> Obj[(External object storage)]
  R1 -. base backup candidate .-> Obj
```

Local PVs do not move data between workers. Single-worker tolerance comes from PostgreSQL
replication plus failover, not from distributed storage.
