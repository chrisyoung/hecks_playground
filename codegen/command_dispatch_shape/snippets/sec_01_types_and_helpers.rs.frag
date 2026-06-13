#[derive(Debug, Clone)]
pub struct CommandResult {
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub event: Option<Event>,
}

/// Resolution of a dispatch address — i111-J + first-class factories
/// phase 2.
///
/// `Aggregate(agg_idx, cmd_idx)` — command lives directly on the
/// aggregate's commands list.
///
/// `Factory(agg_idx, fac_idx)` — the verb names a Factory on the
/// aggregate : a BIRTH. The node type IS the create/transition bit ;
/// dispatch takes the mint path (fresh id, error if it already
/// exists), never the load path.
///
/// `Entity(agg_idx, ent_idx, cmd_idx)` — command lives inside an
/// entity declared inside the aggregate's `entity "Foo" do … end`
/// block. Dispatch still operates on the parent aggregate's record
/// (entities are reached only through the root in DDD), but uses the
/// entity's command — its mutations, givens, lifecycle.
#[derive(Debug, Clone, Copy)]
enum Resolution {
    Aggregate(usize, usize),
    Factory(usize, usize),
    Entity(usize, usize, usize),
}

impl Resolution {
    fn agg_idx(&self) -> usize {
        match self {
            Resolution::Aggregate(a, _) => *a,
            Resolution::Factory(a, _) => *a,
            Resolution::Entity(a, _, _) => *a,
        }
    }
}

/// Borrowed view over the resolved behavior node — first-class
/// factories phase 2. Factory (birth) and Command (transition) share
/// every field the downstream pipeline reads (attributes, references,
/// givens, mutations, emits) ; the Resolution variant carries WHICH
/// path the verb takes, this view erases the difference for the
/// shared phases.
#[derive(Clone, Copy)]
enum BehaviorRef<'a> {
    Cmd(&'a Command),
    Fac(&'a crate::ir::Factory),
}

impl<'a> BehaviorRef<'a> {
    fn name(&self) -> &'a str {
        match self { BehaviorRef::Cmd(c) => &c.name, BehaviorRef::Fac(f) => &f.name }
    }
    fn attributes(&self) -> &'a [crate::ir::Attribute] {
        match self { BehaviorRef::Cmd(c) => &c.attributes, BehaviorRef::Fac(f) => &f.attributes }
    }
    fn references(&self) -> &'a [crate::ir::Reference] {
        match self { BehaviorRef::Cmd(c) => &c.references, BehaviorRef::Fac(f) => &f.references }
    }
    fn givens(&self) -> &'a [crate::ir::Given] {
        match self { BehaviorRef::Cmd(c) => &c.givens, BehaviorRef::Fac(f) => &f.givens }
    }
    fn mutations(&self) -> &'a [crate::ir::Mutation] {
        match self { BehaviorRef::Cmd(c) => &c.mutations, BehaviorRef::Fac(f) => &f.mutations }
    }
    fn emits(&self) -> Option<&'a str> {
        match self { BehaviorRef::Cmd(c) => c.emits.as_deref(), BehaviorRef::Fac(f) => f.emits.as_deref() }
    }
}

/// Borrow the resolved behavior node (Command or Factory) from the
/// runtime's IR.
fn behavior_for<'a>(rt: &'a Runtime, res: Resolution) -> BehaviorRef<'a> {
    match res {
        Resolution::Aggregate(a, c) => BehaviorRef::Cmd(&rt.domain.aggregates[a].commands[c]),
        Resolution::Factory(a, f) => BehaviorRef::Fac(&rt.domain.aggregates[a].factories[f]),
        Resolution::Entity(a, e, c) => BehaviorRef::Cmd(&rt.domain.aggregates[a].entities[e].commands[c]),
    }
}

/// Borrow the lifecycle that gates the resolved command : the entity's
/// own lifecycle when present, otherwise the parent aggregate's.
fn lifecycle_for<'a>(rt: &'a Runtime, res: Resolution) -> Option<&'a Lifecycle> {
    match res {
        Resolution::Aggregate(a, _) => rt.domain.aggregates[a].lifecycle.as_ref(),
        Resolution::Factory(a, _) => rt.domain.aggregates[a].lifecycle.as_ref(),
        Resolution::Entity(a, e, _) => rt.domain.aggregates[a].entities[e]
            .lifecycle
            .as_ref()
            .or(rt.domain.aggregates[a].lifecycle.as_ref()),
    }
}
