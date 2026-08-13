# Mermaid render bisection (temporary)

## A. Minimal flowchart

```mermaid
flowchart LR
A --> B
```

## B. Full flowchart (current ARCHITECTURE version)

```mermaid
flowchart LR
    subgraph Existing["Tickie platform (production today)"]
        FO["Ticketing storefronts"] --> BE["NestJS backend"]
        AC["Access-control apps"] --> BE
        ORG["Organizer dashboard"] --> BE
        BE --> DB[("PostgreSQL")]
        BE --> Q[["BullMQ outbox queue"]]
    end
    subgraph StellarLayer["Stellar layer (this project)"]
        Q --> RPC["Stellar RPC"]
        RPC --> TR["Ticket Registry contract"]
        RPC --> MP["Marketplace contract - Phase 2"]
        MP --> USDC["Stellar Asset Contract - USDC"]
        ANCHOR["Anchor Platform - EUR on-off ramp"] --> USDC
        WK["Stellar Wallets Kit + Passkey Kit"] --> MP
    end
```

## C. Flowchart no subgraph, no indentation

```mermaid
flowchart LR
FO["Ticketing storefronts"] --> BE["NestJS backend"]
BE --> DB[("PostgreSQL")]
BE --> Q[["BullMQ outbox queue"]]
Q --> RPC["Stellar RPC"]
RPC --> TR["Ticket Registry contract"]
```

## D. Minimal sequence

```mermaid
sequenceDiagram
Alice->>Bob: hello
Bob-->>Alice: world
```

## E. Full sequence (current ARCHITECTURE version)

```mermaid
sequenceDiagram
    participant Buyer
    participant BE as Tickie backend
    participant Q as Outbox queue
    participant RPC as Stellar RPC
    participant TR as Ticket Registry

    Buyer->>BE: purchase (card or USDC)
    BE->>BE: create ticket in PostgreSQL (system of record)
    BE-->>Buyer: ticket delivered immediately (QR + wallet link)
    BE->>Q: enqueue on-chain registration (idempotent job)
    Q->>RPC: submit mint_ticket with sha256 ticket id
    RPC->>TR: transaction
    TR-->>RPC: TicketMinted event
    RPC-->>BE: event ingested, ticket marked on-chain
```
