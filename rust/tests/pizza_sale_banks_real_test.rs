//! The cross-context edge against the REAL bluebooks on disk, and the two
//! things that stop a sale reaching the real Banking today. Pins the gap so it
//! cannot close silently or widen unnoticed.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use storehouse::runtime::{Runtime, Value};
use storehouse::{hecksagon_parser, parser};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(rel: &str) -> String {
    let p = repo().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

fn attrs(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn money(cents: i64) -> Value {
    let mut m = HashMap::new();
    m.insert("cents".to_string(), Value::Int(cents));
    m.insert("currency".to_string(), s("USD"));
    Value::Map(m)
}

fn booted() -> Runtime {
    let mut domain = parser::parse(&read("examples/pizzas/bluebook/pizzas.bluebook"));
    domain
        .aggregates
        .extend(parser::parse(&read("examples/banking/hecks/banking.bluebook")).aggregates);
    let hex = hecksagon_parser::parse(&read("examples/pizzas/bluebook/pizzas.hecksagon"));
    Runtime::boot_with_hecksagons(domain, None, vec![hex])
}

#[test]
fn the_real_hecksagon_carries_the_cross_context_handlers() {
    let hex = hecksagon_parser::parse(&read("examples/pizzas/bluebook/pizzas.hecksagon"));

    let deposit = hex
        .driven_adapters
        .iter()
        .find(|a| a.name == "Deposit")
        .expect("pizzas.hecksagon makes a Deposit from every sale");
    assert!(!deposit.handlers.is_empty());

    let banking = hex
        .driven_adapters
        .iter()
        .find(|a| a.name == "Banking")
        .expect("pizzas.hecksagon banks the deposit");
    let h = &banking.handlers[0];
    assert_eq!(h.success, "Pizzas::Deposit.Clear", "the bank's yes comes home");
    assert_eq!(h.failure, "Pizzas::Deposit.Bounce", "and so does its no");
}

/// THE GAP, pinned. Against the real Banking a sale's deposit BOUNCES, for two
/// reasons worth keeping visible :
///
///   1. `{amount}` interpolates a value object to the EMPTY STRING
///      (driven_adapter_args::val_str returns "" for Map/List), so Banking's
///      Money-typed `amount` arrives unusable and `amount.positive?` cannot be
///      judged.
///   2. The real Banking mints runtime account ids, so `account: "pizzeria"`
///      names no account that was ever opened.
///
/// The DEPOSIT INFRASTRUCTURE is what makes this tolerable : the failure is not
/// lost. It lands as a bounced deposit carrying the bank's own reason, which is
/// the whole point of the aggregate. When both gaps close, this test flips to
/// `cleared` — deliberately, by someone who read it.
#[test]
fn against_real_banking_the_deposit_bounces_and_says_why() {
    let mut rt = booted();

    rt.dispatch(
        "Banking::Customer.RegisterCustomer",
        attrs(&[("name", s("Pizzeria")), ("email", s("shop@example.com"))]),
    )
    .expect("register the shop");
    rt.dispatch(
        "Banking::Account.OpenAccount",
        attrs(&[
            ("customer", s("1")),
            ("account_type", s("checking")),
            ("daily_limit", money(50000)),
            ("opening_currency", s("USD")),
        ]),
    )
    .expect("open the shop's account");

    rt.dispatch(
        "Pizzas::Order.PlaceOrder",
        attrs(&[
            ("pizza", s("1")),
            ("customer_name", s("Ada")),
            ("quantity", Value::Int(1)),
            ("total", money(1400)),
        ]),
    )
    .expect("the sale goes through — a sale is never refused over the banking");

    let deposits = rt.all("Deposit");
    assert_eq!(deposits.len(), 1, "the sale made a deposit");

    let status = deposits[0].fields.get("status").map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(
        status, "bounced",
        "a deposit the bank could not take must be BOUNCED, never silently lost"
    );

    let reason = deposits[0].fields.get("reason").map(|v| v.to_string()).unwrap_or_default();
    assert!(
        !reason.is_empty(),
        "and it must carry the bank's own reason, got {reason:?}"
    );
}
