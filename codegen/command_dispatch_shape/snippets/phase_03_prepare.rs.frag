    // First-class factories phase 2 : the node TYPE carries the
    // create/transition bit. A verb resolving to a Factory takes the
    // mint path ; a Command takes the load path. The #729-era
    // `is_create` name heuristic (Create/Add/Place/Register/Open
    // prefixes) is DELETED — creation is declared, never guessed.
    let is_factory_verb = matches!(res, Resolution::Factory(..));

    let self_ref = find_self_ref_res(rt, res);
    let aggregate_name = rt.domain.aggregates[agg_idx].name.clone();
    let aggregate_context = rt.domain.aggregates[agg_idx].context.clone();
    let repo_hash_key = super::repo_key(aggregate_context.as_deref(), &aggregate_name);

    let attrs = attrs;
