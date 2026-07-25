//! log_time — the hand-rolled, chrono-free, wasm-safe time layer of the
//! storehouse log : now_iso8601 (RFC-3339 seconds-precision UTC),
//! interpolate_now_tokens ({now}/{now-N[smhd]} templating),
//! parse_now_token, format_iso8601, civil_from_days (Howard Hinnant's
//! algorithm). Old `storehouse_log::` paths hold via re-exports.
//!
//! Cask extracted VERBATIM from runtime/storehouse_log.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/log_time.rs — kernel-floor log time
//!  layer, relocated verbatim from storehouse_log.rs blanket.]

/// RFC-3339 / ISO-8601 timestamp with seconds precision, UTC. Hand-
/// rolled to avoid a chrono dependency — Cargo.toml stays minimal.
/// Produces `YYYY-MM-DDTHH:MM:SSZ`. Public for callers that want to
/// emit ad-hoc lines on the same stream with matching timestamps.
pub fn now_iso8601() -> String {
    // wasm-safe clock (i630) — raw SystemTime::now() panics
    // "time not implemented" on wasm32 (CF Worker). dispatch_entry
    // runs on every dispatch, so the worker hit this on every POST.
    let secs = crate::clock::now_duration().as_secs();
    format_iso8601(secs)
}

/// The clock as an attr value (the `{now}` primitive). Resolves the
/// time tokens that dispatch templates write :
///   `{now}`      -> the current instant, ISO-8601 UTC
///   `{now+<N>}`  -> now + N seconds (e.g. a TTL : `{now+3600}`)
///   `{now-<N>}`  -> now - N seconds
/// N is a non-negative integer count of seconds. The base instant comes
/// from clock::now_duration(), so `HECKS_NOW=<epoch>` freezes every token
/// in a run. Output is ISO-8601 UTC, which sorts chronologically — so a
/// `where stale_after: { lt: :now }` comparison against a stored
/// `{now+N}` value is correct lexically. Unknown / malformed tokens are
/// left untouched (the caller's other interpolation passes still run).
///
/// This is the SINGLE reachability point for the clock as a value : the
/// hecksagon dispatch path (interpolate_event) and the cadence loop
/// driver both call it, so `expires_at: "{now+3600}"` and
/// `stale_after={now+N}` resolve to real time in BOTH write paths.
pub fn interpolate_now_tokens(template: &str) -> String {
    if !template.contains("{now") {
        return template.to_string();
    }
    let base = crate::clock::now_duration().as_secs() as i64;
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(close) = template[i..].find('}') {
                let token = &template[i + 1..i + close];
                if let Some(secs) = parse_now_token(token, base) {
                    out.push_str(&format_iso8601(secs.max(0) as u64));
                    i += close + 1;
                    continue;
                }
            }
        }
        // Not a now-token : copy the byte through. UTF-8 safe because we
        // only fast-path on the ASCII '{' boundary ; everything else is
        // copied verbatim by char.
        let ch = template[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Parse the inside of a `{...}` token as a now-expression. Returns the
/// resolved epoch seconds (relative to `base`) when the token is `now`,
/// `now+N`, or `now-N` ; None for anything else (so non-now tokens fall
/// through to the caller's field interpolation untouched).
pub(super) fn parse_now_token(token: &str, base: i64) -> Option<i64> {
    let t = token.trim();
    if t == "now" {
        return Some(base);
    }
    let rest = t.strip_prefix("now")?;
    let (sign, digits) = match rest.as_bytes().first()? {
        b'+' => (1i64, &rest[1..]),
        b'-' => (-1i64, &rest[1..]),
        _ => return None,
    };
    let n: i64 = digits.trim().parse().ok()?;
    Some(base + sign * n)
}

/// Format a Unix epoch second count as ISO-8601 UTC. Splits the
/// seconds into Y-M-D H:M:S using the civil-from-days algorithm
/// (Howard Hinnant, MIT-licensed public algorithm) so we don't pull
/// chrono just for one timestamp surface. pub(crate) so the `{now}`
/// token resolver (interpolate_now_tokens) reuses the one formatter
/// instead of cloning the civil-from-days math.
pub(crate) fn format_iso8601(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let s = (secs % 86_400) as u32;
    let (year, month, day) = civil_from_days(days);
    let hour = s / 3600;
    let min = (s % 3600) / 60;
    let sec = s % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, min, sec
    )
}

/// Convert Unix epoch days (offset from 1970-01-01) into (year, month,
/// day). Adapted from Howard Hinnant's `civil_from_days`. Stable for
/// every date between 0000-03-01 and ~5879611-07-11, which is well
/// past anything this logger will encounter.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468; // shift epoch to 0000-03-01
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
