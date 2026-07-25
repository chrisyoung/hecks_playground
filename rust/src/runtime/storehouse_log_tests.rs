//! storehouse_log_tests — the log suite : level gating, now-token
//! interpolation ({now}/{now-N[smhd]}), ISO-8601 formatting (epoch, leap
//! day), rotation knobs.
//!
//! Cask extracted VERBATIM from runtime/storehouse_log.rs (cask-runtime) ;
//! body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/storehouse_log_tests.rs —
//!  kernel-floor log tests, relocated verbatim from storehouse_log.rs
//!  blanket.]

use super::log_time::{format_iso8601, interpolate_now_tokens, parse_now_token};
use super::storehouse_log::*;
use std::fs::OpenOptions;

#[test]
fn iso8601_unix_epoch_zero() {
    assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
}

#[test]
fn iso8601_known_date() {
    // 2026-05-14T18:42:01Z  =  1778784121 epoch
    // (sanity-checked against `date -u -d @1778784121`)
    assert_eq!(format_iso8601(1_778_784_121), "2026-05-14T18:42:01Z");
}

#[test]
fn iso8601_leap_day() {
    // 2024-02-29T00:00:00Z = 1709164800
    assert_eq!(format_iso8601(1_709_164_800), "2024-02-29T00:00:00Z");
}

// ---- `{now}` token parsing (the clock primitive) ----
// parse_now_token takes an explicit base, so these are clock-free and
// race-free under parallel test threads (no env, no SystemTime).
const BASE: i64 = 1_778_784_121; // 2026-05-14T18:42:01Z

#[test]
fn now_token_bare() {
    assert_eq!(parse_now_token("now", BASE), Some(BASE));
}

#[test]
fn now_token_plus_and_minus() {
    assert_eq!(parse_now_token("now+3600", BASE), Some(BASE + 3600));
    assert_eq!(parse_now_token("now-60", BASE), Some(BASE - 60));
}

#[test]
fn now_token_rejects_non_now() {
    assert_eq!(parse_now_token("id", BASE), None);
    assert_eq!(parse_now_token("now*5", BASE), None);
    assert_eq!(parse_now_token("nowish", BASE), None);
    assert_eq!(parse_now_token("now+", BASE), None);
}

#[test]
fn now_tokens_resolve_to_iso_frozen_by_hecks_now() {
    // HECKS_NOW pins the clock so token output is byte-stable. This is
    // the only env-touching test in the module ; asserted values are
    // computed from the pinned base, never wall-clock.
    std::env::set_var("HECKS_NOW", BASE.to_string());
    assert_eq!(interpolate_now_tokens("{now}"), "2026-05-14T18:42:01Z");
    assert_eq!(interpolate_now_tokens("x={now+3600}"), "x=2026-05-14T19:42:01Z");
    assert_eq!(interpolate_now_tokens("x={now-121}"), "x=2026-05-14T18:40:00Z");
    // Non-now tokens + plain text pass through untouched.
    assert_eq!(interpolate_now_tokens("{id} stays"), "{id} stays");
    std::env::remove_var("HECKS_NOW");
}

#[test]
fn maybe_rotate_shifts_oversized_log_then_reopens_fresh() {
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!("sh_log_rot_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("storehouse.log");

    // Oversized live file (500B) ; hold an append fd to it. cap=100 -> WE rotate.
    std::fs::write(&path, vec![b'x'; 500]).unwrap();
    let mut f = OpenOptions::new().create(true).append(true).open(&path).unwrap();
    maybe_rotate(&mut f, &path, 100, 2);
    assert!(path.with_extension("log.1").exists(), ".1 must hold the rotated content");
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 0, "live log reopened fresh");
    assert_eq!(std::fs::metadata(path.with_extension("log.1")).unwrap().len(), 500);
    // The reopened fd writes to the FRESH file, not the rotated-away one.
    writeln!(f, "after").unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() > 0, "writes land in the fresh file");

    // Sibling-rotated case : our fd points at a big file while the live
    // path is small -> our_len > live_len -> reopen the live path.
    let stale = dir.join("stale.log");
    std::fs::write(&stale, vec![b'y'; 9000]).unwrap();
    let mut g = OpenOptions::new().append(true).open(&stale).unwrap();
    maybe_rotate(&mut g, &path, 100_000, 2); // high cap : no self-rotate
    let before = std::fs::metadata(&path).unwrap().len();
    writeln!(g, "z").unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() > before, "stale fd reopened onto live path");
    assert_eq!(std::fs::metadata(&stale).unwrap().len(), 9000, "stale file untouched after reopen");
    let _ = std::fs::remove_dir_all(&dir);
}
