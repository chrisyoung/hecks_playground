//! edit_tests — the Tools.Edit suite (i559) : happy paths (single-line +
//! multi-line, tolerant-indentation matching) and the distinct error
//! surfaces (missing file, old-string-not-found, ambiguous match, missing
//! attr, empty-list sentinel). Shares the attrs helper from tests.rs.
//!
//! Cask extracted VERBATIM from claude_tool_dispatcher/mod.rs
//! (cask-runtime) ; body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/claude_tool_dispatcher/edit_tests.rs
//!  — kernel-floor tool tests, relocated verbatim from mod.rs blanket.]

use super::tests::attrs;
use super::*;

#[test]
fn edit_replaces_unique_substring() {
    let tmp = format!("/tmp/claude_tool_edit_{}.txt", std::process::id());
    std::fs::write(&tmp, "alpha beta gamma").unwrap();
    let e = dispatch("edit", &attrs(&[
        ("file_path", &tmp),
        ("old_string", "beta"),
        ("new_string", "BETA"),
    ]));
    assert!(e.ok, "{:?}", e);
    assert_eq!(std::fs::read_to_string(&tmp).unwrap(), "alpha BETA gamma");
    let _ = std::fs::remove_file(&tmp);
}

// ── i559 — Tools.Edit diagnostic + behavior tests ──
//
// The Tools.Edit kernel hook used to fail silently with
// `ok=false exit=1 output=""` whenever attrs were incomplete or
// the file content didn't match. These tests pin both the happy
// paths (single-line + multi-line) and the four distinct error
// surfaces : missing-file, old-string-not-found, ambiguous match,
// and (covered upstream) missing required attr.

