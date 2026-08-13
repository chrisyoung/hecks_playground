# Hecksagain::Adapters::Shell — the execution port's shell implementation.
#
# Vendored addition, not (yet) upstream hecksagain. Answers Tools::ShellTool
# (bound `Tools::ShellTool.executed_by("Shell")`), the impure edge behind the
# most consequential act the door carries. See ports/execution.rb for why
# this port exists at all.
#
# Usage (never called directly — the dispatcher reaches it through the port):
#   Hecksagain::Adapters::Shell.execute("Bash", shell_command: "echo hi")
#   # => { tool: "bash", output: "hi\n", exit_code: 0, ok: true }
#
# CWD is the process's own working directory, inherited, NOT the registry
# root: a corpus booted through HecksagainRuntime is staged into a
# deterministic tmp copy, so registry.root is a throwaway directory under
# Dir.tmpdir and running commands there would silently resolve every
# relative path against a stage. The process cwd is the real project.

require "open3"

module Hecksagain
  module Adapters
    module Shell
      TOOL = "bash".freeze

      # Matches perform_exec.hecksagon's own documented cap, so one dispatch
      # cannot blow up downstream state with an unbounded capture.
      OUTPUT_LIMIT = 100_000

      module_function

      def execute(operation, args)
        case operation
        when "Bash" then run(args[:shell_command].to_s)
        else refuse(operation)
        end
      end

      # /bin/sh, matching the door's own documented contract ("ShellTool.Bash
      # runs under /bin/sh — no bash process-substitution"). Passed as an
      # explicit argv so the string is the shell's script, never re-split by
      # Ruby into a command of its own.
      def run(command)
        return refuse_empty if command.strip.empty?

        stdout, stderr, status = Open3.capture3("/bin/sh", "-c", command)
        code = status.exitstatus || -1
        { tool: TOOL, output: cap(body(stdout, stderr, code)), exit_code: code, ok: code.zero? }
      rescue StandardError => error
        # A failure to LAUNCH is recorded, never raised — the Cascade record
        # is the audit trail, and a raise here would lose both the attempt
        # and the reason it never started.
        { tool: TOOL, output: "#{error.class}: #{error.message}", exit_code: -1, ok: false }
      end

      # On success stdout alone is the answer. On failure stderr is usually
      # the only thing that explains it, so both ride out together rather
      # than handing back a silent empty string.
      # SILENCE MUST NOT RENDER AS SUCCESS. A command that exits 0 having
      # printed nothing comes back as an empty string, which lands under a
      # success header as a blank body — and a blank body reads as "nothing
      # is there" when it equally means "the command looked in the wrong
      # place". A grep whose --include excluded the only matching extension,
      # a find rooted at a directory that does not hold what was sought, an
      # ls of a glob that matched nothing: all exit 0 and print nothing.
      # This cannot make a shell tool honest about its own scope — only the
      # tool knows what it looked at — but it can stop emptiness from
      # arriving disguised as an answer.
      def body(stdout, stderr, code)
        return stdout if code.zero? && !stdout.empty?

        joined = [stdout, stderr].reject { |stream| stream.nil? || stream.empty? }.join("\n")
        return joined unless joined.empty?

        "... [no output - exited #{code}, wrote nothing to stdout or stderr]"
      end

      def cap(text)
        return text if text.length <= OUTPUT_LIMIT

        "#{text[0, OUTPUT_LIMIT]}\n... [truncated at #{OUTPUT_LIMIT} bytes]"
      end

      def refuse_empty
        { tool: TOOL, output: "no shell_command was given", exit_code: -1, ok: false }
      end

      def refuse(operation)
        { tool: TOOL, output: "the Shell adapter implements Bash, not #{operation}",
          exit_code: -1, ok: false }
      end
    end
  end
end
