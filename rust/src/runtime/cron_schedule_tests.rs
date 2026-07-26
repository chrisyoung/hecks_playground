//! cron_schedule_tests — the five-field cron parser suite : field ranges,
//! steps, lists, wildcards, next-fire computation, malformed rejection.
//!
//! Cask extracted VERBATIM from runtime/cron_schedule.rs (cask-runtime) ;
//! body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/cron_schedule_tests.rs —
//!  kernel-floor cron tests, relocated verbatim from cron_schedule.rs
//!  blanket.]

use super::cron_schedule::*;
use super::cron_schedule::civil_fields;

// Reference instants (UTC), confirmed against the civil-date algorithm :
//   1718928000 = 2024-06-21 00:00:00 UTC (a Friday, dow=5)
//   1718928300 = 2024-06-21 00:05:00 UTC
//   1718929800 = 2024-06-21 00:30:00 UTC
//   1718931600 = 2024-06-21 01:00:00 UTC
const FRI_0000: u64 = 1718928000;
const FRI_0005: u64 = 1718928300;
const FRI_0030: u64 = 1718929800;
const FRI_0100: u64 = 1718931600;

#[test]
fn civil_fields_decomposes_correctly() {
    assert_eq!(civil_fields(FRI_0000), (0, 0, 21, 6, 5)); // Fri 2024-06-21 00:00
    assert_eq!(civil_fields(FRI_0030), (30, 0, 21, 6, 5));
    assert_eq!(civil_fields(FRI_0100), (0, 1, 21, 6, 5));
    // 1970-01-01 00:00:00 was a Thursday (dow 4).
    assert_eq!(civil_fields(0), (0, 0, 1, 1, 4));
}

#[test]
fn star_matches_every_minute() {
    assert!(is_due("* * * * *", FRI_0000));
    assert!(is_due("* * * * *", FRI_0005));
    assert!(is_due("* * * * *", FRI_0030));
}

#[test]
fn step_every_five_minutes() {
    assert!(is_due("*/5 * * * *", FRI_0000)); // minute 0
    assert!(is_due("*/5 * * * *", FRI_0005)); // minute 5
    assert!(is_due("*/5 * * * *", FRI_0030)); // minute 30
    // minute 1,2,3,4 must NOT match */5.
    assert!(!is_due("*/5 * * * *", FRI_0000 + 60));
    assert!(!is_due("*/5 * * * *", FRI_0000 + 180));
}

#[test]
fn exact_minute_and_hour() {
    assert!(is_due("0 1 * * *", FRI_0100));   // 01:00
    assert!(!is_due("0 1 * * *", FRI_0000));  // 00:00 — hour mismatch
    assert!(!is_due("30 0 * * *", FRI_0000)); // minute mismatch
    assert!(is_due("30 0 * * *", FRI_0030));
}

#[test]
fn comma_list_of_minutes() {
    assert!(is_due("0,30 * * * *", FRI_0000));
    assert!(is_due("0,30 * * * *", FRI_0030));
    assert!(!is_due("0,30 * * * *", FRI_0005));
}

#[test]
fn range_of_minutes() {
    assert!(is_due("0-10 * * * *", FRI_0000));
    assert!(is_due("0-10 * * * *", FRI_0005));
    assert!(!is_due("0-10 * * * *", FRI_0030)); // 30 outside 0-10
}

#[test]
fn stepped_range() {
    // 0-30/10 -> minutes 0,10,20,30.
    assert!(is_due("0-30/10 * * * *", FRI_0000));
    assert!(is_due("0-30/10 * * * *", FRI_0030));
    assert!(!is_due("0-30/10 * * * *", FRI_0005));
}

#[test]
fn month_and_dom_fields() {
    assert!(is_due("* * 21 6 *", FRI_0000));   // 21st June
    assert!(!is_due("* * 22 6 *", FRI_0000));  // 22nd — dom mismatch
    assert!(!is_due("* * 21 7 *", FRI_0000));  // July — month mismatch
}

#[test]
fn day_of_week_field() {
    assert!(is_due("* * * * 5", FRI_0000));  // Friday
    assert!(!is_due("* * * * 1", FRI_0000)); // Monday
    // Sunday accepted as both 0 and 7. 1718841600 = 2024-06-20? verify Sunday:
    // 2024-06-23 00:00 UTC = 1719100800, a Sunday (dow 0).
    let sunday = 1719100800u64;
    assert_eq!(civil_fields(sunday).4, 0);
    assert!(is_due("* * * * 0", sunday));
    assert!(is_due("* * * * 7", sunday));
}

