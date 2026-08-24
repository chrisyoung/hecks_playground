module HecksPlayground
  module DSL
    class BluebookBuilder
      # HecksPlayground::DSL::BluebookBuilder::ElsewhereWords
      #
      # THE FENCE. A bluebook composes a DOMAIN — aggregates, the events they
      # announce, the rules they keep. Words that wire a domain into a runtime
      # (adapters, gates, tenancy, subscriptions) belong to the hecksagon, and
      # words about what the software owes the world belong to the world file.
      # They are different documents because they answer to different people.
      #
      # Those words used to be callable here anyway — some defined on the
      # builder, some mixed in from Hecksagon::StrategicDSL and
      # Hecksagon::ExtensionsDSL. `tenancy` had already been moved to the
      # hecksagon and left behind as `def tenancy(_strategy); end`, a keyword
      # that parsed and did nothing, which is worse than one that is absent :
      # an author writes it, the file is accepted, and the setting silently
      # never happens.
      #
      # So the words are gone, and this says where each one went. A NoMethodError
      # would fence the surface just as well and teach nothing.
      #
      #   HecksPlayground.bluebook "Pizzas" do
      #     tenancy :row
      #   end
      #   # => tenancy is a hecksagon word — it belongs in the .hecksagon file,
      #   #    not the bluebook. A bluebook composes a domain ; wiring it into a
      #   #    runtime is the hecksagon's job.
      #
      module ElsewhereWords
        # Each evicted word, and the document that owns it now. The reason is
        # written per-word rather than shared, because "it moved" is not the
        # useful half — WHY it is not a domain concern is.
        HOMES = {
          tenancy:
            ["hecksagon", "how rows are partitioned between tenants is a runtime " \
                          "arrangement, not something the domain knows about itself"],
          on_event:
            ["hecksagon", "a block that runs when an event fires is a subscription " \
                          "— wiring. The domain's own reactions are `policy`"],
          driving_port:
            ["hecksagon", "ports are how the outside reaches the domain, which is " \
                          "the hexagon's whole subject"],
          driven_port:
            ["hecksagon", "ports are how the domain reaches the outside, which is " \
                          "the hexagon's whole subject"],
          domain_module:
            ["hecksagon", "grouping aggregates into Ruby modules is packaging, and " \
                          "packaging is a target's concern"],
          entry_point:
            ["hecksagon", "naming the .rb file that sets up autoloads is packaging. " \
                          "(`entrypoint`, the default command, is a bluebook word " \
                          "and stays.)"],
          shared_kernel:
            ["hecksagon", "which domains share a kernel is a statement about a " \
                          "system of domains, made where they are wired together"],
          uses_kernel:
            ["hecksagon", "which domains share a kernel is a statement about a " \
                          "system of domains, made where they are wired together"],
          anti_corruption_layer:
            ["hecksagon", "translating another context's language happens at the " \
                          "boundary, which is the hexagon"],
          published_event:
            ["hecksagon", "publishing an event to other contexts is an integration " \
                          "decision ; `emits` is how an aggregate announces one"],
          world_concerns:
            ["world", "what the software owes the people it touches is the world " \
                      "file's subject, and it outlives any one domain"],
          load_from:
            [nil, "a bluebook is read, not executed — `load_from` evaluated " \
                  "arbitrary Ruby from a path, which no other target can follow. " \
                  "Split a domain across files with one bluebook per chapter"]
        }.freeze

        def method_missing(name, *args, &block)
          home, reason = HOMES[name]
          return super unless reason

          raise HecksPlayground::ValidationError, ElsewhereWords.refusal(name, home, reason)
        end

        def respond_to_missing?(name, include_private = false)
          return false if HOMES.key?(name)

          super
        end

        def self.refusal(name, home, reason)
          where = home ? "is a #{home} word — it belongs in the .#{home} file, not the bluebook"
                       : "is not a bluebook word"
          "#{name} #{where}. A bluebook composes a domain : #{reason}."
        end
      end
    end
  end
end
