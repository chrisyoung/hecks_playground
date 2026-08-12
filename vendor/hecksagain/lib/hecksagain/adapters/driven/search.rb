# Hecksagain::Adapters::Search — the execution port's filesystem-search
# implementation.
#
# Vendored addition, not (yet) upstream hecksagain. Answers
# Tools::SearchTool (bound `Tools::SearchTool.executed_by("Search")`) —
# Grep and Glob. See ports/execution.rb for why this port exists.
#
# Usage (never called directly — the dispatcher reaches it through the port):
#   Hecksagain::Adapters::Search.execute("Glob", glob_pattern: "*.rb",
#                                                search_path: "/tmp")
#   # => { tool: "glob", output: "/tmp/a.rb\n/tmp/b.rb", exit_code: 0, ok: true }
#
# Grep shells out to ripgrep when it is on PATH and falls back to POSIX
# `grep -rn` when it is not, rather than reimplementing a recursive walk in
# Ruby: both honour .gitignore-less recursive semantics closely enough for a
# search tool, and neither pulls in a dependency. Glob is pure Ruby —
# Dir.glob already IS the primitive, so shelling out would only add a
# process.

require "open3"

module Hecksagain
  module Adapters
    module Search
      OUTPUT_LIMIT = 100_000

      # Exit 1 from grep/rg means "searched fine, matched nothing" — a valid
      # answer, not a failure. Only 2+ is a real error.
      NO_MATCHES = 1

      module_function

      def execute(operation, args)
        case operation
        when "Grep" then grep(args)
        when "Glob" then glob(args)
        else refuse(operation)
        end
      rescue StandardError => error
        fail_with(operation.to_s.downcase, "#{error.class}: #{error.message}")
      end

      def grep(args)
        pattern = args[:pattern].to_s
        path    = present(args[:search_path]) || Dir.pwd
        return fail_with("grep", "no pattern was given") if pattern.empty?

        stdout, stderr, status = Open3.capture3(*grep_argv(pattern, path))
        code = status.exitstatus || -1
        return ok("grep", cap(stdout.empty? ? "no matches for #{pattern.inspect} in #{path}" : stdout)) if code.zero? || code == NO_MATCHES

        fail_with("grep", stderr.empty? ? "grep exited #{code}" : stderr)
      end

      def grep_argv(pattern, path)
        return ["rg", "--line-number", "--no-heading", "--color", "never", pattern, path] if ripgrep?

        ["grep", "-rn", "--", pattern, path]
      end

      def ripgrep?
        return @ripgrep unless @ripgrep.nil?

        @ripgrep = ENV.fetch("PATH", "").split(File::PATH_SEPARATOR)
                      .any? { |dir| File.executable?(File.join(dir, "rg")) }
      end

      def glob(args)
        pattern = args[:glob_pattern].to_s
        path    = present(args[:search_path]) || Dir.pwd
        return fail_with("glob", "no glob_pattern was given") if pattern.empty?

        matches = Dir.glob(File.join(path, pattern)).sort
        ok("glob", cap(matches.empty? ? "no paths matched #{pattern.inspect} under #{path}" : matches.join("\n")))
      end

      def present(value)
        text = value.to_s
        text.empty? ? nil : text
      end

      def cap(text)
        return text if text.length <= OUTPUT_LIMIT

        "#{text[0, OUTPUT_LIMIT]}\n... [truncated at #{OUTPUT_LIMIT} bytes]"
      end

      def ok(tool, output)        = { tool: tool, output: output, exit_code: 0, ok: true }
      def fail_with(tool, output) = { tool: tool, output: output, exit_code: 1, ok: false }

      def refuse(operation)
        fail_with("search", "the Search adapter implements Grep and Glob, not #{operation}")
      end
    end
  end
end
