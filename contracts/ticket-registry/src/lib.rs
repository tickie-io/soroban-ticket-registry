//! # Tickie Ticket Registry
//!
//! On-chain registry for event tickets issued through the
//! [Tickie](https://www.tickie.io) platform. Every ticket sold via Tickie is
//! registered here as a unique on-chain record, providing:
//!
//! - **Verifiable ownership** — each ticket is bound to a Stellar address.
//! - **Duplicate-proof issuance** — a ticket id can only ever be minted once.
//! - **Fraud-resistant entry validation** — check-in is atomic and final, so a
//!   ticket cannot pass the gate twice.
//! - **Resale policy anchoring** — every event carries the organizer royalty
//!   rate and resale price cap enforced by the (Phase 2) marketplace contract.
//!
//! This is Phase 1 of the Tickie × Stellar integration:
//! 1. **Ticket Registry** (this contract)
//! 2. Secondary marketplace with atomic royalty splits (Phase 2)
//! 3. Cross-border USDC settlement via Anchor Platform (Phase 3)
//!
//! Privacy: no personal data is ever written on-chain. Ticket ids are SHA-256
//! hashes of internal Tickie ticket references, and holders are represented
//! only by their Stellar address (GDPR-safe by construction).

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, BytesN, Env,
    String,
};

/// Ledgers per day, assuming ~5s ledger close time.
const DAY_IN_LEDGERS: u32 = 17_280;
/// Rent: extend entries to ~120 days whenever they are touched…
const EXTEND_TO_LEDGERS: u32 = 120 * DAY_IN_LEDGERS;
/// …but only if fewer than ~30 days of TTL remain.
const TTL_THRESHOLD_LEDGERS: u32 = 30 * DAY_IN_LEDGERS;

/// 100% expressed in basis points.
const BPS_DENOMINATOR: u32 = 10_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    EventAlreadyExists = 1,
    EventNotFound = 2,
    TicketAlreadyExists = 3,
    TicketNotFound = 4,
    /// Ticket is checked in or revoked and can no longer change hands.
    TicketNotTransferable = 5,
    /// Ticket is not in the `Valid` state.
    TicketNotValid = 6,
    /// Check-in attempted outside the ticket validity window.
    OutsideValidityWindow = 7,
    InvalidRoyalty = 8,
    InvalidValidityWindow = 9,
    InvalidFaceValue = 10,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketStatus {
    /// Live ticket: can be transferred, resold and checked in.
    Valid,
    /// Consumed at the venue gate. Terminal state.
    CheckedIn,
    /// Cancelled by the organizer (refund, event cancellation). Terminal state.
    Revoked,
}

/// Per-event resale policy and metadata anchor.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventInfo {
    /// Address receiving resale royalties (the organizer's Stellar account).
    pub organizer: Address,
    /// Royalty on every resale, in basis points (e.g. 500 = 5%).
    pub royalty_bps: u32,
    /// Maximum resale price as basis points of face value
    /// (10_000 = resale capped at face value, as required for regulated
    /// French resale; 0 = resale disabled for this event).
    pub resale_cap_bps: u32,
    /// Off-chain metadata URI (event name, venue, imagery…). No PII.
    pub metadata_uri: String,
}

/// One registered ticket.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ticket {
    pub event_id: u64,
    pub owner: Address,
    /// Seat reference (e.g. "T12-R04-S27"), empty for general admission.
    pub seat: String,
    /// Face value in minor units of the settlement asset (USDC, 7 decimals).
    pub face_value: i128,
    /// Unix timestamp (seconds) from which entry is allowed.
    pub valid_from: u64,
    /// Unix timestamp (seconds) after which the ticket expires.
    pub valid_until: u64,
    pub status: TicketStatus,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Event(u64),
    Ticket(BytesN<32>),
}

