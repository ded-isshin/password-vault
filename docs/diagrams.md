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
