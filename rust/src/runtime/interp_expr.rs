//! interp_expr — resolve_expr, the ONE expression resolver : dotted paths,
//! VO derivation method calls (via lookup_derivation), `.size`/`.length`,
//! arithmetic, string interpolation, rand_below, attrs-over-state binding.
//! Composes the pure leaves in interp_expr_ops.rs ; the given/mutation
//! surfaces that call it stay in interpreter.rs.
//!
//! Cask extracted VERBATIM from runtime/interpreter.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/interp_expr.rs — kernel-floor
//!  interpreter, relocated verbatim from interpreter.rs blanket.]

use super::interp_expr_ops::{numeric_value, rand_below_impl, split_call_args};
use super::interpreter::{evaluate_given, lookup_derivation, EvalCtx};
use super::{AggregateState, Value};
use std::collections::HashMap;

pub(super) fn resolve_expr(expr: &str, state: &AggregateState, attrs: &HashMap<String, Value>, ctx: EvalCtx) -> Value {
    // `field.length` is the Ruby-flavoured alias for `field.size` — both
    // resolve to a collection's element count. Bluebook authors reach for
    // `.length` interchangeably (e.g. `given { tasks.length >= 3 }`) ; the
    // `.size` branch below is the single source of truth, so normalize
    // here at the entry point. Without this the `.length` form falls
    // through to the literal-field lookup, returns Null, numeric-coerces
    // to 0, and silently breaks the predicate.
    if let Some(field) = expr.strip_suffix(".length") {
        return resolve_expr(&format!("{}.size", field.trim()), state, attrs, ctx);
    }
    if let Ok(n) = expr.parse::<i64>() {
        return Value::Int(n);
    }
    // Float literal (`1.0`, `0.5`) — no Float variant exists, so carry it
    // as a Str : numeric_value coerces Str through f64 in every
    // comparison. Without this branch a float literal fell through to the
    // field-lookup, resolved Null, numeric-coerced to 0, and
    // `value <= 1.0` judged against 0 — refusing valid payloads (caught
    // by the payload gate on language.bluebook's Confidence, 2026-07-18).
    if expr.parse::<f64>().is_ok() {
        return Value::Str(expr.to_string());
    }
    if expr.starts_with('"') && expr.ends_with('"') {
        return Value::Str(expr[1..expr.len() - 1].to_string());
    }
    if expr == "true" {
        return Value::Bool(true);
    }
    if expr == "false" {
        return Value::Bool(false);
    }
    // rand_below(N) — uniform random integer in [0, N). The bluebook
    // intent `given { rand_below(N) == 0 }` expresses a 1/N probability
    // gate (every Nth tick / RANDOM % N stochastic dispatch). Lifts the
    // gate from shell-side (mindstream's `RANDOM % 300 == 0`) into the
    // bluebook so capabilities like MusingMint own their cadence as
    // declarative IR. N must be a positive integer literal or an
    // attr/state field that resolves to one ; non-positive N short-
    // circuits to 0 so the predicate fires every call (safer than
    // panicking in a daemon).
    if let Some(arg) = expr.strip_prefix("rand_below(").and_then(|s| s.strip_suffix(')')) {
        let arg_val = resolve_expr(arg.trim(), state, attrs, ctx);
        let n = numeric_value(&arg_val).map(|f| f as i64).unwrap_or(0);
        if n <= 0 {
            return Value::Int(0);
        }
        return Value::Int(rand_below_impl(n));
    }
    if let Some(field) = expr.strip_suffix(".size") {
        let val = attrs.get(field)
            .cloned()
            .unwrap_or_else(|| state.get(field).clone());
        return match val {
            Value::List(v) => Value::Int(v.len() as i64),
            Value::Str(s) => Value::Int(s.len() as i64),
            _ => Value::Int(0),
        };
    }
    // <expr>.modulo(N) — periodic cadence gate. Resolves the receiver
    // as an integer (attr / state field / literal) and returns
    // `receiver % N`. The bluebook intent `given { tick.modulo(60) == 0 }`
    // expresses an every-Nth-tick gate (Memory.Consolidate per-60-tick,
    // any heartbeat-counter periodic cadence). Lifts the gate from
    // shell-side (`if (tick % 60).zero?`) into the bluebook so capabilities
    // own their periodic cadence as declarative IR. N must be a positive
    // integer literal or an attr/state field that resolves to one ;
    // non-positive N short-circuits to 0 so the predicate fires every
    // call (mirrors rand_below's safer-than-panicking guard).
    if let Some(modulo_idx) = expr.rfind(".modulo(") {
        if expr.ends_with(')') {
            let receiver = &expr[..modulo_idx];
            let arg = &expr[modulo_idx + ".modulo(".len()..expr.len() - 1];
            let recv_val = resolve_expr(receiver.trim(), state, attrs, ctx);
            let arg_val = resolve_expr(arg.trim(), state, attrs, ctx);
            let n = numeric_value(&arg_val).map(|f| f as i64).unwrap_or(0);
            if n <= 0 {
                return Value::Int(0);
            }
            let lhs = numeric_value(&recv_val).map(|f| f as i64).unwrap_or(0);
            return Value::Int(lhs.rem_euclid(n));
        }
    }
    // Value-object METHOD dispatch (`balance.covers?(amount)`, `amount.zero?`)
    // — a pure derivation declared on the receiver's value object. Fires only
    // when a type context is present (production givens) ; the invariants and
    // payload-gate callers pass an empty ctx, so this is inert there. Placed
    // BEFORE dotted field access : lookup_derivation returns Some only for a
    // declared derivation, so a plain nested-field read (`amount.cents`) skips
    // this and falls through unchanged. Resolve the receiver to its field-map,
    // bind own-fields + positional params into a sub-scope, and evaluate the
    // body — a Boolean derivation as a predicate, any other as an expression.
    {
        let (call_head, args_str): (&str, Option<&str>) = if expr.ends_with(')') {
            match expr.find('(') {
                Some(p) => (&expr[..p], Some(&expr[p + 1..expr.len() - 1])),
                None => (expr, None),
            }
        } else {
            (expr, None)
        };
        if let Some(dot) = call_head.rfind('.') {
            let receiver = call_head[..dot].trim();
            let method = call_head[dot + 1..].trim();
            if !receiver.contains('.') && !receiver.is_empty() {
                if let Some(deriv) = lookup_derivation(ctx, receiver, method) {
                    let recv = attrs
                        .get(receiver)
                        .cloned()
                        .unwrap_or_else(|| state.get(receiver).clone());
                    if let Value::Map(fields) = recv {
                        let mut sub_attrs = fields.clone();
                        if let Some(a) = args_str {
                            let args = split_call_args(a);
                            for (i, pname) in deriv.params.iter().enumerate() {
                                if let Some(one) = args.get(i) {
                                    let v = resolve_expr(one.trim(), state, attrs, ctx);
                                    sub_attrs.insert(pname.clone(), v);
                                }
                            }
                        }
                        let empty = AggregateState::new("_vo");
                        return if deriv.return_type == "Boolean" {
                            Value::Bool(evaluate_given(&deriv.expression, &empty, &sub_attrs, ctx))
                        } else {
                            resolve_expr(&deriv.expression, &empty, &sub_attrs, ctx)
                        };
                    }
                }
            }
        }
    }
    // Dotted value-object field access : `amount.cents`,
    // `amount.currency.code`. Resolve the head from attrs (input shadows
    // state) or state, then step into each Value::Map segment. This only
    // ADDS nested navigation — if the head is not a Map or a segment is
    // missing, it falls through to the flat lookup below, so a plainly-named
    // field is unaffected. Without this a `given { amount.cents > 0 }`
    // resolved the whole dotted string as a flat key, found nothing, and
    // numeric-coerced to 0 — refusing every valid nested-VO payload.
    if let Some((head, path)) = expr.split_once('.') {
        if let Some(base) = attrs.get(head).or_else(|| state.fields.get(head)) {
            let mut cur = base;
            let mut navigated = true;
            for seg in path.split('.') {
                match cur {
                    Value::Map(m) => match m.get(seg) {
                        Some(v) => cur = v,
                        None => { navigated = false; break; }
                    },
                    _ => { navigated = false; break; }
                }
            }
            if navigated {
                return cur.clone();
            }
        }
    }
    // Command attributes shadow state when they share a name — the
    // `given` clause runs at dispatch time with the inbound input
    // already in scope, mirroring how Ruby's predicate DSL evaluates
    // against a binding that includes both attrs and state.
    if let Some(v) = attrs.get(expr) {
        return v.clone();
    }
    state.get(expr).clone()
}
