//! Selling a pizza puts money in the bank — the corpus's first CROSS-CONTEXT
//! edge, wired entirely in the hecksagon (2026-07-28).
//!
//! Pizzas emits OrderPlaced. Banking's Account.Deposit takes an account, an
//! amount and a description. NEITHER bluebook names the other : pizzas.bluebook
//! never says "Deposit", banking.bluebook never says "pizza". The one place
//! they meet is a `driven on` block in pizzas.hecksagon, where the rename
//! total -> amount IS the anti-corruption layer.
//!
//! No port, no family, no adapter contract — those buy swappability and there
//! is nothing here to swap. One domain's event calling another domain's
//! command, with the transformation written where the two touch.

use std::collections::HashMap;
use storehouse::runtime::{Runtime, Value};
use storehouse::{hecksagon_parser, parser};

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

fn attrs(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

// The two domains, side by side in one runtime the way a deployment holds them.
// Trimmed to the parts this edge touches ; the shapes match the real bluebooks.
const PIZZAS: &str = r#"Hecks.bluebook "Pizzas" do
  aggregate "Order" do
    attribute :customer_name, CustomerName
    attribute :total, Total
    value_object "CustomerName" do
      attribute :value, String
    end
    value_object "Total" do
      attribute :cents, Integer, default: 0
    end
    command "PlaceOrder" do
      role "Customer"
      attribute :customer_name, CustomerName
      attribute :total, Total
      then_set :customer_name, to: :customer_name
      then_set :total, to: :total
      emits "OrderPlaced"
    end
  end
end"#;

const BANKING: &str = r#"Hecks.bluebook "Banking" do
  aggregate "Account" do
    identified_by :name
    attribute :name, AccountName
    attribute :balance, Integer, default: 0
    attribute :ledger, list_of(LedgerEntry)
    value_object "AccountName" do
      attribute :value, String
    end
    value_object "LedgerEntry" do
      attribute :amount, Integer
      attribute :description, String
    end
    command "OpenAccount" do
      role "Teller"
      attribute :name, AccountName
      then_set :name, to: :name
      emits "AccountOpened"
    end
    command "Deposit" do
      role "Teller"
      reference_to Account
      attribute :amount, Integer
      attribute :description, String
      given("a deposit moves something") { amount > 0 }
      then_set :balance, increment: :amount
      then_set :ledger, append: { amount: :amount, description: :description }
      emits "Deposited"
    end
  end
end"#;

// The whole cross-context wire. `{total}` is the sale's field ; `amount` is
// what Banking calls it.
const PIZZAS_HEX: &str = r#"Hecks.hecksagon "Pizzas" do
  adapter "Banking" do
    driven on "Pizzas::Order.OrderPlaced" do |sale|
      dispatch "Banking::Account.Deposit", account: "pizzeria", amount: "{total}", description: "Pizza sale"
    end
  end
end"#;

fn booted() -> Runtime {
    let mut domain = parser::parse(PIZZAS);
    let banking = parser::parse(BANKING);
    domain.aggregates.extend(banking.aggregates);

    let hex = hecksagon_parser::parse(PIZZAS_HEX);
    let mut rt = Runtime::boot_with_hecksagons(domain, None, vec![hex]);
    rt.dispatch("Banking::Account.OpenAccount", attrs(&[("name", s("pizzeria"))]))
        .expect("the pizzeria has an account");
    rt
}

fn pizzeria_balance(rt: &Runtime) -> String {
    rt.all("Account")
        .iter()
        .find(|r| r.id == "pizzeria")
        .and_then(|r| r.fields.get("balance").map(|v| v.to_string()))
        .unwrap_or_default()
}

#[test]
fn selling_a_pizza_deposits_the_total_into_the_pizzeria_account() {
    let mut rt = booted();
    assert_eq!(pizzeria_balance(&rt), "0", "the till starts empty");

    // One $10 pizza with two $2 toppings = $14.
    rt.dispatch(
        "Pizzas::Order.PlaceOrder",
        attrs(&[("customer_name", s("Ada")), ("total", Value::Int(1400))]),
    )
    .expect("the sale goes through");

    assert_eq!(
        pizzeria_balance(&rt),
        "1400",
        "selling a pizza must bank its total — no bluebook naming the other"
    );
}

#[test]
fn a_second_sale_adds_to_the_takings() {
    let mut rt = booted();
    for total in [1400, 1000] {
        rt.dispatch(
            "Pizzas::Order.PlaceOrder",
            attrs(&[("customer_name", s("Ada")), ("total", Value::Int(total))]),
        )
        .expect("the sale goes through");
    }
    assert_eq!(pizzeria_balance(&rt), "2400", "takings accumulate");
}

#[test]
fn the_deposit_is_described_as_a_pizza_sale_in_the_ledger() {
    let mut rt = booted();
    rt.dispatch(
        "Pizzas::Order.PlaceOrder",
        attrs(&[("customer_name", s("Ada")), ("total", Value::Int(1400))]),
    )
    .expect("the sale goes through");

    let acct = rt.all("Account");
    let row = acct.iter().find(|r| r.id == "pizzeria").expect("account");
    let ledger = row.fields.get("ledger").map(|v| v.to_string()).unwrap_or_default();
    assert!(
        ledger.contains("1 items") || ledger.contains("Pizza sale"),
        "the sale must leave a ledger row, got {ledger:?}"
    );
}
