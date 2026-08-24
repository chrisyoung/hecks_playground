# HecksPlayground::ThreadContextMethods
#
# Thread-local tenant and actor context.
# Extracted from the HecksPlayground module.
#
module HecksPlayground
  # HecksPlayground::ThreadContextMethods
  #
  # Thread-local tenant and actor context extended onto the HecksPlayground module.
  #
  module ThreadContextMethods
    def tenant
      Thread.current[:hecks_playground_tenant]
    end

    def tenant=(tenant_id)
      Thread.current[:hecks_playground_tenant] = tenant_id&.to_s
    end

    def with_tenant(tenant_id)
      old = Thread.current[:hecks_playground_tenant]
      Thread.current[:hecks_playground_tenant] = tenant_id.to_s
      yield
    ensure
      Thread.current[:hecks_playground_tenant] = old
    end

    def actor
      Thread.current[:hecks_playground_actor]
    end

    def actor=(actor)
      Thread.current[:hecks_playground_actor] = actor
    end

    def with_actor(actor)
      old = Thread.current[:hecks_playground_actor]
      Thread.current[:hecks_playground_actor] = actor
      yield
    ensure
      Thread.current[:hecks_playground_actor] = old
    end

    def current_user
      Thread.current[:hecks_playground_current_user]
    end

    def current_user=(user)
      Thread.current[:hecks_playground_current_user] = user
    end

    def with_user(user)
      old = Thread.current[:hecks_playground_current_user]
      Thread.current[:hecks_playground_current_user] = user
      yield
    ensure
      Thread.current[:hecks_playground_current_user] = old
    end
  end
end
