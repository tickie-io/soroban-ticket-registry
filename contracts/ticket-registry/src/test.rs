#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, BytesN, Env, String};

const EVENT_ID: u64 = 42;
const VALID_FROM: u64 = 1_000;
const VALID_UNTIL: u64 = 2_000;

fn setup() -> (Env, TicketRegistryClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(TicketRegistry, (&admin,));
    let client = TicketRegistryClient::new(&env, &contract_id);
    (env, client, admin)
}

fn ticket_id(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

fn create_default_event(env: &Env, client: &TicketRegistryClient) -> Address {
    let organizer = Address::generate(env);
    client.create_event(
        &EVENT_ID,
        &organizer,
        &500,    // 5% royalty on resale
        &10_000, // resale capped at face value
        &String::from_str(
            env,
            "https://raw.githubusercontent.com/tickie-io/soroban-ticket-registry/main/examples/metadata/42.json",
        ),
    );
    organizer
}

fn mint_default_ticket(env: &Env, client: &TicketRegistryClient, id: &BytesN<32>) -> Address {
    let owner = Address::generate(env);
    client.mint_ticket(
        id,
        &EVENT_ID,
        &owner,
        &String::from_str(env, "T12-R04-S27"),
        &35_0000000, // 35 USDC face value
        &VALID_FROM,
        &VALID_UNTIL,
    );
    owner
}

#[test]
fn create_event_and_mint_ticket() {
    let (env, client, _admin) = setup();
    let organizer = create_default_event(&env, &client);

    let event = client.get_event(&EVENT_ID);
    assert_eq!(event.organizer, organizer);
    assert_eq!(event.royalty_bps, 500);
    assert_eq!(event.resale_cap_bps, 10_000);

    let id = ticket_id(&env, 1);
    let owner = mint_default_ticket(&env, &client, &id);

    let ticket = client.get_ticket(&id);
    assert_eq!(ticket.event_id, EVENT_ID);
    assert_eq!(ticket.owner, owner);
    assert_eq!(ticket.status, TicketStatus::Valid);
    assert_eq!(client.owner_of(&id), owner);
    assert!(client.is_valid(&id));
}

#[test]
fn duplicate_ticket_id_is_rejected() {
    let (env, client, _admin) = setup();
    create_default_event(&env, &client);
    let id = ticket_id(&env, 1);
    mint_default_ticket(&env, &client, &id);

    let other_owner = Address::generate(&env);
    let result = client.try_mint_ticket(
        &id,
        &EVENT_ID,
        &other_owner,
        &String::from_str(&env, "T12-R04-S28"),
        &35_0000000,
        &VALID_FROM,
        &VALID_UNTIL,
    );
    assert_eq!(result, Err(Ok(Error::TicketAlreadyExists)));
}

#[test]
fn mint_requires_existing_event() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let result = client.try_mint_ticket(
        &ticket_id(&env, 1),
        &99, // never created
        &owner,
        &String::from_str(&env, ""),
        &0,
        &VALID_FROM,
        &VALID_UNTIL,
    );
    assert_eq!(result, Err(Ok(Error::EventNotFound)));
}

#[test]
fn royalty_above_100_percent_is_rejected() {
    let (env, client, _admin) = setup();
    let organizer = Address::generate(&env);
    let result = client.try_create_event(
        &EVENT_ID,
        &organizer,
        &10_001,
        &10_000,
        &String::from_str(&env, ""),
    );
    assert_eq!(result, Err(Ok(Error::InvalidRoyalty)));
}

#[test]
fn inverted_validity_window_is_rejected() {
    let (env, client, _admin) = setup();
    create_default_event(&env, &client);
    let owner = Address::generate(&env);
    let result = client.try_mint_ticket(
        &ticket_id(&env, 1),
        &EVENT_ID,
        &owner,
        &String::from_str(&env, ""),
        &0,
        &VALID_UNTIL, // from >= until
        &VALID_FROM,
    );
    assert_eq!(result, Err(Ok(Error::InvalidValidityWindow)));
}

#[test]
fn transfer_moves_ownership() {
    let (env, client, _admin) = setup();
    create_default_event(&env, &client);
    let id = ticket_id(&env, 1);
    mint_default_ticket(&env, &client, &id);

    let new_owner = Address::generate(&env);
    client.transfer(&id, &new_owner);
    assert_eq!(client.owner_of(&id), new_owner);
}

#[test]
#[should_panic]
fn transfer_without_owner_authorization_panics() {
    let (env, client, _admin) = setup();
    create_default_event(&env, &client);
    let id = ticket_id(&env, 1);
    mint_default_ticket(&env, &client, &id);

    // Drop all auth mocks: the owner no longer signs, so transfer must fail.
    env.set_auths(&[]);
    client.transfer(&id, &Address::generate(&env));
}

#[test]
fn check_in_consumes_the_ticket_exactly_once() {
    let (env, client, _admin) = setup();
    create_default_event(&env, &client);
    let id = ticket_id(&env, 1);
    mint_default_ticket(&env, &client, &id);

    env.ledger().with_mut(|l| l.timestamp = VALID_FROM + 100);
    client.check_in(&id);
    assert_eq!(client.get_ticket(&id).status, TicketStatus::CheckedIn);

    // A duplicated ticket presented a second time at the gate is rejected.
    assert_eq!(client.try_check_in(&id), Err(Ok(Error::TicketNotValid)));
    assert!(!client.is_valid(&id));
}

#[test]
fn check_in_outside_validity_window_fails() {
    let (env, client, _admin) = setup();
    create_default_event(&env, &client);
    let id = ticket_id(&env, 1);
    mint_default_ticket(&env, &client, &id);

    env.ledger().with_mut(|l| l.timestamp = VALID_UNTIL + 1);
    assert_eq!(
        client.try_check_in(&id),
        Err(Ok(Error::OutsideValidityWindow))
    );
}

#[test]
fn checked_in_ticket_cannot_be_transferred() {
    let (env, client, _admin) = setup();
    create_default_event(&env, &client);
    let id = ticket_id(&env, 1);
    mint_default_ticket(&env, &client, &id);

    env.ledger().with_mut(|l| l.timestamp = VALID_FROM + 100);
    client.check_in(&id);

    let result = client.try_transfer(&id, &Address::generate(&env));
    assert_eq!(result, Err(Ok(Error::TicketNotTransferable)));
}

#[test]
fn revoked_ticket_is_dead() {
    let (env, client, _admin) = setup();
    create_default_event(&env, &client);
    let id = ticket_id(&env, 1);
    mint_default_ticket(&env, &client, &id);

    client.revoke(&id);
    assert_eq!(client.get_ticket(&id).status, TicketStatus::Revoked);
    assert!(!client.is_valid(&id));
    assert_eq!(
        client.try_transfer(&id, &Address::generate(&env)),
        Err(Ok(Error::TicketNotTransferable))
    );
    assert_eq!(client.try_check_in(&id), Err(Ok(Error::TicketNotValid)));
}

#[test]
fn expired_ticket_is_not_valid() {
    let (env, client, _admin) = setup();
    create_default_event(&env, &client);
    let id = ticket_id(&env, 1);
    mint_default_ticket(&env, &client, &id);

    env.ledger().with_mut(|l| l.timestamp = VALID_UNTIL + 1);
    assert!(!client.is_valid(&id));
}

#[test]
fn admin_rotation() {
    let (env, client, admin) = setup();
    assert_eq!(client.get_admin(), admin);

    let new_admin = Address::generate(&env);
    client.set_admin(&new_admin);
    assert_eq!(client.get_admin(), new_admin);
}
