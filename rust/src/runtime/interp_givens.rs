//! interp_givens — the given gate : check_givens (every command `given`
//! evaluated against state + attrs, GivenFailed on the first false),
//! evaluate_predicate (the public wrapper the invariants module reuses —
//! an invariant IS a given checked on every command's resulting state),
//! and evaluate_given (the comparison/boolean evaluator over
//! interp_expr's resolver). `interpreter::check_givens` /
//! `interpreter::evaluate_predicate` stay valid through parent re-exports.
//!
//! SPLIT ORDER, lowest precedence outward — this is semantics, not style :
//!
//!   ||  ->  &&  ->  .any? / .empty?  ->  .include?  ->  >= <= < > == !=
//!       ->  !  ->  bare
//!
//! `!` sits second-from-last because it binds TIGHTER than every binary
//! operator: Ruby reads `!a && b` as `(!a) && b`. The `.any?` / `.empty?`
//! branches match on a SUFFIX, so they carry an explicit guard against a
//! leading `!` — without it they strip the suffix off `!value` instead of
//! `value` and swallow the negation entirely.
//!
//! Cask extracted VERBATIM from runtime/interpreter.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/interp_givens.rs — kernel-floor
//!  interpreter, relocated verbatim from interpreter.rs blanket.]

use super::interpreter::EvalCtx;
use super::{AggregateState, RuntimeError, Value};
use crate::ir::Command;
use std::collections::HashMap;

pub fn check_givens(
    cmd: &Command,
    state: &AggregateState,
    attrs: &HashMap<String, Value>,
    ctx: EvalCtx,
) -> Result<(), RuntimeError> {
        // A mutation the parser could not resolve (an unknown `then_set` op like
        // `bogus_op:`) is recorded with `invalid_op = Some(_)` instead of
        // panicking the parse. Refuse to dispatch it : applying the placeholder
        // `Set` would silently write the wrong value — the exact silent failure
        // the graceful-record path exists to avoid. `validate` catches this
        // earlier ; this is the runtime's own guard for callers that skip it.
        for m in &cmd.mutations {
            if let Some(op) = &m.invalid_op {
                return Err(RuntimeError::GivenFailed {
                    message: format!(
                        "unknown mutation op `{}:` in then_set for `:{}` — valid ops are \
                         to, append, append_unique, remove, increment, decrement, \
                         multiply, clamp, decay, from",
                        op, m.field
                    ),
                    expression: "invalid_mutation_op".to_string(),
                });
            }
        }
        // `required: true` attributes are enforced before givens — a kwarg that is
    // absent, Null, or empty refuses the command the same shape a failed given
    // does. Structural superset of the old `given { x != "" }` idiom, which
    // could not see a truly-absent (Null) kwarg.
    for attr in &cmd.attributes {
        if attr.required {
            let present = attrs.get(&attr.name)
                .is_some_and(|v| !matches!(v, Value::Null) && v.to_string() != "");
            if !present {
                return Err(RuntimeError::GivenFailed {
                    message: format!("{} is required", attr.name),
                    expression: "required".to_string(),
                });
            }
        }
    }
    for given in &cmd.givens {
        if !evaluate_given(&given.expression, state, attrs, ctx) {
            return Err(RuntimeError::GivenFailed {
                message: given
                    .message
                    .clone()
                    .unwrap_or_else(|| given.expression.clone()),
                expression: given.expression.clone(),
            });
        }
    }
    Ok(())
}


pub use super::interp_predicate::evaluate_predicate;
use super::interp_predicate::evaluate_given;
