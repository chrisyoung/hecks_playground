//! dispatch_detail_tests — the rich-block timeline suite : ANSI stripping,
//! event-trace scope capture, parse_fqn 3-part/domainless forms, header
//! rendering.
//!
//! Cask extracted VERBATIM from runtime/dispatch_detail.rs (cask-runtime) ;
//! body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/dispatch_detail_tests.rs —
//!  kernel-floor timeline tests, relocated verbatim from dispatch_detail.rs
//!  blanket.]

use super::dispatch_detail::*;
use super::dispatch_detail::COLLECTOR;

#[test]
fn strip_ansi_removes_colour_codes() {
    let coloured = format!("{}hello{} world", CYAN, RESET);
    assert_eq!(strip_ansi(&coloured), "hello world");
}

#[test]
fn strip_ansi_leaves_plain_text() {
    assert_eq!(strip_ansi("no codes here"), "no codes here");
}

#[test]
fn colour_for_cascade_fail_is_red() {
    assert_eq!(colour_for("cascade", false), RED);
    assert_eq!(colour_for("cascade", true), GREEN);
}

#[test]
fn colour_for_adapter_ok_is_magenta() {
    assert_eq!(colour_for("adapter", true), MAGENTA);
}

#[test]
fn record_event_noop_without_scope() {
    // No scope open : record_event must not panic and must not
    // leave a collector behind.
    record_event("dispatch", "Foo.Bar", true);
    assert!(!is_in_scope());
}

#[test]
fn parse_fqn_three_parts() {
    assert_eq!(
        parse_fqn("Voice::Voice.Speak"),
        ("Voice".to_string(), "Voice".to_string(), "Speak".to_string())
    );
    assert_eq!(
        parse_fqn("Tools::ShellTool.Bash"),
        ("Tools".to_string(), "ShellTool".to_string(), "Bash".to_string())
    );
}

#[test]
fn parse_fqn_domainless() {
    assert_eq!(
        parse_fqn("Heart.Beat"),
        (String::new(), "Heart".to_string(), "Beat".to_string())
    );
}

#[test]
fn header_line_renders_three_bracketed_parts() {
    let plain = strip_ansi(&header_line("Voice::Voice.Speak"));
    assert_eq!(plain, "[Voice Voice Speak]");
}

#[test]
fn header_line_domainless_has_no_empty_slot() {
    let plain = strip_ansi(&header_line("Heart.Beat"));
    assert_eq!(plain, "[Heart Beat]");
}

#[test]
fn header_line_shows_every_realm_segment() {
    // A deep canonical FQN brackets ALL :: segments — realm + subrealms +
    // domain + aggregate + command — not just Domain::Aggregate.command.
    let plain = strip_ansi(&header_line("Hecks::Framework::Tools::FileTool.Edit"));
    assert_eq!(plain, "[Hecks Framework Tools FileTool Edit]");
}

#[test]
fn scope_collects_events_into_timeline() {
    let mut scope = DispatchScope::begin("inv_t", "Foo::Bar.Baz", "{}".to_string());
    assert!(is_in_scope());
    record_event("dispatch", "Foo::Bar.Baz", true);
    record_event("event", "Bar.Bazzed", true);
    scope.finish("ok", "{}".to_string());
    let events = COLLECTOR.with(|c| c.borrow().clone()).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, "dispatch");
    // Disarm so Drop doesn't emit to the real log during the test.
    scope.armed = false;
    COLLECTOR.with(|c| *c.borrow_mut() = None);
}

#[test]
fn take_last_events_returns_events_after_scope_drop() {
    // Verify LAST_EVENTS is populated by Drop and drained by
    // take_last_events() — the i718 warm-serve fix.
    let mut scope = DispatchScope::begin("inv_u", "A::B.C", "{}".to_string());
    record_event("dispatch", "A::B.C", true);
    record_event("event", "B.Cd", true);
    scope.finish("ok", "{}".to_string());
    drop(scope); // triggers LAST_EVENTS write
    let taken = take_last_events();
    assert_eq!(taken.len(), 2);
    assert_eq!(taken[0].kind, "dispatch");
    assert_eq!(taken[1].kind, "event");
    // Second call must drain (idempotent — returns empty Vec).
    assert!(take_last_events().is_empty());
}