#[test]
fn day_or_semantics_union_when_both_restricted() {
    // dom=21 OR dow=1(Mon). On Fri the 21st : dom matches, dow does not ->
    // union is true.
    assert!(is_due("* * 21 * 1", FRI_0000));
    // dom=22 OR dow=5(Fri). On Fri the 21st : dom no, dow yes -> union true.
    assert!(is_due("* * 22 * 5", FRI_0000));
    // dom=22 OR dow=1. Neither matches Fri the 21st -> false.
    assert!(!is_due("* * 22 * 1", FRI_0000));
}

#[test]
fn dom_and_dow_anded_when_one_is_star() {
    // dom=21, dow=* -> plain AND ; matches on the 21st.
    assert!(is_due("* * 21 * *", FRI_0000));
    // dom=*, dow=5 -> AND ; matches on Friday.
    assert!(is_due("* * * * 5", FRI_0000));
    // dom=22, dow=* -> AND ; 22 != 21 -> false (NOT union with the * dow).
    assert!(!is_due("* * 22 * *", FRI_0000));
}

#[test]
fn malformed_never_matches() {
    assert!(!is_due("* * * *", FRI_0000));        // 4 fields
    assert!(!is_due("* * * * * *", FRI_0000));    // 6 fields
    assert!(!is_due("", FRI_0000));               // empty
    assert!(!is_due("*/0 * * * *", FRI_0000));    // zero step
    assert!(!is_due("abc * * * *", FRI_0000));    // non-numeric
}

#[test]
fn should_fire_dedups_within_a_minute_then_refires_next_match() {
    // Simulate the `storehouse drive` poll loop : poll every 15s across a
    // "*/5 * * * *" handler. The handler must fire ONCE when minute 0
    // arrives, stay silent for the rest of that minute despite repeated
    // due-polls, and fire again only when the NEXT matching minute (5)
    // arrives. `last` mirrors the daemon's recorded minute bucket.
    let expr = "*/5 * * * *";
    let mut last: Option<u64> = None;
    let mut fires: Vec<u64> = Vec::new();

    // Poll the whole 00:00:00 minute at 15s cadence (4 polls). All are
    // `is_due` (minute 0 matches */5) but only the FIRST should fire.
    for sec in [0u64, 15, 30, 45] {
        if let Some(bucket) = should_fire(expr, FRI_0000 + sec, last) {
            last = Some(bucket);
            fires.push(FRI_0000 + sec);
        }
    }
    assert_eq!(fires, vec![FRI_0000], "fires exactly once in minute 0");

    // Minute 1-4 : NOT due (*/5), so no fire regardless of poll.
    for min in 1..=4u64 {
        assert!(should_fire(expr, FRI_0000 + min * 60, last).is_none());
    }

    // Minute 5 : due again, new bucket -> fires once more.
    let m5 = FRI_0000 + 5 * 60;
    assert!(should_fire(expr, m5, last).is_some(), "refires at minute 5");
    if let Some(bucket) = should_fire(expr, m5, last) {
        last = Some(bucket);
    }
    // ... and dedups within minute 5 too.
    assert!(should_fire(expr, m5 + 30, last).is_none());
}

#[test]
fn should_fire_skips_non_matching_minutes_entirely() {
    // "30 * * * *" fires at minute 30 of every hour. Polling minute 0 must
    // never fire ; minute 30 fires once.
    let expr = "30 * * * *";
    assert!(should_fire(expr, FRI_0000, None).is_none());      // minute 0
    assert!(should_fire(expr, FRI_0030, None).is_some());      // minute 30
    // Once fired, the same minute won't refire.
    let b = should_fire(expr, FRI_0030, None).unwrap();
    assert!(should_fire(expr, FRI_0030 + 20, Some(b)).is_none());
}

#[test]
fn minute_bucket_is_floor_division() {
    assert_eq!(minute_bucket(FRI_0000), FRI_0000 / 60);
    assert_eq!(minute_bucket(FRI_0000 + 59), FRI_0000 / 60); // same minute
    assert_eq!(minute_bucket(FRI_0000 + 60), FRI_0000 / 60 + 1); // next minute
}

#[test]
fn realistic_combined_expression() {
    // "30 0 21 6 *" — 00:30 on June 21st, any weekday.
    assert!(is_due("30 0 21 6 *", FRI_0030));
    assert!(!is_due("30 0 21 6 *", FRI_0000)); // minute 0, not 30
    assert!(!is_due("30 0 21 6 *", FRI_0100)); // hour 1, not 0
}