// ── Contract events (ingested by the Tickie backend via Stellar RPC) ─────

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChanged {
    pub new_admin: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventCreated {
    #[topic]
    pub event_id: u64,
    pub organizer: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketMinted {
    #[topic]
    pub ticket_id: BytesN<32>,
    pub event_id: u64,
    pub owner: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketTransferred {
    #[topic]
    pub ticket_id: BytesN<32>,
    pub from: Address,
    pub to: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketCheckedIn {
    #[topic]
    pub ticket_id: BytesN<32>,
    pub owner: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketRevoked {
    #[topic]
    pub ticket_id: BytesN<32>,
    pub owner: Address,
}

#[contract]
pub struct TicketRegistry;

#[contractimpl]
impl TicketRegistry {
    /// Deploy-time constructor. `admin` is the Tickie platform account that
    /// issues tickets and operates venue check-in devices.
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    // ── Admin ────────────────────────────────────────────────────────────

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    /// Rotate the platform admin key.
    pub fn set_admin(env: Env, new_admin: Address) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        AdminChanged { new_admin }.publish(&env);
    }

    // ── Events (the real-world kind) ─────────────────────────────────────

    /// Register an event and its resale policy. Admin only.
    pub fn create_event(
        env: Env,
        event_id: u64,
        organizer: Address,
        royalty_bps: u32,
        resale_cap_bps: u32,
        metadata_uri: String,
    ) -> Result<(), Error> {
        Self::require_admin(&env);
        if royalty_bps > BPS_DENOMINATOR {
            return Err(Error::InvalidRoyalty);
        }
        let key = DataKey::Event(event_id);
        if env.storage().persistent().has(&key) {
            return Err(Error::EventAlreadyExists);
        }
        let info = EventInfo {
            organizer: organizer.clone(),
            royalty_bps,
            resale_cap_bps,
            metadata_uri,
        };
        env.storage().persistent().set(&key, &info);
        Self::bump(&env, &key);
        EventCreated {
            event_id,
            organizer,
        }
        .publish(&env);
        Ok(())
    }

    pub fn get_event(env: Env, event_id: u64) -> Result<EventInfo, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .ok_or(Error::EventNotFound)
    }

    // ── Tickets ──────────────────────────────────────────────────────────

    /// Register a ticket sold on the Tickie platform. Admin only.
    ///
    /// `ticket_id` is the SHA-256 hash of the internal Tickie ticket
    /// reference: registration is idempotent-safe because a given id can
    /// only ever exist once (duplicate mints are rejected, never silently
    /// overwritten).
    #[allow(clippy::too_many_arguments)]
    pub fn mint_ticket(
        env: Env,
        ticket_id: BytesN<32>,
        event_id: u64,
        owner: Address,
        seat: String,
        face_value: i128,
        valid_from: u64,
        valid_until: u64,
    ) -> Result<(), Error> {
        Self::require_admin(&env);
        if !env.storage().persistent().has(&DataKey::Event(event_id)) {
            return Err(Error::EventNotFound);
        }
        if valid_from >= valid_until {
            return Err(Error::InvalidValidityWindow);
        }
        if face_value < 0 {
            return Err(Error::InvalidFaceValue);
        }
        let key = DataKey::Ticket(ticket_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::TicketAlreadyExists);
        }
        let ticket = Ticket {
            event_id,
            owner: owner.clone(),
            seat,
            face_value,
            valid_from,
            valid_until,
            status: TicketStatus::Valid,
        };
        env.storage().persistent().set(&key, &ticket);
        Self::bump(&env, &key);
        TicketMinted {
            ticket_id,
            event_id,
            owner,
        }
        .publish(&env);
        Ok(())
    }

    pub fn get_ticket(env: Env, ticket_id: BytesN<32>) -> Result<Ticket, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Ticket(ticket_id))
            .ok_or(Error::TicketNotFound)
    }

    pub fn owner_of(env: Env, ticket_id: BytesN<32>) -> Result<Address, Error> {
        Ok(Self::get_ticket(env, ticket_id)?.owner)
    }

    /// True if the ticket exists, is in the `Valid` state and has not expired.
    pub fn is_valid(env: Env, ticket_id: BytesN<32>) -> bool {
        match Self::get_ticket(env.clone(), ticket_id) {
            Ok(t) => t.status == TicketStatus::Valid && env.ledger().timestamp() <= t.valid_until,
            Err(_) => false,
        }
    }

    /// Transfer a ticket to a new holder. Requires the current owner's
    /// authorization. Phase 2 routes resales through the marketplace
    /// contract, which enforces the event resale policy and performs the
    /// atomic royalty split before calling into this entry point.
    pub fn transfer(env: Env, ticket_id: BytesN<32>, to: Address) -> Result<(), Error> {
        let key = DataKey::Ticket(ticket_id.clone());
        let mut ticket: Ticket = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::TicketNotFound)?;
        if ticket.status != TicketStatus::Valid {
            return Err(Error::TicketNotTransferable);
        }
        ticket.owner.require_auth();
        let from = ticket.owner.clone();
        ticket.owner = to.clone();
        env.storage().persistent().set(&key, &ticket);
        Self::bump(&env, &key);
        TicketTransferred {
            ticket_id,
            from,
            to,
        }
        .publish(&env);
        Ok(())
    }

    /// Consume a ticket at the venue gate. Admin only (invoked by the Tickie
    /// access-control backend). Atomic and final: a second check-in of the
    /// same ticket fails, which is what makes duplicated tickets worthless.
    pub fn check_in(env: Env, ticket_id: BytesN<32>) -> Result<(), Error> {
        Self::require_admin(&env);
        let key = DataKey::Ticket(ticket_id.clone());
        let mut ticket: Ticket = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::TicketNotFound)?;
        if ticket.status != TicketStatus::Valid {
            return Err(Error::TicketNotValid);
        }
        let now = env.ledger().timestamp();
        if now < ticket.valid_from || now > ticket.valid_until {
            return Err(Error::OutsideValidityWindow);
        }
        ticket.status = TicketStatus::CheckedIn;
        env.storage().persistent().set(&key, &ticket);
        Self::bump(&env, &key);
        TicketCheckedIn {
            ticket_id,
            owner: ticket.owner,
        }
        .publish(&env);
        Ok(())
    }

    /// Cancel a ticket (refund or event cancellation). Admin only.
    pub fn revoke(env: Env, ticket_id: BytesN<32>) -> Result<(), Error> {
        Self::require_admin(&env);
        let key = DataKey::Ticket(ticket_id.clone());
        let mut ticket: Ticket = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::TicketNotFound)?;
        if ticket.status != TicketStatus::Valid {
            return Err(Error::TicketNotValid);
        }
        ticket.status = TicketStatus::Revoked;
        env.storage().persistent().set(&key, &ticket);
        Self::bump(&env, &key);
        TicketRevoked {
            ticket_id,
            owner: ticket.owner,
        }
        .publish(&env);
        Ok(())
    }

    // ── Internals ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD_LEDGERS, EXTEND_TO_LEDGERS);
    }

    fn bump(env: &Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_THRESHOLD_LEDGERS, EXTEND_TO_LEDGERS);
    }
}

mod test;
