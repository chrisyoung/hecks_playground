module Hecksagain
  module Bluebook
    class Assembly
      # ONE TABLE, WHERE THERE WERE FIVE HAND-WRITTEN MIRRORS OF IT.
      #
      # `bluebook.bluebook` already declares what every construct is made of, and
      # `Plan` already reads it — parent, fields, lists, setters. What the language
      # cannot say is the three things Ruby needs to rebuild one, so those are here
      # and ONLY those:
      #
      #   holder    which class holds it
      #   make      :declare for a construct that became a class, :new for an
      #             instance — the boundary `IR::Query` sits on, since its body is
      #             inherited instance methods
      #   fields    each keyword the constructor takes, as
      #             keyword => [key in the declaration, how to read it]
      #
      # A READER is a symbol naming a method on Marks, or :plain for a value that
      # needs no decoding, or [:each, reader] for a list. Nothing here is a rule
      # about whether a declaration is admissible — the language settled that on the
      # way in. This is only how a spelling becomes an object again.
      #
      # WHY A TABLE AND NOT A METHOD PER CATEGORY. The judge used to carry one
      # hand-written branch per category, and the price was fourteen verbs the
      # language declared and the walk never offered — every rule hanging off them
      # decoration, and nothing red, because a branch that does not exist cannot
      # fail. An assembler with a method per category is the same shape. So the
      # table is checked AGAINST the language by spec/assembly_spec: a field the
      # language declares that no contract consumes is a failure, not a silence.
      #
      # The `derived` list is how a field says it needs no assembling — a parent
      # pointer the containment tree already knows, or something computed from what
      # is here (`query_name` is `Naming.snake(name)`, `creates?` is whether a verb
      # names a root). Naming one is a claim, and the coverage gate holds it.
      def self.contract(category) = CONTRACTS.fetch(category.to_s)

      CONTRACTS = {
        "Bluebook" => Contract.new(
          holder: IR::Bluebook, make: :new,
          fields: {
            name:           [:name,           :plain],
            version:        [:version,        :plain],
            vision:         [:vision,         :plain],
            classification: [:classification, :plain],
            formerly_known_as: [:formerly_known_as, :plain],
            # Vendored addition, not (yet) upstream hecksagain (migration
            # plan task 4): same three-part shape `redirects_native`
            # already used to close this exact gap on Command -- the
            # self-hosted Bluebook aggregate now declares `category`
            # (bluebook.bluebook), so the Judge's rebuild needs to read it
            # back too, or `IR::Bluebook.new`'s own default (`category:
            # nil`) would silently win over whatever the original DSL
            # declaration said.
            category: [:category, :plain]
          },
          rows: { normalisations: :normalisation_table },
          derived: { normalisations: :elsewhere }
        ),

        "Aggregate" => Contract.new(
          holder: IR::Aggregate, make: :new,
          fields: {
            name:          [:name,          :plain],
            description:   [:description,   :plain],
            identified_by: [:identified_by, :plain],
            attributes:    [:attributes,    [:each, :attribute]],
            provenance:    [:provenance,    :plain]
          },
          rows: { transitions: :transition_rows, value_objects: :value_object_names, identified_by: :identity_rows },
          reads: { identified_by: [:each, :identity_path], attributes: [:each_with_id, :attribute] },
          derived: {
            position: :walk,
            state_field:   [:folded, :lifecycle, :field],
            state_start:   [:folded, :lifecycle, :default],
            transitions:   [:folded, :lifecycle, :transitions],
            value_objects: :children
          }
        ),

        "Command" => Contract.new(
          holder: IR::Command, make: :declare,
          fields: {
            name:       [:name,       :plain],
            role:       [:role,       :plain],
            goal:       [:goal,       :plain],
            references: [:references, :plain],
            attributes: [:attributes, [:each, :shape_field]],
            givens:     [:givens,     [:each, :given]],
            ensures:    [:ensures,    [:each, :given]],
            mutations:  [:mutations,  [:each, :mutation]],
            emits:      [:emits,      :plain],
            provenance: [:provenance, :plain],
            # Vendored addition, not (yet) upstream hecksagain (migration
            # plan task 5): this Contract REBUILDS Command objects via
            # `IR::Command.declare(**fields)` (the self-hosted meta-
            # validator's own judging pass reads bluebook structure back
            # through contracts like this one) -- `redirects_native` was
            # never added to the field list, so `declare`'s own default
            # (`redirects_native: []`) silently WON on every rebuilt
            # command, discarding whatever the original DSL declaration
            # actually said. Found live, not by inspection: a real
            # `storehouse mailboxes`-adapter-facing check
            # (GovernedDoor.LookupDoor) queried FileTool.Read's own
            # `redirects_native "Read"` and got `[]` back -- the hard
            # PreToolUse block's entire mechanism depends on this field
            # surviving. Same shape as `emits`, a plain string list.
            redirects_native: [:redirects_native, :plain]
          },
          rows: { mutations: :mutation_rows },
          reads: { attributes: [:each, :shape_field], givens: [:each, :rule], ensures: [:each, :rule],
                  mutations: [:call, :mutations], emits: :names, provenance: :provenance,
                  redirects_native: :names },
          derived: { position: :walk }
        ),

        "ValueObject" => Contract.new(
          holder: IR::ValueObject, make: :declare,
          fields: {
            name:       [:name,       :plain],
            attributes: [:attributes, [:each, :shape_field]],
            invariants: [:invariants, [:each, :invariant]],
            members:    [:members,    [:each, :member]],
            closed_set: [:closed_set, :flag]
          },
          # The language counts the admitted rows ; the IR keeps a flag and the rows.
          reads: { attributes: [:each, :shape_field], invariants: [:each, :rule],
                  closed_set: [:call, :closed_set_of], members: [:call, :members_row] },
          derived: { position: :walk, rows: [:folded, %i[closed_set members], nil] }
        ),

        "Query" => Contract.new(
          holder: IR::Query, make: :new,
          fields: {
            name:        [:name,        :plain],
            description: [:description, :plain],
            attributes:  [:attributes,  [:each, :shape_field]],
            wheres:          [:wheres,          [:each, :where_clause]],
            order_by:        [:order_by,        :order_by],
            limit:           [:limit,           :limit],
            # Vendored addition, not (yet) upstream hecksagain (migration
            # plan task 8): `count`/`median_field`/`group_by_field` never
            # reached this table, so a query carrying any of the three
            # survived the DSL parse and vanished the moment the same
            # bluebook went through `MetaValidator.call` (every ordinary
            # `Hecks.bluebook` load). Same shape as `redirects_native`'s
            # own gap on Command — the self-hosted grammar's own "Query"
            # aggregate needed the matching fields too (see
            # behavior.bluebook). `count` rides as a real Ruby boolean
            # (`:flag`, decoded from the language's stored text by
            # `count_flag` below — see its own comment) ; `median_field`/
            # `group_by_field` are Symbols, the same `:identity` reading
            # `correlates_by` already uses.
            count:           [:count,           :flag],
            median_field:    [:median_field,    :identity],
            group_by_field:  [:group_by_field,  :identity],
            # Held by the language as an OPEN MAP, so every one of these reads the
            # same way and a ninth option needs no new field on either side.
            offset:          [:offset,          [:option, :offset]],
            cursor:          [:cursor,          [:option, :cursor]],
            null_semantics:  [:null_semantics,  [:option, :null_semantics]],
            authorization:   [:authorization,   [:option, :authorization]],
            consistency:     [:consistency,     [:option, :consistency]],
            freshness:       [:freshness,       [:option, :freshness]],
            inspection:      [:inspection,      [:option, :inspection]],
            index_hints:     [:index_hints,     :index_hints]
          },
          rows: { wheres: :where_rows, options: :option_rows },
          reads: { attributes: [:each, :shape_field], wheres: [:each, :where_clause],
                  order_by: [:call, :order_by], limit: [:call, :limit],
                  count: [:call, :count_flag] },
          derived: {
            position: :walk,
            order_field: [:folded, :order_by, :field],
            order_way:   [:folded, :order_by, :direction],
            options:     [:folded, %i[offset cursor null_semantics authorization
                                       consistency freshness inspection index_hints], nil]
          }
        ),

        "Entity" => Contract.new(
          holder: IR::Entity, make: :declare,
          fields: {
            name:          [:name,          :plain],
            description:   [:description,   :plain],
            # A LIST OF PATHS, exactly as an aggregate's is. The two used to differ
            # — a Symbol here and a String there, which byte equality with `to_h`
            # COULD NOT SEE because both render as a string, so the assembled graph
            # got a String and `element_of` looked up `args["sequence"]` in a
            # symbol-keyed payload and found nothing: "Reverse acts on one
            # LedgerEntry — pass sequence:", while passing sequence. There is one
            # spelling now, and no room left for that difference.
            identified_by: [:identified_by, :plain],
            attributes:    [:attributes,    [:each, :shape_field]]
          },
          rows: { transitions: :transition_rows, identified_by: :identity_rows },
          reads: { identified_by: [:each, :identity_path], attributes: [:each, :shape_field] },
          derived: {
            position: :walk,
            owner:       :parent,
            state_field: [:folded, :lifecycle, :field],
            state_start: [:folded, :lifecycle, :default],
            transitions: [:folded, :lifecycle, :transitions]
          }
        ),

        "Policy" => Contract.new(
          holder: IR::Policy, make: :new,
          fields: {
            name:            [:name,            :plain],
            aggregate:       [:aggregate,       :plain],
            on_event:        [:on_event,        :plain],
            trigger_command: [:trigger_command, :plain],
            target_domain:   [:target_domain,   :plain],
            # Vendored addition, not (yet) upstream hecksagain (migration
            # plan task 8): `wheres`/`with_literals`/`for_each` never
            # reached this table, so a policy carrying any of the three
            # survived the DSL parse and vanished the moment the same
            # bluebook went through `MetaValidator.call` (every ordinary
            # `Hecks.bluebook` load) — see reaction.bluebook's own
            # comment on the matching grammar addition. `wheres`/
            # `with_literals` are open maps, the SAME `:bindings` reading
            # `Handler#remembers` already uses (Symbol keys, matching
            # `where`'s own `transform_keys(&:to_sym)`) — `with_literals`
            # needs its own `:literal_map` twin only because ITS keys are
            # Strings (`with`/`map`'s own `transform_keys(&:to_s)`).
            # `for_each` is a compound field the language keeps as two
            # (`for_each_from`/`for_each_where`, both `derived:` below,
            # since assembling `for_each` as one thing is this row's job)
            # — see IR::Policy#for_each_from/#for_each_where's own comment.
            wheres:          [:wheres,          :bindings],
            with_literals:   [:with_literals,   :literal_map],
            for_each:        [:for_each,        :for_each_spec]
          },
          rows: {
            wheres:         :policy_wheres_rows,
            with_literals:  :policy_with_literals_rows,
            for_each_where: :policy_for_each_where_rows
          },
          reads: {
            wheres:        [:from, :wheres],
            with_literals: [:from, :with_literals],
            for_each:      [:call, :policy_for_each]
          },
          derived: {
            position: :walk,
            # `[:computed, ...]`, not `[:folded, ...]` — a folded claim
            # needs a REAL reconstructed declaration somewhere in the
            # assembly corpus to carry both keys together (Contract#folded's
            # own comment), and `for_each` is new enough that nothing in
            # that corpus exercises it yet. `computes?` asks a narrower,
            # STATIC question instead — does `IR::Policy` answer to this
            # name while its OWN constructor does not accept it as a
            # keyword — which the two readers just added to `ir/policy.rb`
            # satisfy directly, with no corpus content required.
            for_each_from:  [:computed, :for_each_from],
            for_each_where: [:computed, :for_each_where]
          }
        ),

        "ProcessManager" => Contract.new(
          holder: IR::ProcessManager, make: :new,
          fields: {
            name:          [:name,          :plain],
            # A SYMBOL. `SagaInterpreter` does `event.payload[pm.correlates_by]` — a
            # hash lookup on a symbol-keyed payload — and `value == pm.correlates_by`
            # when resolving a leg s bindings. A String there finds nothing and
            # resolves to nothing, so the wire never advanced and a drawer that
            # should have held 2500 held 0. to_h could not show it: it stringifies.
            correlates_by: [:correlates_by, :identity],
            starts_on:     [:starts_on,     :plain],
            ends_on:       [:ends_on,       :plain],
            states:        [:states,        :plain]
          },
          reads: { states: :names },
          derived: { position: :walk }
        ),

        "Handler" => Contract.new(
          holder: IR::ProcessManagerHandler, make: :new,
          fields: {
            event_type: [:event_type, :plain],
            from_state: [:from_state, :plain],
            to_state:   [:to_state,   :plain],
            # Vendored addition, not (yet) upstream hecksagain (migration
            # plan task 4): same open-map shape Dispatch's own `with_spec`
            # already is — `remember key: from_event(...)` has no value
            # object that can hold it, so it rides as rows.
            remembers:  [:remembers,  :bindings],
            # `guard_count` is now a real seventh member of IR::
            # ProcessManagerHandler (its own comment explains why) — only
            # the COUNT survives the round trip, never the predicates
            # themselves (raw Procs). `:plain`, the same as every other
            # scalar here.
            guard_count: [:guard_count, :plain]
          },
          rows: { remembers: :remembers_rows },
          reads: { remembers: [:from, :remembers] },
          derived: { position: :walk }
        ),

        "Dispatch" => Contract.new(
          holder: IR::DispatchSpec, make: :new,
          fields: {
            command_name: [:command_name, :plain],
            with_spec:    [:with_spec,    :bindings]
          },
          rows: { with_spec: :with_spec_rows },
          reads: { with_spec: [:from, :with_spec] },
          derived: { position: :walk, handler: :parent }
        ),

        "ReadModel" => Contract.new(
          holder: IR::ReadModel, make: :new,
          fields: {
            name:             [:name,             :plain],
            description:      [:description,      :plain],
            reference_name:   [:reference_name,   :identity],
            reference_target: [:reference_target, :plain],
            aggregate_heads:  [:aggregate_heads,  [:each, :head]],
            group_by:         [:group_by,         [:each, :group_by_field]],
            # A read model inherits every option an ask has, so it reads them the
            # same way — see Query.
            wheres:           [:wheres,           [:each, :where_clause]],
            order_by:         [:order_by,         :order_by],
            limit:            [:limit,            :limit],
            offset:           [:offset,           [:option, :offset]],
            cursor:           [:cursor,           [:option, :cursor]],
            null_semantics:   [:null_semantics,   [:option, :null_semantics]],
            authorization:    [:authorization,    [:option, :authorization]],
            consistency:      [:consistency,      [:option, :consistency]],
            freshness:        [:freshness,        [:option, :freshness]],
            inspection:       [:inspection,       [:option, :inspection]],
            index_hints:      [:index_hints,      :index_hints]
          },
          rows: { options: :read_model_option_rows },
          reads: { reference_name: :symbol, aggregate_heads: [:each, :head], group_by: [:each, :group_by_field] },
          derived: {
            position: :walk,
            query_name: [:computed, :query_name],
            options:    [:folded, %i[offset cursor null_semantics authorization
                                      consistency freshness inspection index_hints], nil]
          }
        ),

        # A member's pairs are an OPEN MAP, which is why Member is its own root in
        # the language. The IR keeps them as a plain hash on the value object, so
        # they are assembled with their shape rather than as a construct.
        "Member" => Contract.new(
          holder: nil, make: nil,
          fields: {},
          rows: { pairs: :pair_rows },
          derived: { position: :walk, shape: :parent, pairs: [:folded, %i[members], nil] }
        )
      }.freeze
    end
  end
end
