# HecksPlayground::Compiler::SourceTransformer
#
# Transforms Ruby source for binary bundling: strips require/require_relative
# calls, neutralizes chapter loading, and expands compact class syntax
# (class HecksPlayground::Foo::Bar) into nested module form.
#
#   cleaned = SourceTransformer.transform(source)
#
module HecksPlayground
  module Compiler
    module SourceTransformer
      # Transforms source for binary inclusion: strips loading
      # infrastructure and expands compact class syntax.
      #
      # @param source [String] Ruby source code
      # @return [String] transformed source
      def self.transform(source)
        expand_compact_classes(strip_bundled_lines(source))
      end

      # Converts compact class syntax (class HecksPlayground::Foo::Bar) to nested
      # module form so namespaces exist before classes reference them.
      COMPACT_CLASS = /^class\s+((?:\w+::)+)(\w+)(.*)/

      def self.expand_compact_classes(source)
        extra_ends = 0
        result = source.gsub(COMPACT_CLASS) do
          parts = $1.split("::").reject(&:empty?)
          class_name = $2
          rest = $3
          extra_ends += parts.size
          parts.map { |mod| "module #{mod}" }.join("\n") +
            "\nclass #{class_name}#{rest}"
        end
        result + ("end\n" * extra_ends)
      end

      # Strips require, require_relative, chapter loading, Dir[] loading,
      # and require_paragraphs calls from bundled source.
      def self.strip_bundled_lines(source)
        lines = source.lines
        result = []
        skip_depth = 0

        lines.each do |line|
          stripped = line.strip
          if skip_depth > 0
            skip_depth += line.count("(") - line.count(")")
            result << "# [v0] #{line.chomp}\n"
            skip_depth = 0 if skip_depth <= 0
          elsif should_strip?(stripped)
            result << "# [v0] #{stripped}\n"
            if multiline_call?(stripped)
              skip_depth = stripped.count("(") - stripped.count(")")
            end
          else
            result << line
          end
        end
        result.join
      end

      def self.should_strip?(line)
        require_line?(line) ||
          chapter_load?(line) ||
          dir_glob_require?(line) ||
          require_paragraphs?(line) ||
          hecks_playground_autoload?(line)
      end

      HECKS_PLAYGROUND_GEMS = %w[
        hecks_playground bluebook hecksagon hecks_playground_cli hecks_playground_persist hecks_playground_mongodb
        hecksul heckscode hecks_playground_serve hecks_playground_multidomain hecks_playground_targets
        go_hecks_playground node_hecks_playground hecks_playground_ai active_hecks_playground hecks_playground_live hecks_playground_static
      ].freeze

      HECKS_PLAYGROUND_REQUIRE = /\Arequire\s+["'](?:#{HECKS_PLAYGROUND_GEMS.join("|")})/
      HECKS_PLAYGROUND_AUTOLOAD = /\A\s*autoload\s+:\w+,\s+["'](?:#{HECKS_PLAYGROUND_GEMS.join("|")})/

      def self.hecks_playground_autoload?(line)
        line.match?(HECKS_PLAYGROUND_AUTOLOAD)
      end

      def self.require_line?(line)
        line.match?(/\Arequire_relative\s/) || line.match?(HECKS_PLAYGROUND_REQUIRE)
      end

      def self.chapter_load?(line)
        return false if line.match?(/\bdef\s/)
        line.match?(/\A\s*(?:HecksPlayground::)?Chapters\.(load_chapter|load_aggregates|define_paragraphs)\b/)
      end

      def self.require_paragraphs?(line)
        line.match?(/\A\s*require_paragraphs\(/) ||
          line.match?(/\A\s*Chapters\.require_paragraphs\b/)
      end

      def self.dir_glob_require?(line)
        line.match?(/\ADir\[.*\].*\.each\s*\{.*require/)
      end

      def self.multiline_call?(line)
        line.include?("(") && line.count("(") > line.count(")")
      end

      private_class_method :strip_bundled_lines, :expand_compact_classes,
                           :should_strip?, :require_line?,
                           :hecks_playground_autoload?, :chapter_load?,
                           :require_paragraphs?, :dir_glob_require?,
                           :multiline_call?
    end
  end
end
