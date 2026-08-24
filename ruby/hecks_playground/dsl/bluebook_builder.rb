# Bootstrap: EventBuilder and StrategicBuilders are used at class-body
# time. Cannot use chapter-driven loading.
require "hecks_playground/dsl/event_builder"
require "hecks_playground/dsl/bluebook_builder/strategic_builders"
require "hecks_playground/dsl/bluebook_builder/elsewhere_words"

module HecksPlayground
  module DSL

    # HecksPlayground::DSL::BluebookBuilder
    #
    # Top-level DSL builder for domain definitions. Collects aggregate definitions
    # and domain-level policies, then builds a BluebookModel::Structure::Domain.
    # Enforces unique aggregate names. Domain-level policies are cross-aggregate
    # reactive policies defined outside any aggregate block.
    #
    #   HecksPlayground.bluebook "Banking" do
    #     aggregate "Loan" do ... end
    #     aggregate "Account" do ... end
    #
    #     policy "DisburseFunds" do
    #       on "IssuedLoan"
    #       trigger "Deposit"
    #       map principal: :amount
    #     end
    #   end
    #
    # Builds a BluebookModel::Structure::Domain from top-level DSL declarations.
    #
    # BluebookBuilder is the entry point for defining an entire domain model. It
    # collects aggregate definitions, cross-aggregate reactive policies, domain
    # services, read model views, workflows, event subscribers, and tenancy
    # configuration. The +#build+ method assembles these into an immutable
    # Domain IR (intermediate representation) object.
    #
    # Aggregates names must be unique within a domain; attempting to define
    # a duplicate raises +ArgumentError+. Errors within aggregate blocks are
    # wrapped in +HecksPlayground::ValidationError+ with context about which aggregate
    # failed.
    #
    # Includes AttributeCollector for domain-level attribute declarations
    # (rarely used, but available for domain metadata).
    class BluebookBuilder
      Structure = BluebookModel::Structure

      include AttributeCollector
      include Describable
      include ElsewhereWords

      # Initialize a new domain builder with the given domain name.
      #
      # Sets up empty collections for all domain-level elements.
      #
      # @param name [String] the domain name (e.g. "Banking", "PizzaShop")
      # @param version [String, nil] optional version string (semver or CalVer)
      def initialize(name, version: nil)
        @name = name
        @version = version
        @aggregates = []
        @paragraphs = []
        @policies = []
        @services = []
        @views = []
        @workflows = []
        @attributes = []
        @actors = []
        @sagas = []
        @process_managers = []
        @cadences = []
        @block_grammars = []
        @glossary_rules = []
        @vision = nil
        @subdomain = nil
        @sme = nil
        @glossary_terms = []
      end

      # Set the strategic vision statement for this domain.
      # Describes what problem the domain solves in business terms.
      #
      #   vision "Manage pizza creation and ordering for a pizzeria"
      #
      # @param text [String] the vision statement
      # @return [void]
      def vision(text)
        @vision = text
      end

      def category(text)
        @category = text
      end

      # Declare a subject matter expert for this domain.
      # The SME joins the product executor room as a domain-specific advisor.
      #
      #   sme "Dr. Pizza", "20 years in pizza operations, knows every edge case"
      #
      # @param name [String] the SME's name
      # @param expertise [String] what they know and how they help
      # @return [void]
      def sme(name, expertise = nil)
        @sme = { name: name, expertise: expertise }
      end

      # Classify this domain's subdomain type (Evans strategic design).
      #
      #   subdomain :core
      #
      # @param type [Symbol] one of :core, :supporting, :generic
      # @return [void]
      def subdomain(type)
        @subdomain = type.to_sym
      end

      # Shorthand for subdomain(:core)
      def core;       subdomain(:core);       end
      # Shorthand for subdomain(:supporting)
      def supporting; subdomain(:supporting); end
      # Shorthand for subdomain(:generic)
      def generic;    subdomain(:generic);    end

      # Define a glossary term with its business definition.
      # Builds the ubiquitous language dictionary for this domain.
      #
      #   define "Topping", "A measured ingredient placed on a pizza"
      #   define "Order", "A customer's request for one or more pizzas"
      #
      # @param term [String] the domain term name
      # @param definition [String] what this term means in the domain
      # @return [void]
      def define(term, definition)
        @glossary_terms << { name: term.to_s, definition: definition.to_s }
      end

      def actor(name, description: nil)
        @actors << Structure::Actor.new(name: name.to_s, description: description)
      end


      # Saga for long-running cross-aggregate coordination.
      #   saga "OrderFulfillment" do
      #     step "ReserveInventory" do
      #       on_success "InventoryReserved"
      #       compensate "ReleaseInventory"
      #     end
      #     timeout "48h"
      #     on_timeout "CancelOrder"
      #   end
      def saga(name, &block)
        builder = SagaBuilder.new(name)
        builder.instance_eval(&block) if block
        @sagas << builder.build
      end

      # Process manager for event-driven cross-aggregate state machines.
      # Mirrors the runtime class +HecksPlayground::EventSourcing::ProcessManager+ exactly,
      # so the DSL reads as a thin declarative wrapper. Phase 1 of the dream-study
      # plan ships parser + IR ; runtime instantiation is Phase 2.
      #
      #   process_manager "SleepCycle" do
      #     correlates_by :body_id
      #     starts_on    "SleepStarted"
      #     ends_on      "WakeFinished"
      #     state "light"
      #     state "rem"
      #     on "PhaseElapsed", transition: { light: :light } do |event, pm|
      #       { commands: ["AdvancePhase"] }
      #     end
      #   end
      def process_manager(name, &block)
        builder = ProcessManagerBuilder.new(name)
        builder.instance_eval(&block) if block
        @process_managers << builder.build
      end

      # Cadence — declarative scheduled dispatch. Replaces imperative
      # while-true-sleep-1-dispatch loops with `cadence "Name" do every
      # "1s" ; dispatch "Aggregate.Command" ; end`.
      #
      #   cadence "BodyTick" do
      #     every "1s"
      #     dispatch "Consciousness.ElapsePhase"
      #     dispatch "Tick.MindstreamTick"
      #   end
      def cadence(name, &block)
        builder = CadenceBuilder.new(name)
        builder.instance_eval(&block) if block
        @cadences << builder.build
      end

      # Block grammar — declarative keyword routing for the parser
      # itself (i218). Retires the hardcoded if-chain in
      # rust/src/parser.rs by lifting it into bluebook.
      #
      #   block_grammar "Bluebook" do
      #     block "aggregate",        parser: :parse_aggregate
      #     block "policy",           parser: :parse_policy
      #     block "process_manager",  parser: :parse_process_manager
      #     block "cadence",          parser: :parse_cadence
      #   end
      def block_grammar(name, &block)
        builder = BlockGrammarBuilder.new(name)
        builder.instance_eval(&block) if block
        @block_grammars << builder.build
      end

      # Ubiquitous language enforcement.
      #   glossary do
      #     prefer "stakeholder", not: ["user", "person"]
      #   end
      #   glossary(strict: true) do
      #     prefer "stakeholder", not: ["user", "person"]
      #   end
      def glossary(strict: false, &block)
        @glossary_strict = strict
        gb = GlossaryBuilder.new(@glossary_rules)
        gb.instance_eval(&block) if block
      end



      # Define a domain service with the given name and optional configuration block.
      #
      # Domain services orchestrate operations that span multiple aggregates.
      # The block is evaluated in the context of a ServiceBuilder.
      #
      # @param name [String] the service name (e.g. "TransferMoney")
      # @yield block evaluated in the context of ServiceBuilder
      # @return [void]
      def service(name, &block)
        builder = ServiceBuilder.new(name)
        builder.instance_eval(&block) if block
        @services << builder.build
      end



      # Define an aggregate root; raises on duplicate names.
      #
      # Creates an AggregateBuilder, evaluates the block within it, and adds
      # the resulting Aggregate IR to the domain. Duplicate aggregate names
      # raise +ArgumentError+. Errors within the block (other than HecksPlayground::Error
      # subclasses) are wrapped in +HecksPlayground::ValidationError+ with context.
      #
      # @param name [String] the aggregate root name (e.g. "Pizza", "Account")
      # @yield block evaluated in the context of AggregateBuilder
      # @return [void]
      # @raise [ArgumentError] if an aggregate with the same name already exists
      # @raise [HecksPlayground::ValidationError] if the block raises a non-HecksPlayground error
      def aggregate(name, description = nil, definition: nil, &block)
        if @aggregates.any? { |a| a.name == name }
          raise ArgumentError, "Duplicate aggregate name: #{name}"
        end

        builder = AggregateBuilder.new(name)
        desc = definition || description
        builder.description(desc) if desc
        begin
          if block
            AggregateBuilder::VoTypeResolution.with_vo_constants do
              builder.instance_eval(&block)
            end
          end
        rescue HecksPlayground::Error
          raise
        rescue => e
          raise HecksPlayground::ValidationError, "Error in aggregate '#{name}': #{e.message}"
        end
        agg = builder.build
        # i142 — stamp the bluebook namespace as the aggregate's context
        # so Ruby IR matches Rust IR (parser.rs sets this from
        # `HecksPlayground.bluebook "X"`). Both halves of the parity contract
        # emit "context" in their canonical JSON dumpers.
        agg.context = @name if agg.respond_to?(:context=)
        @aggregates << agg
      end

      # Define a paragraph — a named group of aggregates within a chapter.
      #
      # Paragraphs organize a chapter's aggregates into focused sections.
      # The block receives the builder so aggregates can be defined inside it.
      #
      #   paragraph "Ports" do
      #     aggregate "EventBus" do ... end
      #     aggregate "CommandBus" do ... end
      #   end
      #
      # @param name [String] the paragraph name (e.g. "Ports")
      # @yield block evaluated in the context of BluebookBuilder (self)
      # @return [void]
      def paragraph(name, &block)
        before = @aggregates.dup
        instance_eval(&block) if block
        added = @aggregates - before
        @paragraphs << Structure::Paragraph.new(name: name, aggregates: added)
      end



      # Declare the default command for `storehouse run <file>` when the
      # bluebook is marked executable (shebang). Stored on the domain so
      # the Rust runtime can look it up; invisible to the canonical IR
      # dump (parity contract is unchanged).
      #
      #   entrypoint "StartSession"
      #
      def entrypoint(command_name)
        @entrypoint = command_name.to_s
      end

      # Capability dashboards (status, statusline) declare their layout
      # as ordered `section "Title" do row "label", :field … end` blocks.
      # The Rust runner walks these to render — see
      # capabilities/status/status.bluebook + rust/src/run_status/.
      # Ruby parity dump intentionally ignores the rows ; sections are
      # not yet first-class in BluebookModel. The block is evaluated
      # against a tiny no-op SectionBuilder so the DSL parses cleanly
      # under instance_eval.
      def section(_title, &block)
        builder = SectionBuilder.new
        builder.instance_eval(&block) if block
        nil
      end

      # Tiny no-op collector — accepts `row "label", :field` lines so
      # status.bluebook parses on the Ruby side. The Rust parser is the
      # canonical reader of section composition.
      class SectionBuilder
        def row(_label, _field = nil); end
      end

      # Define a cross-aggregate reactive policy.
      #
      # Domain-level policies react to events from one aggregate and trigger
      # commands on another, enabling decoupled cross-context workflows.
      # The block is evaluated in the context of a PolicyBuilder.
      #
      # @param name [String] the policy name (e.g. "DisburseFunds")
      # @yield block evaluated in the context of PolicyBuilder
      # @return [void]
      def policy(name, &block)
        builder = PolicyBuilder.new(name)
        builder.instance_eval(&block) if block
        @policies << builder.build
      end

      # Accept-and-ignore: legacy nursery bluebooks declare `lifecycle` at
      # the top level (outside any `aggregate` block). The canonical DSL
      # attaches `lifecycle` to a specific aggregate. Rust's line-scanner
      # ignores top-level lifecycle — this mirrors that behavior so parity
      # passes while migration continues.
      #
      #   HecksPlayground.bluebook "Acoustics" do
      #     aggregate "Room" do ... end
      #     lifecycle "AcousticDesign" do ... end  # no-op here
      #   end
      def lifecycle(*_args, **_kwargs, &_block)
        # no-op — canonical location is inside `aggregate`
      end

      # Accept-and-ignore: legacy nursery bluebooks declare `event` at the
      # top level (outside any `aggregate` block). The canonical DSL
      # attaches explicit events to a specific aggregate. Rust's
      # line-scanner silently skips top-level `event` lines — mirroring
      # that behavior keeps parity green while migration continues.
      #
      #   HecksPlayground.bluebook "Quality" do
      #     aggregate "Test" do ... end
      #     event "TestRun"     # no-op here
      #   end
      def event(*_args, **_kwargs, &_block)
        # no-op — canonical location is inside `aggregate`
      end

      # Define a read model (view) projected from domain events.
      #
      # Read models are denormalized projections built by applying event
      # handlers. The block is evaluated in the context of a ReadModelBuilder.
      #
      # @param name [String] the read model name (e.g. "OrderSummary")
      # @yield block evaluated in the context of ReadModelBuilder
      # @return [void]
      def view(name, &block)
        builder = ReadModelBuilder.new(name)
        builder.instance_eval(&block) if block
        @views << builder.build
      end

      # Define a multi-step workflow composed of commands and branches.
      #
      # Workflows orchestrate sequences of commands with optional branching
      # logic based on specification predicates. The block is evaluated in
      # the context of a WorkflowBuilder.
      #
      # @param name [String] the workflow name (e.g. "LoanApproval")
      # @yield block evaluated in the context of WorkflowBuilder
      # @return [void]
      def workflow(name, &block)
        builder = WorkflowBuilder.new(name)
        builder.instance_eval(&block) if block
        @workflows << builder.build
      end

      # Build and return the BluebookModel::Structure::Domain IR object.
      #
      # Assembles all collected domain-level elements into an immutable
      # Domain intermediate representation.
      #
      # @return [BluebookModel::Structure::Domain] the fully built domain IR object
      def build
        domain = Structure::Domain.new(
          name: @name, version: @version, aggregates: @aggregates, paragraphs: @paragraphs, policies: @policies,
          services: @services, views: @views, workflows: @workflows,
          actors: @actors,
          sagas: @sagas, process_managers: @process_managers,
          cadences: @cadences, block_grammars: @block_grammars,
          glossary_rules: @glossary_rules,
          glossary_strict: @glossary_strict || false,
          description: @description,
          vision: @vision, subdomain: @subdomain,
          glossary_terms: @glossary_terms,
          sme: @sme,
          category: @category
        )
        classify_references(domain)
        domain
      end

      private

      def classify_references(domain)
        agg_names = domain.aggregates.map(&:name)
        domain.aggregates.each do |agg|
          local_types = agg.value_objects.map(&:name) + agg.entities.map(&:name)
          (agg.references || []).each do |ref|
            # DSL keywords (has_one / has_many / belongs_to / reference_to)
            # set ref.kind at construction — that authored intent IS the
            # canonical IR kind (matches Rust's ReferenceKind exactly).
            # Legacy composition / aggregation / cross_context derivation
            # only runs when no DSL keyword has tagged the ref.
            next unless ref.kind.nil?
            ref.kind = if ref.domain
                         :cross_context
                       elsif local_types.include?(ref.type)
                         :composition
                       elsif agg_names.include?(ref.type)
                         :aggregation
                       else
                         :aggregation
                       end
          end
        end
      end
    end
  end
end
