# Hecksagain::Adapters::Filesystem — the execution port's file-IO
# implementation.
#
# Vendored addition, not (yet) upstream hecksagain. Answers Tools::FileTool
# (bound `Tools::FileTool.executed_by("Filesystem")`) — Read / Write /
# Update / Edit. See ports/execution.rb for why this port exists.
#
# Usage (never called directly — the dispatcher reaches it through the port):
#   Hecksagain::Adapters::Filesystem.execute("Read", file_path: "/tmp/x")
#   # => { tool: "read", output: "...", exit_code: 0, ok: true }
#
# Read returns the file's text VERBATIM — no line-number gutter. The gutter
# is a presentation choice of one particular agent harness, and an Edit
# whose old_string was copied out of a gutter-decorated read would never
# match. Verbatim in, verbatim out, so a read and an edit compose.

require "fileutils"

module Hecksagain
  module Adapters
    module Filesystem
      OUTPUT_LIMIT = 100_000

      module_function

      def execute(operation, args)
        case operation
        when "Read"   then read(args)
        when "Write"  then write(args, tool: "write")
        when "Update" then write(args, tool: "write")
        when "Edit"   then edit(args)
        else refuse(operation)
        end
      rescue StandardError => error
        fail_with(operation.downcase, "#{error.class}: #{error.message}")
      end

      def read(args)
        path = args[:file_path].to_s
        return fail_with("read", "#{path}: no such file") unless File.file?(path)

        lines = File.readlines(path)
        lines = slice(lines, args[:offset], args[:limit])
        ok("read", cap(lines.join))
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

        File.write(path, truthy?(args[:replace_all]) ? source.gsub(old_string, new_string) : source.sub(old_string, new_string))
        ok("edit", "replaced #{truthy?(args[:replace_all]) ? count : 1} occurrence(s) in #{path}")
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
        fail_with("file", "the Filesystem adapter implements Read, Write, Update and Edit, not #{operation}")
      end
    end
  end
end
