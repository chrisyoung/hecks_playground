# HecksPlayground::BluebookModel::PredicateSource
#
# The SOURCE TEXT of a predicate block, recovered with Prism.
#
#   PredicateSource.canonical(proc { cents > 0 })  # => "cents > 0"
#
# WHY THIS EXISTS. Value-object invariants and derivations are declared as Ruby
# blocks, and the canonical IR excluded their bodies with the note "the Ruby
# side holds the predicate as a Proc (source unrecoverable)". That was true when
# it was written. Ruby 3.3 ships Prism, so it is not true now : a block records
# where it was written, and the file can be read.
#
# WHAT THE OMISSION COSTS. An invariant can be INVERTED while keeping its name
# — `cents > 0` becomes `cents < 0` — and the parity contract sees no change,
# because the contract carried only the name. Two runtimes then enforce
# different rules and agree perfectly about it. The name is not the rule.
#
# THE ARROW ONLY RUNS ONE WAY. Ruby source is READ to produce IR ; IR is never
# read to produce Ruby.
#
# A block with no readable source — eval'd, or defined in C — recovers as nil,
# which is the honest answer and the one the dump must then leave out rather
# than guess at.
require "prism"

module HecksPlayground
  module BluebookModel
    module PredicateSource
      # Parsed files, keyed by path. A bluebook holds many predicates and
      # re-parsing the file for each one would be silly.
      TREES = {}

      module_function

      # The canonical text of a block's body, or nil when it has no readable
      # source.
      def canonical(block)
        body = body_source(block)
        body && canonicalise(body)
      end

      def body_source(block)
        return nil unless block.respond_to?(:source_location)

        file, line = block.source_location
        return nil unless file && line && File.readable?(file)

        node_at(file, line)&.body&.slice
      rescue StandardError
        # Recovery is best-effort by construction : a predicate whose source
        # cannot be read must not take a bluebook down at parse time. It
        # recovers as nil and is left out of the contract.
        nil
      end

      # The block written at this line. Prism gives every node its exact
      # location, so the match is positional rather than a guess from text.
      def node_at(file, line)
        found = nil
        walk(tree_for(file)) do |node|
          next unless node.is_a?(Prism::BlockNode)
          next unless node.location.start_line == line

          found ||= node
        end
        found
      end

      def tree_for(file)
        TREES[file] ||= Prism.parse_file(file).value
      end

      def walk(node, &visit)
        return unless node.is_a?(Prism::Node)

        visit.call(node)
        node.compact_child_nodes.each { |child| walk(child, &visit) }
      end

      # One meaning, one text. The Rust parser stores a VO invariant's body as
      # its source lines joined with " && " and trimmed, so a multi-statement
      # body has to arrive here in the same shape or the two runtimes would
      # carry the same rule spelled two ways and the contract would split on a
      # difference that is not one.
      #
      # NO ALIAS FOLDING. An earlier draft folded `.length` to `.size` here, on
      # the reasoning that the interpreter floor treats them as one operation.
      # It does — but it folds them at EVALUATION (interp_expr.rs normalises the
      # `.length` suffix, payload_gate_terms strips either), which is where an
      # alias belongs. Folding again at EXTRACTION bought nothing and cost
      # parity : the Rust parser captures the source verbatim, so the fold was
      # one-sided and drifted four bluebooks by construction.
      #
      # The deeper reason is what this whole extraction is for. The contract
      # carries the predicate so an INVERTED invariant cannot hide behind an
      # unchanged name. A contract that silently rewrites the author's text
      # cannot make that promise — it only narrows which rewrites it hides.
      # Whitespace is normalised because the shape of a multi-line body is not
      # part of the rule ; the words are.
      def canonicalise(source)
        source.to_s
              .split("\n")
              .map(&:strip)
              .reject(&:empty?)
              .join(" && ")
              .gsub(/\s+/, " ")
              .strip
      end
    end
  end
end
