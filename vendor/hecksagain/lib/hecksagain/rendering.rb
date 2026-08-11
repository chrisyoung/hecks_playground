require "json"

module Hecksagain
  # HOW A VALUE READS INSIDE A REFUSAL, in one place, because a refusal is an
  # ANSWER and its wording is contract — pinned byte-for-byte by the corpus.
  #
  # `inspect` and JSON agree on scalars and disagree on everything composite :
  # a hash is `{"cents"=>100}` against `{"cents":100}`, an array is `["a", "b"]`
  # against `["a","b"]`. Each such spelling is invisible until a composite
  # actually reaches a message, which is why both known cases were found by
  # something deliberately passing the wrong shape rather than by review — the
  # first by hand in spec/corpus/banking.json, the second by bin/fuzz, at a
  # different site, after the first was patched where it was found.
  #
  # So it lives here rather than in whichever file noticed it, and the house style
  # is JSON because that is what every other refusal in the corpus already prints.
  module Rendering
    module_function

    def describe(value)
      case value
      when nil then "nil"
      when Hash, Array then JSON.generate(value)
      else
        # Vendored fix, not (yet) upstream hecksagain (migration plan
        # task 9): a Hecksagain::Runtime::Value reaching a refusal
        # message (MutationApplier's own increment/decrement/multiply
        # `amount`, an Admissibility `current`, ...) had no case here,
        # falling straight through to Ruby's own #inspect and leaking a
        # raw object pointer ("#<Hecksagain::Runtime::Value:0x...>")
        # into an otherwise-correct domain refusal. Duck-typed on
        # `respond_to?(:to_h)` rather than a `require`d `Runtime::Value`
        # constant -- `Runtime::Value` itself requires THIS file
        # (`runtime/value.rb`'s own `require_relative "../rendering"`),
        # so naming it here would be circular -- the same guard
        # Resolver#unwrap_scalar already uses for the identical reason.
        # A single-field wrapper (the overwhelming common case here — an
        # amount, an id, a lifecycle field) unwraps to its bare scalar,
        # described recursively ; a genuinely multi-field value renders
        # as its fields' JSON, same as a bare Hash already does two
        # lines up. Confirmed live via CircuitBreaker::Breaker.
        # RecordFailure / MindStream::MindStream.Think (real-dispatch
        # smoke sweep across miette).
        if value.respond_to?(:to_h) && !value.is_a?(Hash) && !value.is_a?(Array)
          fields = value.to_h
          fields.size == 1 ? describe(fields.values.first) : JSON.generate(fields)
        else
          value.inspect
        end
      end
    end
  end
end
