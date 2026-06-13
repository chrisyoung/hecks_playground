fn find_behavior_references<'a>(rt: &'a Runtime, name: &str) -> Option<&'a [crate::ir::Reference]> {
    // i156 — accept the same three address forms as `command_dispatch::resolve` :
    // Context.Aggregate.Command, Aggregate.Command, and bare Command. Bare-name
    // setups historically picked first-match-wins ; qualified setups (i142+)
    // must filter by aggregate (and optionally context) so reference injection
    // looks up self-ref kwargs against the right aggregate.
    //
    // First-class factories phase 2 — the verb may name a Factory (a
    // birth) instead of a Command ; factories carry references too
    // (cross-aggregate context for the thing being born), so each arm
    // searches commands then factories and returns the references slice,
    // which is all build_attrs consumes.
    fn on_agg<'a>(agg: &'a crate::ir::Aggregate, verb: &str) -> Option<&'a [crate::ir::Reference]> {
        if let Some(c) = agg.commands.iter().find(|c| c.name == verb) {
            return Some(&c.references);
        }
        if let Some(f) = agg.factories.iter().find(|f| f.name == verb) {
            return Some(&f.references);
        }
        None
    }
    let parts: Vec<&str> = name.split('.').collect();
    match parts.as_slice() {
        [context, agg_name, cmd_name] => {
            for agg in &rt.domain.aggregates {
                if agg.name != *agg_name { continue; }
                if agg.context.as_deref() != Some(*context) { continue; }
                if let Some(refs) = on_agg(agg, cmd_name) { return Some(refs); }
            }
            None
        }
        [agg_name, cmd_name] => {
            for agg in &rt.domain.aggregates {
                if agg.name != *agg_name { continue; }
                if let Some(refs) = on_agg(agg, cmd_name) { return Some(refs); }
            }
            None
        }
        _ => {
            for agg in &rt.domain.aggregates {
                if let Some(refs) = on_agg(agg, name) { return Some(refs); }
            }
            None
        }
    }
}
