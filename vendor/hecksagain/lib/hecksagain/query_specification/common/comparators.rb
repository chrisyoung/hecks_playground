module Hecksagain
  module QuerySpecification
    module Common
      # `none_in_state`, vendored addition not (yet) upstream hecksagain
      # (migration plan task 4): a CROSS-AGGREGATE ANTI-JOIN comparator --
      # `where ref: { none_in_state: "Claim:held" }` holds true when NO
      # record in the named aggregate, keyed by this record's own field
      # value, is currently in the named state. plan.bluebook's own
      # description: "a keyed point lookup (HashMap hit), never a scan" --
      # a real, deliberate, pre-existing feature (WhereOp::NoneInState in
      # the old Rust runtime), not invented here -- see Runtime::
      # QueryInterpreter#holds?'s own comment for the evaluation side.
      COMPARATORS = %i[eq ne gt gte lt lte in contains none_in_state].freeze
    end
    def self.render_value(value) = value.is_a?(Symbol) ? ":#{value}" : value.to_s
  end
end
