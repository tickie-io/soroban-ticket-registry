# Tickie × Stellar — Soroban Ticket Registry

[![CI](https://github.com/tickie-io/soroban-ticket-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/tickie-io/soroban-ticket-registry/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

On-chain ticket ownership, compliant resale and USDC settlement for the live events industry — built on [Soroban](https://developers.stellar.org/docs/build/smart-contracts), by [Tickie](https://www.tickie.io/en).

**Tickie** is a profitable, bootstrapped B2B event-infrastructure platform: 70+ professional organizers (sports clubs, festivals, venues), 500,000+ tickets delivered, live in France and expanding internationally. This repository contains the Stellar settlement and ownership layer we are building on top of that installed base, as part of our [Stellar Community Fund](https://communityfund.stellar.org/) Build submission.

## Why this exists

Event ticketing has three structural problems that off-chain databases cannot solve:

1. **Fraud** — duplicated and counterfeit tickets are only detected at the gate, when it is too late.
2. **Grey-market resale** — French organizers alone lose an estimated €6.5M per event night to unregulated resale, with zero value returning to the rightsholder.
3. **Cross-border settlement** — card rails cost ~3% plus multi-day payout delays, which makes per-resale royalty distribution economically impossible.

Stellar is the only chain where the unit economics of per-resale royalty splits close: sub-cent fees and ~5-second finality across thousands of micro-settlements per event.

## Roadmap (3 phases, ~6 months)

| Phase | Contract / component | Status |
|---|---|---|
| 1 | **Ticket Registry** (`contracts/ticket-registry`) — duplicate-proof on-chain ticket issuance, ownership, transfer and atomic gate check-in | ✅ In development (this repo) |
| 2 | **Marketplace** — compliant peer-to-peer resale with **atomic royalty splits** to organizers, enforcing per-event price caps and transfer windows | 🔜 Planned |
| 3 | **USDC settlement** — cross-border payments and organizer payouts via the [Anchor Platform](https://developers.stellar.org/docs/anchoring-assets), [Stellar Wallets Kit](https://stellarwalletskit.dev/) and [Passkey Kit](https://github.com/kalepail/passkey-kit) onboarding | 🔜 Planned |

The full technical design — contract interfaces, data flows, failure modes, compliance model — is in [ARCHITECTURE.md](ARCHITECTURE.md).

## The Ticket Registry contract

Every ticket sold through Tickie is registered as a unique on-chain record:

- `create_event` — registers an event with its resale policy (organizer address, royalty bps, resale price cap) and metadata URI.
- `mint_ticket` — registers a ticket (event, seat, face value, validity window). A ticket id can only ever be minted once: duplicates are structurally impossible.
- `transfer` — moves ownership, authorized by the current holder only.
- `check_in` — consumes the ticket at the venue gate, atomically and exactly once.
- `revoke` — organizer-side cancellation (refunds, cancelled events).
- `get_ticket` / `get_event` / `owner_of` / `is_valid` — read entry points used by the Tickie backend and access-control devices.

Privacy by construction: **no personal data on-chain**. Ticket ids are SHA-256 hashes of internal references; holders appear only as Stellar addresses (GDPR-compatible).

## Live on testnet

The Phase 1 contract is deployed and exercised on Stellar testnet:

- Contract: [`CBUVEOKA5YI3JQRJ2HRNULK63FVXKGWUWHWRCBXJ353YHNJFLG3ZDXYG`](https://stellar.expert/explorer/testnet/contract/CBUVEOKA5YI3JQRJ2HRNULK63FVXKGWUWHWRCBXJ353YHNJFLG3ZDXYG)
- Verified end-to-end on-chain: `create_event` → `mint_ticket` → `get_ticket`/`is_valid` → `check_in`, including the two failure paths that matter for fraud resistance (duplicate mint rejected with `TicketAlreadyExists`, second gate check-in rejected with `TicketNotValid`).

## Getting started

Prerequisites: [Rust](https://www.rust-lang.org/tools/install) (stable) and the [Stellar CLI](https://developers.stellar.org/docs/tools/cli/install-cli).

```bash
# Run the test suite
cargo test

# Build the Wasm contract
cargo build --target wasm32v1-none --release

# Deploy to testnet (generates and funds an identity if needed)
./scripts/deploy_testnet.sh
```

## Repository layout

```
contracts/
  ticket-registry/    Phase 1 — Soroban ticket registry contract
scripts/
  deploy_testnet.sh   Build + deploy + initialize on Stellar testnet
ARCHITECTURE.md       Full technical architecture (all 3 phases)
```

## Team

Built in-house by the Tickie engineering team, led by [Nidhal Sabbah](https://github.com/nisa10880) (CTO & co-founder).

- Website: https://www.tickie.io/en
- X: https://x.com/tickie_io
- LinkedIn: https://www.linkedin.com/company/tickie

## License

[MIT](LICENSE) — all Soroban contracts in this repository are open source and designed as reusable primitives for the broader Stellar event-tech ecosystem.
