    let (mut state, is_new) = if is_factory_verb {
        // BIRTH — first-class factories phase 2. A factory mints a fresh
        // record ; it NEVER targets an existing one. The id comes from
        // the dispatch attrs (identified_by) or the repo's counter mint ;
        // an existing record under that id REFUSES the dispatch — no
        // silent upsert at birth. Factories ignore self_ref and cascade
        // hints by construction : a thing being born has no prior id to
        // load.
        let id = repo.id_for_command(&attrs);
        if repo.find(&id).is_some() {
            return Err(RuntimeError::AggregateAlreadyExists {
                aggregate: aggregate_name.clone(),
                id: id.clone(),
                factory: command_name.to_string(),
            });
        }
        (AggregateState::new(&id), true)
    } else if let Some(ref_name) = &self_ref {
        // Universal self-ref dispatch — i519 sidequest. Callers may pass
        // either the snake-cased aggregate name (the historical kwarg
        // determined by `find_self_ref_res`) or the universal `id` key.
        // The snake-cased lookup takes precedence so existing dispatches
        // are byte-identical ; the `id` fallback removes the convention-
        // discovery cliff for new callers (`reference_to(ExemptRegistry)`
        // is no longer "guess `exempt_registry=`").
        //
        // Safety : the `attribute :id` / `reference_to` collision was
        // checked across every bluebook in the corpus at i519-time ; no
        // command declares its own `id` attribute alongside a self-ref.
        // The corpus contract is enforceable by a validator if it ever
        // drifts.
        let self_ref_id = attrs.get(ref_name).map(|v| v.to_string())
            .or_else(|| cascade_fk_id.clone())
            .or_else(|| attrs.get("id").map(|v| v.to_string()));
        if let Some(id) = self_ref_id {
            match repo.find(&id).cloned() {
                Some(s) => (s, false),
                None => return Err(RuntimeError::AggregateNotFound(
                    format!("no aggregate of type '{}' found with id '{}' — the dispatch expected an existing record but none matched (passed via attr '{}')",
                        aggregate_name, id, ref_name)
                )),
            }
        } else if let Some(ref id) = cascade_id {
            // Cascade hint resolved a same-type id — reuse it even
            // when the command has a self-ref kwarg the cascade didn't
            // populate (the upstream event carries the id, not the
            // kwarg name).
            match repo.find(id).cloned() {
                Some(s) => (s, false),
                None => return Err(RuntimeError::AggregateNotFound(
                    format!("no aggregate of type '{}' found with id '{}' — the dispatch expected an existing record but none matched (cascade hint)",
                        aggregate_name, id)
                )),
            }
        } else {
            // No id resolvable on a self-ref'd COMMAND — a transition
            // with nothing to transition. The #729-era heuristic minted
            // here for Create*/Add*/… names ; phase 2 deleted it. A verb
            // that births must be a `factory` block — declared, not
            // name-guessed.
            return Err(RuntimeError::MissingAttribute(
                self_ref_missing_message(rt, res, command_name, ref_name)
            ));
        }
    } else if let Some(ref id) = cascade_id {
        // Same-type cascade with an existing record — reuse it,
        // skipping id_for_command's counter-mint.
        match repo.find(id).cloned() {
            Some(s) => (s, false),
            None => (AggregateState::new(id), true),
        }
    } else {
        let id = repo.id_for_command(&attrs);
        match repo.find(&id).cloned() {
            Some(s) => (s, false),
            None => (AggregateState::new(&id), true),
        }
    };
