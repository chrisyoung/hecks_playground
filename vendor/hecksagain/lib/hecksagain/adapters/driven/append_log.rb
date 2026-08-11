require_relative "heki"

module Hecksagain
  module Adapters
    # Vendored addition, not (yet) upstream hecksagain (migration plan
    # task 4) -- see append_log.adapter's own comment. A real alias, not
    # a stub: Heki's Journal module already implements append-only
    # persistence (ports/persistence/append_only), so AppendLog gets the
    # actual, working behavior, addressable under the name this domain's
    # own vocabulary uses.
    AppendLog = Heki
  end
end
