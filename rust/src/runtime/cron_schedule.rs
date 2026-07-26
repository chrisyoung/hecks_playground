//! cron_schedule — the pure 5-field cron-expression matcher for
//! `driving on cron` Drivers fired by `storehouse drive`.
//!
//! Where `drive_scheduler` is the pure tick-counter for the INTERVAL kind,
//! this is its sibling for the CRON kind : given a 5-field cron string and a
//! wall-clock instant, decide whether the expression is DUE at that minute.
//! Pure + heavily table-tested : no Runtime, no IO, no live clock — the
//! caller (`run_drive`) supplies `now` (from `crate::clock::now_duration`,
//! pinnable via `HECKS_NOW` for deterministic tests).
//!
//! Supported syntax per field (minute hour day-of-month month day-of-week) :
//!   `*`      — any value
//!   `*/N`    — every N (step over the field's full range)
//!   `N`      — exact value
//!   `N-M`    — inclusive range
//!   `A,B,C`  — comma list (each element may itself be `*`, `N`, `N-M`, `*/N`,
//!              or a stepped range `N-M/S`)
//! Field ranges : minute 0-59, hour 0-23, dom 1-31, month 1-12, dow 0-6
//! (Sunday = 0 ; 7 is accepted as Sunday too). Standard cron day-or semantics :
//! when BOTH day-of-month and day-of-week are restricted (neither `*`), the
//! match is their UNION ; otherwise the AND of all five fields.
//!
//! Usage :
//!   let due = cron_schedule::is_due("*/5 * * * *",
//!                                   crate::clock::now_duration().as_secs());
//!   // true on minutes 0,5,10,... of every hour.

/// Whether `cron_expr` matches the wall-clock minute containing `now_secs`
/// (Unix epoch seconds, UTC). A malformed expression never matches (returns
/// false) — the daemon treats an unparseable cron as "never due" rather than
/// panicking the whole drive loop.
pub fn is_due(cron_expr: &str, now_secs: u64) -> bool {
    let fields: Vec<&str> = cron_expr.split_whitespace().collect();
    if fields.len() != 5 {
        return false;
    }
    let (minute, hour, dom, month, dow) = civil_fields(now_secs);

    let min_ok = field_matches(fields[0], minute, 0, 59);
    let hour_ok = field_matches(fields[1], hour, 0, 23);
    let mon_ok = field_matches(fields[3], month, 1, 12);
    if !(min_ok && hour_ok && mon_ok) {
        return false;
    }

    // Day-or semantics : if both dom and dow are restricted, match the UNION ;
    // otherwise AND them with the rest (a `*` field is always satisfied).
    let dom_restricted = fields[2] != "*";
    let dow_restricted = fields[4] != "*";
    let dom_ok = field_matches(fields[2], dom, 1, 31);
    let dow_ok = dow_field_matches(fields[4], dow);
    if dom_restricted && dow_restricted {
        dom_ok || dow_ok
    } else {
        dom_ok && dow_ok
    }
}

/// The minute-bucket an instant falls in : `now_secs / 60`. The drive daemon
/// keys its per-handler last-fired mark on this so a sub-minute poll fires a
/// due expression at most once per minute. Exposed as the single source of the
/// minute boundary so the daemon and the tests agree on it.
pub fn minute_bucket(now_secs: u64) -> u64 {
    now_secs / 60
}

/// Whole live firing decision for one cron handler, kept pure so the
/// minute-dedup — the key correctness concern — is unit-testable without the
/// daemon's infinite poll loop. Fires iff the expression is DUE at `now_secs`
/// AND this handler has not already fired in the current minute bucket.
/// `last_fired_minute` is the daemon's recorded bucket for this handler (None
/// = never fired). Returns the new bucket to record when it decides to fire,
/// so the caller updates its map in lock-step :
///   if let Some(min) = should_fire(expr, now, last) { map.insert(id, min); .. }
pub fn should_fire(cron_expr: &str, now_secs: u64, last_fired_minute: Option<u64>) -> Option<u64> {
    if !is_due(cron_expr, now_secs) {
        return None;
    }
    let bucket = minute_bucket(now_secs);
    if last_fired_minute == Some(bucket) {
        return None; // already fired this minute — dedup the sub-minute poll
    }
    Some(bucket)
}

/// Match a single cron field against a value, given that field's inclusive
/// `[lo, hi]` range. Splits on commas ; each element is matched independently
/// and the field matches if ANY element does.
fn field_matches(field: &str, value: u32, lo: u32, hi: u32) -> bool {
    field.split(',').any(|elem| element_matches(elem.trim(), value, lo, hi))
}

/// Day-of-week wrapper : normalises both the cron `7` (Sunday) and the
/// computed dow so 7 and 0 are interchangeable, then matches over 0-6.
fn dow_field_matches(field: &str, dow: u32) -> bool {
    field.split(',').any(|raw| {
        let elem = raw.trim();
        // `7` anywhere is Sunday : a literal 7 element matches dow 0.
        if elem == "7" && dow == 0 {
            return true;
        }
        element_matches(elem, dow, 0, 6)
    })
}

/// Match one non-comma element : `*`, `*/N`, `N`, `N-M`, or `N-M/S`.
fn element_matches(elem: &str, value: u32, lo: u32, hi: u32) -> bool {
    if elem == "*" {
        return true;
    }
    // Step form : `<base>/<step>` where base is `*` or a range.
    if let Some((base, step_s)) = elem.split_once('/') {
        let step: u32 = match step_s.parse() {
            Ok(s) if s > 0 => s,
            _ => return false,
        };
        let (rlo, rhi) = match range_bounds(base, lo, hi) {
            Some(b) => b,
            None => return false,
        };
        return value >= rlo && value <= rhi && (value - rlo).is_multiple_of(step);
    }
    // Range form : `N-M`.
    if let Some((a, b)) = elem.split_once('-') {
        return match (a.parse::<u32>(), b.parse::<u32>()) {
            (Ok(a), Ok(b)) => value >= a && value <= b,
            _ => false,
        };
    }
    // Exact value.
    matches!(elem.parse::<u32>(), Ok(n) if n == value)
}

/// The inclusive bounds a step's base spans : `*` -> the field range ;
/// `N-M` -> that range ; bare `N` -> `[N, hi]` (cron's `N/step` reading).
fn range_bounds(base: &str, lo: u32, hi: u32) -> Option<(u32, u32)> {
    if base == "*" {
        return Some((lo, hi));
    }
    if let Some((a, b)) = base.split_once('-') {
        return Some((a.parse().ok()?, b.parse().ok()?));
    }
    let n: u32 = base.parse().ok()?;
    Some((n, hi))
}

/// Decompose Unix epoch seconds (UTC) into the five cron fields :
/// (minute, hour, day-of-month, month, day-of-week). Day-of-week is 0=Sunday
/// .. 6=Saturday. Reuses the Euclidean affine civil-date algorithm that
/// `clock::now_iso8601_internal` uses, so the calendar maths stays consistent.
pub(super) fn civil_fields(secs: u64) -> (u32, u32, u32, u32, u32) {
    let day_secs = (secs % 86400) as u32;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;

    let z = (secs / 86400) as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };

    // Day-of-week : 1970-01-01 (epoch day 0) was a Thursday (=4).
    let dow = (((secs / 86400) as i64 + 4).rem_euclid(7)) as u32;

    (minute, hour, day, month, dow)
}
