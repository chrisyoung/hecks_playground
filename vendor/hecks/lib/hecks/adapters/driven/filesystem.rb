# Hecks::Adapters::Filesystem — the execution port's file-IO
# implementation.
#
# Vendored addition, not (yet) upstream hecks. Answers Tools::FileTool
# (bound `Tools::FileTool.executed_by("Filesystem")`) — Read / Write /
# Update / Edit. See ports/execution.rb for why this port exists.
#
# Usage (never called directly — the dispatcher reaches it through the port):
#   Hecks::Adapters::Filesystem.execute("Read", file_path: "/tmp/x")
#   # => { tool: "read", output: "...", exit_code: 0, ok: true }
#
# Read returns the file's text VERBATIM — no line-number gutter. The gutter
# is a presentation choice of one particular agent harness, and an Edit
# whose old_string was copied out of a gutter-decorated read would never
# match. Verbatim in, verbatim out, so a read and an edit compose.

require "fileutils"

module Hecks
  module Adapters
    module Filesystem
      OUTPUT_LIMIT = 100_000

      # Cascade's ToolKind documents a CLOSED set ("bash"/"edit"/"read"/
      # "write"/"grep"/"glob"/"web_fetch"/"web_search"). One lookup used by
      # the success path, the failure path and the rescue alike — an outcome
      # that reports a different tool kind depending on whether it worked
      # makes the audit trail's own vocabulary drift exactly where it is
      # being read most carefully.
      TOOL_KINDS = { "Read" => "read", "Write" => "write",
                     "Update" => "write", "Edit" => "edit" }.freeze

      module_function

      def execute(operation, args)
        case operation
        when "Read"            then read(args)
        when "Write", "Update" then write(args, tool: kind(operation))
        when "Edit"            then edit(args)
        else refuse(operation)
        end
      rescue StandardError => error
        fail_with(kind(operation), "#{error.class}: #{error.message}")
      end

      def kind(operation) = TOOL_KINDS.fetch(operation.to_s, "file")

      def read(args)
        path = args[:file_path].to_s
        return fail_with("read", "#{path}: no such file") unless File.file?(path)

        lines = File.readlines(path)
        # An empty file and a successful read that returned nothing are
        # indistinguishable as a blank body at exit 0. Say which it is.
        return ok("read", "... [empty file: 0 lines]") if lines.empty?

        window = slice(lines, args[:offset], args[:limit])
        shown  = fit(window)
        ok("read", shown.join + coverage(lines, shown, args[:offset]))
      end

      # A FRAGMENT MUST ANNOUNCE ITSELF. `read` knows the file's true length
      # at the very moment it decides to hand back less than all of it, and
      # throwing that away is what makes a 20-line window shape-identical to
      # a whole-file read. The reader then cannot tell "this is the file"
      # from "this is the first tenth of it", so a claim formed from the
      # fragment is voiced as a claim about the whole — a tool that
      # recognises nothing refuses nothing, and agrees with whoever called
      # it. Coverage is the fact that makes refusal possible.
      #
      # Appended as a trailing footer, never a gutter: a WHOLE read returns
      # byte-verbatim as the header promises, so read and edit still compose.
      # Only a genuine fragment carries the extra line. Same shape as cap's
      # own "... [truncated at N bytes]" notice.
      def coverage(lines, shown, offset)
        return "" if shown.size >= lines.size

        from = offset.to_i
        from = 0 if from.negative?
        return "\n... [fragment: no lines at line #{from + 1} of #{lines.size}]" if shown.empty?

        "\n... [fragment: lines #{from + 1}-#{from + shown.size} of #{lines.size} " \
          "- #{lines.size - shown.size} not shown]"
      end

      # The byte cap cuts on WHOLE LINES so that coverage can be counted
      # from what actually survived it. Cutting mid-line and then counting
      # the PRE-cut window is how a truncation notice comes to claim more
      # lines than it showed — a number that agrees with the caller instead
      # of with the file, which is the very failure this footer exists to
      # end. One honest footer, not two notices that disagree.
      #
      # A single line longer than the whole budget is kept, byte-capped, so
      # an enormous minified line reports as one shown line rather than
      # vanishing into "no lines".
      def fit(lines)
        kept = []
        size = 0
        lines.each do |line|
          break if size + line.bytesize > OUTPUT_LIMIT

          kept << line
          size += line.bytesize
        end
        kept.empty? && !lines.empty? ? [cap(lines.first)] : kept
      end

      def write(args, tool:)
        path    = args[:file_path].to_s
        content = args[:content].to_s
        FileUtils.mkdir_p(File.dirname(path))
        File.write(path, content)
        ok(tool, "wrote #{content.bytesize} bytes to #{path}")
      end

      # Exact-string substitution, and a match count that must be exactly one
      # unless replace_all was asked for — the same contract the door's own
      # documentation states ("exact-string"). An ambiguous edit is refused
      # rather than guessed at, because guessing silently changes the wrong
      # line and nothing downstream can tell.
      def edit(args)
        path = args[:file_path].to_s
        return fail_with("edit", "#{path}: no such file") unless File.file?(path)

        old_string = args[:old_string].to_s
        new_string = args[:new_string].to_s
        source     = File.read(path)
        count      = source.scan(old_string).size

        return fail_with("edit", "old_string was not found in #{path}") if count.zero?
        if count > 1 && !truthy?(args[:replace_all])
          return fail_with("edit", "old_string appears #{count} times in #{path} — " \
                                   "pass replace_all, or give a longer unique string")
        end

        # THE BLOCK FORM IS NOT A STYLE CHOICE. `sub(old_string, new_string)`
        # matches literally (String pattern) but still interpolates
        # backreferences in the REPLACEMENT — `\1`, `\0`, `\&`, `\\` get
        # substituted rather than written. Editing any file whose new text
        # contains a backslash (a regex, a "\n" inside a string literal, a
        # Windows path) would silently write the wrong bytes, and a
        # silently-wrong edit is worse than a refused one. A block's return
        # value is never interpolated.
        all     = truthy?(args[:replace_all])
        updated = all ? source.gsub(old_string) { new_string } : source.sub(old_string) { new_string }
        File.write(path, updated)
        ok("edit", "replaced #{all ? count : 1} occurrence(s) in #{path}")
      end

      def slice(lines, offset, limit)
        from  = offset.to_i
        from  = 0 if from.negative?
        taken = lines[from..] || []
        limit.to_i.positive? ? taken.first(limit.to_i) : taken
      end

      def truthy?(value) = [true, "true", "1", 1].include?(value)

      def cap(text)
        return text if text.length <= OUTPUT_LIMIT

        "#{text[0, OUTPUT_LIMIT]}\n... [truncated at #{OUTPUT_LIMIT} bytes]"
      end

      def ok(tool, output)   = { tool: tool, output: output, exit_code: 0,  ok: true }
      def fail_with(tool, output) = { tool: tool, output: output, exit_code: 1, ok: false }

      def refuse(operation)
        fail_with(kind(operation),
                  "the Filesystem adapter implements Read, Write, Update and Edit, not #{operation}")
      end
    end
  end
end