#[test]
fn edit_single_line_happy_path() {
    let tmp = format!("/tmp/claude_tool_edit_single_{}.txt", std::process::id());
    std::fs::write(&tmp, "alpha foo gamma").unwrap();
    let r = dispatch("edit", &attrs(&[
        ("file_path", &tmp),
        ("old_string", "foo"),
        ("new_string", "BAR"),
    ]));
    assert!(r.ok, "{:?}", r);
    assert_eq!(std::fs::read_to_string(&tmp).unwrap(), "alpha BAR gamma");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn edit_multi_line_happy_path() {
    // Mirrors the real-world failure case : a multi-line CSS
    // block where Tools.Edit was returning ok=false exit=1
    // output="" because the original dispatch attrs never made
    // it to the kernel hook (they don't live on aggregate state
    // by design, per the Tools bluebook).
    let tmp = format!("/tmp/claude_tool_edit_multi_{}.txt", std::process::id());
    let original = "    .mark-wrap {\n      display: inline-block;\n      position: relative;\n      width: 9rem;\n    }\n";
    let updated  = "    .mark-wrap {\n      display: inline-block;\n      width: 9rem;\n    }\n";
    std::fs::write(&tmp, original).unwrap();
    let old = "    .mark-wrap {\n      display: inline-block;\n      position: relative;\n      width: 9rem;\n    }";
    let new = "    .mark-wrap {\n      display: inline-block;\n      width: 9rem;\n    }";
    let r = dispatch("edit", &attrs(&[
        ("file_path", &tmp),
        ("old_string", old),
        ("new_string", new),
    ]));
    assert!(r.ok, "{:?}", r);
    assert_eq!(std::fs::read_to_string(&tmp).unwrap(), updated);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn edit_old_string_not_found_returns_clear_error() {
    let tmp = format!("/tmp/claude_tool_edit_missing_{}.txt", std::process::id());
    std::fs::write(&tmp, "hello world\nsecond line\n").unwrap();
    let r = dispatch("edit", &attrs(&[
        ("file_path", &tmp),
        ("old_string", "absent_substring"),
        ("new_string", "irrelevant"),
    ]));
    assert!(!r.ok);
    let msg = r.error.expect("error message present");
    assert!(msg.contains("old_string not found"), "{}", msg);
    assert!(msg.contains(&tmp), "path missing from error: {}", msg);
    assert!(msg.contains("first 200 chars"), "diagnostic preview missing: {}", msg);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn edit_old_string_matches_twice_returns_not_unique_error() {
    let tmp = format!("/tmp/claude_tool_edit_dup_{}.txt", std::process::id());
    std::fs::write(&tmp, "dup\nmiddle\ndup\n").unwrap();
    let r = dispatch("edit", &attrs(&[
        ("file_path", &tmp),
        ("old_string", "dup"),
        ("new_string", "DUP"),
    ]));
    assert!(!r.ok);
    let msg = r.error.expect("error message present");
    assert!(msg.contains("2 times") || msg.contains("appears 2"),
            "expected match-count in error: {}", msg);
    assert!(msg.contains("unique") || msg.contains("replace_all"),
            "expected guidance in error: {}", msg);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn edit_file_does_not_exist_returns_clear_error() {
    let absent = format!("/tmp/claude_tool_edit_never_{}_nope.txt", std::process::id());
    // Ensure it really doesn't exist.
    let _ = std::fs::remove_file(&absent);
    let r = dispatch("edit", &attrs(&[
        ("file_path", &absent),
        ("old_string", "anything"),
        ("new_string", "irrelevant"),
    ]));
    assert!(!r.ok);
    let msg = r.error.expect("error message present");
    assert!(msg.contains("file not found"), "{}", msg);
    assert!(msg.contains(&absent), "path missing from error: {}", msg);
}

#[test]
fn edit_missing_old_string_attr_returns_clear_error() {
    let tmp = format!("/tmp/claude_tool_edit_noattr_{}.txt", std::process::id());
    std::fs::write(&tmp, "anything").unwrap();
    let r = dispatch("edit", &attrs(&[
        ("file_path", &tmp),
        // old_string omitted on purpose
    ]));
    assert!(!r.ok);
    assert!(r.error.unwrap().contains("missing required attr: old_string"));
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn edit_treats_empty_list_sentinel_as_missing() {
    // When attrs are snapshotted from aggregate state and the
    // attribute is unset, the runtime renders Value::List(vec![])
    // as "[0 items]". The kernel hook must treat that sentinel
    // as "not provided", not as a literal value to search for.
    let tmp = format!("/tmp/claude_tool_edit_sentinel_{}.txt", std::process::id());
    std::fs::write(&tmp, "hello").unwrap();
    let r = dispatch("edit", &attrs(&[
        ("file_path", &tmp),
        ("old_string", "[0 items]"),
        ("new_string", "x"),
    ]));
    assert!(!r.ok);
    assert!(r.error.unwrap().contains("missing required attr: old_string"));
    let _ = std::fs::remove_file(&tmp);
}

// ── whitespace-tolerant matching ──

#[test]
fn edit_tolerant_matches_despite_wrong_indentation() {
    // File is indented 12 spaces ; old_string is authored with 8. Exact
    // match fails ; the tolerant fallback matches by trimmed line content
    // and re-indents new_string to the file's real 12-space indent.
    let tmp = format!("/tmp/claude_tool_edit_tol_{}.txt", std::process::id());
    std::fs::write(&tmp, "fn f() {\n            let x = 1;\n            let y = 2;\n}\n").unwrap();
    let r = dispatch("edit", &attrs(&[
        ("file_path", &tmp),
        ("old_string", "        let x = 1;\n        let y = 2;"),
        ("new_string", "        let x = 10;\n        let y = 20;"),
    ]));
    assert!(r.ok, "tolerant edit should succeed: {:?}", r);
    assert_eq!(
        std::fs::read_to_string(&tmp).unwrap(),
        "fn f() {\n            let x = 10;\n            let y = 20;\n}\n",
        "new_string must be re-indented to the file's 12-space indent",
    );
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn edit_tolerant_ambiguous_is_refused() {
    // Two trimmed-equal lines ; old_string (tab-prefixed) has no exact
    // match, so the tolerant path runs and must refuse the ambiguous
    // match rather than corrupt the file.
    let tmp = format!("/tmp/claude_tool_edit_tolamb_{}.txt", std::process::id());
    std::fs::write(&tmp, "  foo();\n  foo();\n").unwrap();
    let r = dispatch("edit", &attrs(&[
        ("file_path", &tmp),
        ("old_string", "\tfoo();"),
        ("new_string", "bar();"),
    ]));
    assert!(!r.ok, "ambiguous tolerant match must be refused: {:?}", r);
    assert!(r.error.unwrap().contains("appears 2 times"));
    let _ = std::fs::remove_file(&tmp);
}
