//! sqlite_bluebook_query_parity_test — hook the sqlite adapter up to a bluebook
//! and prove it answers EVERY query exactly as the canonical in-memory oracle.
//!
//! Two lower layers already exist : sqlite_scope_test proves persistence
//! (save/reload through the runtime), and the crate's query_parity_tests prove
//! every WhereOp at the repository seam. This is the layer ABOVE both — named
//! bluebook `query` declarations resolved THROUGH THE RUNTIME on a sqlite-backed
//! aggregate, asserted identical to the same bluebook on the memory backend over
//! identical data. Whatever a bluebook query can say — an all-scan, a single Eq
//! on the lifecycle field, a kwarg-param Eq, an Eq on a typed INTEGER column, a
//! multi-clause Eq AND, or order_by + limit — the adapter must answer it the way
//! the oracle does. "Hook up the adapter and it just works", proven for the whole
//! query surface, entirely outside the generic runtime (this test IS the
//! composition root : it calls storehouse_sqlite::register() itself).

use storehouse::hecksagon_parser;
use storehouse::parser;
use storehouse::runtime::{repo_key, Runtime, Value};
use std::collections::HashMap;

const SUPPORT: &str = r#"Hecks.bluebook "Support" do
  aggregate "Ticket" do
    attribute :title, String
    attribute :assignee, String
    attribute :priority, Integer
    attribute :status, TicketStatus, default: "open" do
      transition "Close" => "closed"
    end
    value_object "TicketStatus" do
      attribute :value, String
    end
    command "Open" do
      attribute :title, String
      attribute :assignee, String
      attribute :priority, Integer
    end
    command "Close" do
      reference_to Ticket
    end
    query "AllTickets" do
    end
    query "OpenTickets" do
      where(status: "open")
    end
    query "ByAssignee" do |assignee|
      where(assignee: assignee)
    end
    query "ByPriority" do |priority|
      where(priority: priority)
    end
    query "OpenForAssignee" do |assignee|
      where(status: "open", assignee: assignee)
    end
    query "ByTitle" do
      order_by :title
      limit 3
    end
  end
end"#;

fn hm(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// The titles in a resolve_query result, in the runtime's returned order. Title
/// is the record's stable unique identity here (the query record JSON serialises
/// the aggregate's `fields`, and `id` is a struct field OUTSIDE that map, so it
/// is not a JSON key ; title is, and each ticket's is distinct).
fn titles(v: &serde_json::Value) -> Vec<String> {
    match v["state"].as_array() {
        Some(rows) => rows
            .iter()
            .filter_map(|r| r["title"].as_str().map(String::from))
            .collect(),
        None => vec![],
    }
}

fn sorted(mut xs: Vec<String>) -> Vec<String> {
    xs.sort();
    xs
}

/// Sorted titles a query returns — the membership identity for a parity check.
fn set(rt: &Runtime, name: &str, attrs: &HashMap<String, String>) -> Vec<String> {
    sorted(titles(&rt.resolve_query(name, attrs)))
}

/// Seed five tickets, then Close the third — the SAME writes for both backends.
fn seed(rt: &mut Runtime) -> Vec<String> {
    let rows = [
        ("Login bug", "alice", 1),
        ("Crash on save", "bob", 5),
        ("Typo", "alice", 2),
        ("Slow query", "carol", 9),
        ("Auth leak", "bob", 10),
    ];
    let mut ids = Vec::new();
    for (title, who, pri) in rows {
        let mut a = HashMap::new();
        a.insert("title".to_string(), Value::Str(title.to_string()));
        a.insert("assignee".to_string(), Value::Str(who.to_string()));
        a.insert("priority".to_string(), Value::Int(pri as i64));
        let r = rt.dispatch("Support::Ticket.Open", a).expect("Open persists");
        ids.push(r.aggregate_id.clone());
    }
    // Close "Typo" (index 2) : open -> closed, so it leaves the open set.
    let mut c = HashMap::new();
    c.insert("id".to_string(), Value::Str(ids[2].clone()));
    rt.dispatch("Support::Ticket.Close", c).expect("Close transitions");
    ids
}

#[test]
fn sqlite_answers_every_bluebook_query_like_the_memory_oracle() {
    storehouse_sqlite::register();

    let db = std::env::temp_dir()
        .join(format!("sqlite_query_parity_{}.db", std::process::id()))
        .to_string_lossy()
        .into_owned();
    let _ = std::fs::remove_file(&db);
    let sqlite_hex =
        format!("Hecks.hecksagon \"Support\" do\n  adapter :sqlite, db: \"{db}\"\nend\n");

    // Two runtimes, same domain : one sqlite-backed (the adapter under test), one
    // memory-backed (the canonical where_matches oracle).
    let mut sql = Runtime::boot_with_hecksagons(
        parser::parse(SUPPORT),
        None,
        vec![hecksagon_parser::parse(&sqlite_hex)],
    );
    let mut mem = Runtime::boot_in_memory(parser::parse(SUPPORT));

    // The aggregate under test really IS SQL-backed — never a silent heki swap.
    assert!(
        sql.repositories
            .get(&repo_key(Some("Support"), "Ticket"))
            .map(|r| r.is_adapter())
            .unwrap_or(false),
        "Ticket must be sqlite-backed by its :sqlite hecksagon",
    );

    seed(&mut sql);
    seed(&mut mem);

    // Every query shape a bluebook can express, resolved through the runtime on
    // BOTH backends. For each, sqlite must (1) return the concrete right rows and
    // (2) match the memory oracle exactly — so a shared bug can't hide behind
    // empty == empty.
    let cases: &[(&str, HashMap<String, String>, &[&str])] = &[
        // all-scan
        ("AllTickets", hm(&[]),
            &["Auth leak", "Crash on save", "Login bug", "Slow query", "Typo"]),
        // single Eq on the lifecycle field — "Typo" was closed, so it drops out
        ("OpenTickets", hm(&[]),
            &["Auth leak", "Crash on save", "Login bug", "Slow query"]),
        // kwarg-param Eq on a String column
        ("ByAssignee", hm(&[("assignee", "alice")]), &["Login bug", "Typo"]),
        // Eq on a typed INTEGER column (the param arrives as the string "5")
        ("ByPriority", hm(&[("priority", "5")]), &["Crash on save"]),
        // multi-clause Eq AND — alice's only OPEN ticket ("Typo" is closed)
        ("OpenForAssignee", hm(&[("assignee", "alice")]), &["Login bug"]),
    ];
    for (q, attrs, expected) in cases {
        let want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
        assert_eq!(set(&sql, q, attrs), want, "query `{q}` : sqlite returned the wrong rows");
        assert_eq!(
            set(&sql, q, attrs),
            set(&mem, q, attrs),
            "query `{q}` : sqlite must match the memory oracle",
        );
    }

    // order_by + limit : identity in ORDER, not just membership — and the concrete
    // ascending-title, top-3 answer.
    let s_ord = titles(&sql.resolve_query("ByTitle", &HashMap::new()));
    let m_ord = titles(&mem.resolve_query("ByTitle", &HashMap::new()));
    assert_eq!(s_ord, m_ord, "ByTitle : sqlite must match the oracle's order + limit");
    assert_eq!(
        s_ord,
        vec!["Auth leak", "Crash on save", "Login bug"],
        "ByTitle : order_by title asc, limit 3",
    );
}
