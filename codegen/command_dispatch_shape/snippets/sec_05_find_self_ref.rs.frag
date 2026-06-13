/// Resolution-aware self-ref finder (i111-J). For an entity command,
/// the self-ref still points at the parent aggregate's record because
/// the entity is reached through the root. The reference's actual
/// name (which may be aliased via `role: :incident_id`) is what the
/// runner / DSL passes the kwarg under ; the runtime must look up
/// under the same key.
fn find_self_ref_res(rt: &Runtime, res: Resolution) -> Option<String> {
    // Factories BIRTH — a self-reference on a factory is meaningless by
    // construction (the record does not exist yet), so the mint path
    // never consults one. Converted creators may still carry a
    // transitional `reference_to(Self)` ; it is ignored here.
    if matches!(res, Resolution::Factory(..)) { return None; }
    let agg_idx = res.agg_idx();
    let agg = &rt.domain.aggregates[agg_idx];
    let cmd = behavior_for(rt, res);
    let agg_snake = to_snake_case(&agg.name);
    for r in cmd.references() {
        let ref_snake = to_snake_case(&r.target);
        if ref_snake == agg_snake || agg_snake.ends_with(&ref_snake) {
            return Some(r.name.clone());
        }
    }
    None
}

