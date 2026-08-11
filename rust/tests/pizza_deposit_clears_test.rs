//! A sale makes a deposit ; the bank answers ; the answer comes back as Clear
//! or Bounce. Before this, a refused deposit existed only on stderr.

use std::collections::HashMap;
use storehouse::runtime::{Runtime, Value};
use storehouse::{hecksagon_parser, parser};

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

fn attrs(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

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

  aggregate "Deposit" do
    reference_to Order
    attribute :amount, Amount
    attribute :reason, Reason
    value_object "Amount" do
      attribute :cents, Integer, default: 0
    end
    value_object "Reason" do
      attribute :value, String
    end
    value_object "DepositStatus" do
      attribute :value, String
    end
    attribute :status, DepositStatus, default: "pending" do
      transition "Clear"  => "cleared"
      transition "Bounce" => "bounced"
    end
    command "Make" do
      role "System"
      reference_to Order
      attribute :amount, Amount
      then_set :amount, to: :amount
      emits "DepositMade"
    end
    command "Clear" do
      role "System"
      reference_to Deposit
      emits "DepositCleared"
    end
    command "Bounce" do
      role "System"
      reference_to Deposit
      attribute :reason, Reason
      then_set :reason, to: :reason
      emits "DepositBounced"
    end
    query "Outstanding" do
      where status: "pending"
    end
    query "Bounced" do
      where status: "bounced"
    end
  end
end"#;

const BANKING: &str = r#"Hecks.bluebook "Banking" do
  aggregate "Account" do
    identified_by :name
    attribute :name, AccountName
    attribute :balance, Integer, default: 0
    value_object "AccountName" do
      attribute :value, String
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
      emits "Deposited"
    end
  end
end"#;

const HEX: &str = r#"Hecks.hecksagon "Pizzas" do
  adapter "Deposit" do
    driven on "Pizzas::Order.OrderPlaced" do |sale|
      dispatch "Pizzas::Deposit.Make", order: "{id}", amount: "{total}"
    end
  end
  adapter "Banking" do
    driven on "Pizzas::Deposit.DepositMade" do |deposit|
      dispatch "Banking::Account.Deposit", account: "pizzeria", amount: "{amount}", description: "Pizza sale"
      success "Pizzas::Deposit.Clear"
      failure "Pizzas::Deposit.Bounce"
    end
  end
end"#;

fn booted(open_account: bool) -> Runtime {
    let mut domain = parser::parse(PIZZAS);
    domain.aggregates.extend(parser::parse(BANKING).aggregates);
    let hex = hecksagon_parser::parse(HEX);
    let mut rt = Runtime::boot_with_hecksagons(domain, None, vec![hex]);
    if open_account {
        rt.dispatch("Banking::Account.OpenAccount", attrs(&[("name", s("pizzeria"))]))
            .expect("the pizzeria has an account");
    }
    rt
}

fn sell(rt: &mut Runtime, total: i64) {
    rt.dispatch(
        "Pizzas::Order.PlaceOrder",
        attrs(&[("customer_name", s("Ada")), ("total", Value::Int(total))]),
    )
    .expect("the sale goes through");
}

fn deposits(rt: &Runtime) -> Vec<(String, String)> {
    rt.all("Deposit")
        .iter()
        .map(|r| {
            (
                r.fields.get("status").map(|v| v.to_string()).unwrap_or_default(),
                r.fields.get("amount").map(|v| v.to_string()).unwrap_or_default(),
            )
        })
        .collect()
}

#[test]
fn a_sale_makes_a_deposit_before_the_bank_is_asked() {
    let mut rt = booted(true);
    sell(&mut rt, 1400);

    let d = deposits(&rt);
    assert_eq!(d.len(), 1, "one sale, one deposit — got {d:?}");
    assert_eq!(d[0].1, "1400", "the deposit carries the sale's total");
}

#[test]
fn the_banks_yes_clears_the_deposit() {
    let mut rt = booted(true);
    sell(&mut rt, 1400);

    let d = deposits(&rt);
    assert_eq!(d[0].0, "cleared", "the bank took it — the deposit must clear, got {d:?}");

    let banked = rt
        .all("Account")
        .iter()
        .find(|r| r.id == "pizzeria")
        .and_then(|r| r.fields.get("balance").map(|v| v.to_string()));
    assert_eq!(banked.as_deref(), Some("1400"), "and the money is in the bank");
}

#[test]
fn the_banks_no_bounces_the_deposit_and_the_sale_still_stands() {
    // A zero-dollar sale : Banking's own given (`a deposit moves something`)
    // refuses it, which is a REAL refusal rather than a contrived one.
    let mut rt = booted(true);
    sell(&mut rt, 0);

    assert_eq!(rt.all("Order").len(), 1, "the sale is not rolled back");

    let d = deposits(&rt);
    assert_eq!(
        d[0].0, "bounced",
        "the bank refused — the deposit must be BOUNCED, not silently lost, got {d:?}"
    );
}

#[test]
fn every_refused_sale_leaves_a_bounced_deposit_behind() {
    let mut rt = booted(true);
    sell(&mut rt, 0);
    sell(&mut rt, 0);

    let d = deposits(&rt);
    assert_eq!(d.len(), 2, "two sales, two deposits — got {d:?}");
    assert!(
        d.iter().all(|(status, _)| status == "bounced"),
        "neither reached the bank, so both must be bounced — got {d:?}"
    );
    assert!(
        !d.iter().any(|(status, _)| status == "pending"),
        "nothing may be left pending once the bank has answered — got {d:?}"
    );
}
