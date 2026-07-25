//! tests — the claude_tool dispatch suite : bash capture/exit, read/write
//! round-trips + sentinel guards, offset/limit windowing, unknown-tool
//! error, and the glob child-module behaviors. The Tools.Edit suite lives
//! in edit_tests.rs (shares the attrs helper below).
//!
//! Cask extracted VERBATIM from claude_tool_dispatcher/mod.rs
//! (cask-runtime) ; body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/claude_tool_dispatcher/tests.rs —
//!  kernel-floor tool tests, relocated verbatim from mod.rs blanket.]

use super::*;

pub(super) fn attrs(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn bash_echo_runs_and_captures_output() {
    let r = dispatch("bash", &attrs(&[("shell_command", "echo hello")]));
    assert!(r.ok, "{:?}", r);
    assert!(r.output.contains("hello"));
    assert_eq!(r.exit_code, 0);
}

#[test]
fn bash_nonzero_exit_marks_not_ok() {
    let r = dispatch("bash", &attrs(&[("shell_command", "false")]));
    assert!(!r.ok);
    assert_ne!(r.exit_code, 0);
}

#[test]
fn read_write_round_trip() {
    let tmp = format!("/tmp/claude_tool_test_{}.txt", std::process::id());
    let w = dispatch("write", &attrs(&[("file_path", &tmp), ("content", "hi there")]));
    assert!(w.ok, "{:?}", w);
    let r = dispatch("read", &attrs(&[("file_path", &tmp)]));
    assert!(r.ok && r.output == "hi there", "{:?}", r);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn write_rejects_the_empty_list_sentinel_path() {
    // An unset list-shaped file_path renders as the sentinel "[0 items]".
    // write must REFUSE it (missing required attr) — never create a junk file
    // named "[0 items]" in the cwd. Regression for the boot-time dropping a
    // path-less FileTool.Write/Update cascade kept leaving behind.
    for bad in ["[0 items]", ""] {
        let r = dispatch("write", &attrs(&[("file_path", bad), ("content", "x")]));
        assert!(!r.ok, "write must reject path {bad:?}, got: {:?}", r);
        assert!(r.error.as_deref().unwrap_or("").contains("file_path"),
            "error must name file_path, got: {:?}", r);
        assert!(!std::path::Path::new(bad).exists(),
            "write must NOT create a file named {bad:?}");
    }
}

#[test]
fn write_refuses_contentless_write_and_preserves_the_file() {
    // The data-loss vector : a spurious cascade re-fires the :write
    // binding off accumulated FileTool state, which carries a stale
    // file_path but NO content (content is event-only, never persisted).
    // run_write must REFUSE (content absent) and leave the file intact,
    // instead of fs::write(path, "") truncating it. Regression for the
    // FileTool.Read-with-offset truncation that emptied a 330-line file.
    let tmp = format!("/tmp/claude_tool_noclobber_{}.txt", std::process::id());
    std::fs::write(&tmp, "precious\ndata\n").unwrap();
    // No `content` key at all -> must refuse, must NOT touch the file.
    let r = dispatch("write", &attrs(&[("file_path", &tmp)]));
    assert!(!r.ok, "content-less write must be refused, got: {:?}", r);
    assert!(r.error.as_deref().unwrap_or("").contains("content"),
        "error must name content, got: {:?}", r);
    assert_eq!(std::fs::read_to_string(&tmp).unwrap(), "precious\ndata\n",
        "the file must be untouched by a refused write");
    // An INTENTIONAL empty write (content present, "") still works.
    let w = dispatch("write", &attrs(&[("file_path", &tmp), ("content", "")]));
    assert!(w.ok, "explicit empty-content write must succeed, got: {:?}", w);
    assert_eq!(std::fs::read_to_string(&tmp).unwrap(), "",
        "explicit empty write truncates intentionally");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn read_honors_offset_and_limit() {
    let tmp = format!("/tmp/claude_tool_window_{}.txt", std::process::id());
    std::fs::write(&tmp, "l1\nl2\nl3\nl4\nl5").unwrap();
    // offset=2, limit=2 -> 1-based lines 2..=3
    let r = dispatch("read", &attrs(&[("file_path", &tmp), ("offset", "2"), ("limit", "2")]));
    assert!(r.ok, "{:?}", r);
    assert_eq!(r.output, "l2\nl3", "windowed read returns only the requested lines");
    // offset only -> from line 4 to end
    let r2 = dispatch("read", &attrs(&[("file_path", &tmp), ("offset", "4")]));
    assert_eq!(r2.output, "l4\nl5");
    // neither -> whole file (backward compatible)
    let r3 = dispatch("read", &attrs(&[("file_path", &tmp)]));
    assert_eq!(r3.output, "l1\nl2\nl3\nl4\nl5");
    // offset past EOF -> empty, not a panic
    let r4 = dispatch("read", &attrs(&[("file_path", &tmp), ("offset", "99")]));
    assert!(r4.ok && r4.output.is_empty(), "{:?}", r4);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn unknown_tool_returns_error() {
    let r = dispatch("nonsense", &attrs(&[]));
    assert!(!r.ok);
    assert!(r.error.unwrap().contains("unknown tool kind"));
}

// ── glob coverage ── (translation rules tested in glob.rs sibling)

// Glob tests build a self-contained tempdir tree so they don't
// depend on the cargo-test cwd (other tests in the suite mutate
// process cwd, e.g. statusline tests chdir to /tmp).
fn build_glob_fixture() -> String {
    let root = format!("/tmp/claude_tool_glob_{}_{}",
        std::process::id(),
        crate::clock::now_duration().as_nanos());
    std::fs::create_dir_all(format!("{}/runtime", root)).unwrap();
    std::fs::create_dir_all(format!("{}/runtime/nested", root)).unwrap();
    std::fs::write(format!("{}/runtime/alpha.rs", root), "// alpha\n").unwrap();
    std::fs::write(format!("{}/runtime/beta.rs",  root), "// beta\n").unwrap();
    std::fs::write(format!("{}/runtime/gamma.rs", root), "// gamma\n").unwrap();
    std::fs::write(format!("{}/runtime/nested/deep.rs", root), "// deep\n").unwrap();
    root
}

#[test]
fn glob_directory_pattern_finds_files_in_dir() {
    let root = build_glob_fixture();
    let pat = format!("{}/runtime/*.rs", root);
    let r = dispatch("glob", &attrs(&[("glob_pattern", &pat)]));
    let _ = std::fs::remove_dir_all(&root);
    assert!(r.ok, "{:?}", r);
    let lines: Vec<&str> = r.output.lines().collect();
    assert_eq!(lines.len(), 3, "expected 3 top-level .rs files, got {:?}", lines);
    assert!(r.output.contains("alpha.rs") && r.output.contains("beta.rs") && r.output.contains("gamma.rs"));
    // Non-recursive must NOT descend into nested/.
    assert!(!r.output.contains("deep.rs"), "non-recursive walk should skip nested/, got {:?}", r.output);
}

#[test]
fn glob_recursive_pattern_descends() {
    let root = build_glob_fixture();
    let pat = format!("{}/runtime/**/*.rs", root);
    let r = dispatch("glob", &attrs(&[("glob_pattern", &pat)]));
    let _ = std::fs::remove_dir_all(&root);
    assert!(r.ok, "{:?}", r);
    assert!(r.output.contains("deep.rs"), "recursive walk should find nested file, got {:?}", r.output);
    assert!(r.output.contains("alpha.rs"), "recursive walk should also find top-level, got {:?}", r.output);
}

#[test]
fn glob_empty_match_is_ok_not_error() {
    let root = build_glob_fixture();
    let pat = format!("{}/runtime/*.nonexistent-extension-xyz", root);
    let r = dispatch("glob", &attrs(&[("glob_pattern", &pat)]));
    let _ = std::fs::remove_dir_all(&root);
    assert!(r.ok, "no-match must be ok=true, got {:?}", r);
    assert_eq!(r.output.trim(), "");
}

#[test]
fn glob_ignores_empty_list_sentinel_in_search_path() {
    // The runtime snapshots the whole aggregate state into attrs, so
    // unset list-shaped attributes arrive as the literal "[0 items]"
    // rendering. The dispatcher must treat that as absent — not feed
    // the literal "[0 items]" to find as a directory.
    let root = build_glob_fixture();
    let pat = format!("{}/runtime/*.rs", root);
    let r = dispatch("glob", &attrs(&[
        ("glob_pattern", &pat),
        ("search_path",  "[0 items]"),
    ]));
    let _ = std::fs::remove_dir_all(&root);
    assert!(r.ok, "sentinel must be ignored, got {:?}", r);
    assert!(r.output.contains("alpha.rs"), "expected fixture files, got {:?}", r.output);
}

#[test]
fn glob_missing_directory_surfaces_stderr() {
    // Pointing at a directory that doesn't exist — find exits non-zero
    // and writes to stderr. Verify we capture it in the error message.
    let missing = format!("/tmp/nonexistent_dir_xyz_{}", std::process::id());
    let pat = format!("{}/*.rs", missing);
    let r = dispatch("glob", &attrs(&[("glob_pattern", &pat)]));
    assert!(!r.ok, "missing dir should fail: {:?}", r);
    let msg = r.error.unwrap_or_default();
    assert!(
        msg.contains("nonexistent_dir_xyz") || msg.to_lowercase().contains("no such"),
        "error message should mention the missing dir or 'no such', got {}", msg
    );
}
