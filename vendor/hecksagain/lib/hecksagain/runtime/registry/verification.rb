require_relative "../../bluebook/hexagon"

module Hecksagain
  module Runtime
    class Registry
      # The wiring gate: every bind names a declared aggregate, every
      # adapter satisfies the verb its port declares, every world setting is
      # a field the adapter admits, and the default adapter is usable at
      # all. Included into Registry — `verify!` is what a boot calls after
      # loading, and the smaller checks are also called piecemeal by the
      # repository factory.
      module Verification
        def verify!
          verify_default_adapter!
          verify_governed_roles!

          @hecksagons.each_value do |hexagon|
            hexagon.binds.each do |bind|
              # A domain-level default (§0) — `persisted_by "Heki"` bare,
              # applying to whichever aggregates don't override it — names
              # no aggregate of its own, so there's nothing to look up in
              # the bluebook for THIS row specifically. Still validate its
              # own adapter/verb shape (the same reason
              # `verify_default_adapter!` checks the framework-wide
              # default the same way, aggregate-less). Coverage of real
              # aggregates that only resolve THROUGH this default comes
              # from their own dispatch-time `BindingPolicy.resolve` —
              # deliberately not required to be exhaustive here, the same
              # leniency this method already extended to any aggregate
              # left out of an explicit bind list entirely (real test
              # fixtures bind only the aggregates they exercise).
              if bind.aggregate.nil?
                check_verb(bind)
                next
              end

              aggregate = bluebook(hexagon.domain)&.aggregate(bind.aggregate_name)
              raise WiringError, "#{bind.aggregate} is bound but not declared in the bluebook" unless aggregate

              check_verb(bind)

              repository(hexagon.domain, aggregate)
            end
          end
          self
        end

        def verify_default_adapter!
          name = Ports::Persistence::DEFAULT_ADAPTER

          check_verb(
            Bluebook::Bind.new(
              aggregate: "(default)",
              verb:      Ports::Persistence::VERB,
              adapter:   name
            )
          )
          adapter_class(name)
          self
        rescue WiringError => error
          raise WiringError,
                "the default persistence adapter (#{name}) is not usable, so an " \
                "aggregate with no bind could not be given one: #{error.message}"
        end

        # `role` IS REAL ACCESS CONTROL ONLY WHEN GOVERNANCE CAN CHECK IT
        # AGAINST SOMETHING — a command that declares a role but whose
        # domain never attaches Governance would leave that role forever
        # unchecked, exactly the defect ADR 0025 §9 names ("role gates
        # access control by exact string equality ... Governance ...
        # connected to none of it").
        #
        # HERE, AT verify! TIME, NOT HecksagonBuilder#build — moved off
        # the builder (used to be `refuse_ungoverned_roles!`, called once
        # per `Hecks.hecksagon "X" do ... end` block) because a domain can
        # be split across multiple files that all load into the SAME
        # hecksagon (`Registry#add_hecksagon`/`merge_hecksagons` — a base
        # file plus `bluebook/environments/<name>.hecksagon`,
        # `Hecks.boot(path, environment: ...)`). Checking each block's own
        # framework_members, before any merge, meant only the ONE block
        # that happened to declare `uses_framework "Governance"` passed —
        # every other file touching that domain would refuse even though
        # the domain's FINAL, merged wiring was never actually ungoverned.
        # `@hecksagons` holds exactly that final, merged Hecksagon per
        # domain by the time verify! runs, so checking here sees the same
        # shape dispatch itself will.
        #
        # GOVERNANCE ITSELF IS EXEMPT — it cannot `uses_framework` its own
        # aggregates, and it IS the source of truth a role check runs
        # against, the same self-reference `CommandRules::Authorization
        # #governance_attached?` grants it at dispatch time.
        def verify_governed_roles!
          @hecksagons.each_value do |hexagon|
            next if hexagon.domain == "Governance"
            next if hexagon.framework_members.include?("Governance")

            bluebook_ir = bluebook(hexagon.domain)
            next unless bluebook_ir

            offender = governed_commands_in(bluebook_ir).find { |command| !command.role.to_s.empty? }
            next unless offender

            raise WiringError,
                  "#{offender.hecks_fqn} declares role #{offender.role.inspect}, but " \
                  "#{hexagon.domain}'s hecksagon never uses_framework \"Governance\" — role is only " \
                  "real access control once Governance is attached to check it against; without that " \
                  "it is silent decoration, the exact defect this refusal exists to catch"
          end
          self
        end

        # Every command this domain declares, an aggregate's own AND every
        # entity nested inside one — the same reach `refuse_role_mismatch`
        # itself needs at dispatch time, just walked ahead of time here.
        def governed_commands_in(bluebook_ir)
          bluebook_ir.aggregates.flat_map { |aggregate| aggregate.commands + aggregate.entities.flat_map(&:commands) }
        end

        def check_verb(bind)
          port = port_for(bind)
          return if port.verb.to_s == bind.verb.to_s

          raise WiringError,
                "#{bind.adapter} implements the #{port.name} port (verb #{port.verb}) " \
                "and cannot satisfy #{bind.verb}"
        end

        def check_settings(bind, settings)
          adapter = @adapters[bind.adapter]
          return unless adapter

          declared = settings.keys - [:adapter]
          unknown  = declared.reject { |field| adapter.declares?(field) }
          return if unknown.empty?

          raise WiringError,
                "#{bind.adapter} does not declare #{unknown.map(&:inspect).join(', ')} — " \
                "it declares #{adapter.all_fields.map(&:inspect).join(', ')}. " \
                "Add the field to the adapter, or remove it from the world."
        end

        def port_for(bind)
          adapter = @adapters[bind.adapter]
          raise WiringError, "unknown adapter #{bind.adapter.inspect}" unless adapter

          @ports[adapter.port] ||
            raise(WiringError, "adapter #{bind.adapter} declares unknown port #{adapter.port.inspect}")
        end

        def adapter_class(name)
          Adapters.const_get(name)
        rescue NameError
          raise WiringError, "no Ruby adapter implementation for #{name.inspect} " \
                             "(expected Hecksagain::Adapters::#{name})"
        end
      end
    end
  end
end
