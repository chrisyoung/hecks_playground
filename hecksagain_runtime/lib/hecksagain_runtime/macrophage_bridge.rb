# macrophage_bridge.rb — Ruby port of rust/cli/src/main.rs's run_macrophage.
#
# Reads a PostToolUse hook payload (already parsed JSON), classifies the
# touched file, and dispatches the matching Macrophage::* command --
# RecordBluebookEdit / RecordImperativeEdit / RecordExemptedEdit /
# RecordSupportEdit / RecordOtherEdit -- mirroring the Rust classify_file +
# run_macrophage pair (rust/cli/src/main.rs:5293-5306, :4710-4830).
#
# KNOWN SIMPLIFICATION, documented rather than silently dropped: the Rust
# version also asks the IR-query substrate whether an imperative file is
# STRUCTURALLY exempt (a hecksagon ShellAdapter/specializer/capability-runner
# row referencing it) before falling back to exempt_registry.heki. This port
# only checks exempt_registry.heki. Structural exemption detection is real,
# separately-scoped follow-up work -- flagging here rather than pretending
# parity. Until it lands, a file that used to be exempted structurally will
# report RecordImperativeEdit here (the SAFER direction to be wrong in --
# over-complaining, never under-complaining).
#
# SINGLETON IDENTITY, found live (storehouse-mcp rewrite, migration plan
# Part 5 "correction" pass): `Macrophage::Macrophage` is `identified_by
# { name.value }` (a computed identity block, not a bare field) and the
# corpus's own commentary calls it "the SINGLETON Macrophage::Macrophage.
# Record*Edit commands" -- but none of the five Record*Edit commands
# declare a `name` attribute of their own, so a dispatch carrying only
# `file_path:` left the runtime nothing to compute the identity from
# ("RecordImperativeEdit creates a Macrophage — pass name.value:").
# Confirmed directly (not guessed) that hecksagain's dispatcher accepts
# `name:` as an implicit identity-resolution kwarg even though the command
# itself never declares it -- the SAME mechanism CLAUDE.md's "self-ref
# dispatch" convention documents for `reference_to`. `SINGLETON_NAME` is
# this bridge's own choice of identity value (not a corpus-declared
# constant -- none exists), stable across calls so every hook invocation
# resolves to the same one live record, matching the aggregate's own
# stated design ("umbrella for the singleton Macrophage aggregate").

require "json"
require "time"

module HecksagainRuntime
  module MacrophageBridge
    BLUEBOOK_EXT   = %w[bluebook hecksagon fixtures behaviors world].freeze
    IMPERATIVE_EXT = %w[rs sh rb js jsx ts tsx py go c cpp h hpp java swift].freeze
    SUPPORT_EXT    = %w[md heki json toml yaml yml txt].freeze
    SINGLETON_NAME = "the-macrophage"

    def self.call(root, payload)
      return { ok: true, skipped: "HECKS_GOVERNANCE_OFF" } if ENV["HECKS_GOVERNANCE_OFF"]

      tool_name  = payload["tool_name"].to_s
      file_path  = payload.dig("tool_input", "file_path").to_s
      file_path  = detect_bash_write_target(payload.dig("tool_input", "command").to_s) if file_path.empty? && tool_name == "Bash"
      return { ok: true, skipped: "no file target" } if file_path.to_s.empty?

      kind      = classify_file(file_path)
      exempted  = kind == :imperative && exempt_registry_hit?(root, file_path)
      cmd_name  = case kind
                  when :bluebook   then "RecordBluebookEdit"
                  when :imperative then exempted ? "RecordExemptedEdit" : "RecordImperativeEdit"
                  when :support    then "RecordSupportEdit"
                  when :downstream then "RecordDownstreamEdit"
                  else "RecordOtherEdit"
                  end

      result = HecksagainRuntime.dispatch(root, "Macrophage::Macrophage.#{cmd_name}", file_path: file_path, name: SINGLETON_NAME)
      result.merge(kind: kind, exempted: exempted, dispatched: cmd_name)
    rescue StandardError => e
      { ok: false, error: e.message, error_class: e.class.name }
    end

    def self.classify_file(path)
      return :downstream if path.include?("/tests/") || path.include?("_test.") || path.end_with?("_spec.rb")

      ext = path.to_s.split(".").last.to_s.downcase
      return :bluebook   if BLUEBOOK_EXT.include?(ext)
      return :imperative if IMPERATIVE_EXT.include?(ext)
      return :support    if SUPPORT_EXT.include?(ext)

      :other
    end

    def self.detect_bash_write_target(cmd)
      # Mirrors the Rust fast-path: only bother if the command mentions a
      # kernel-surface extension at all. Loose but matches the original's
      # intent -- catch `python3 -c "open('foo.rs','w')..."`-style bypasses.
      match = cmd.to_s.match(/([\w\/.\-]+\.(?:#{IMPERATIVE_EXT.join('|')}))/)
      match && match[1]
    end

    def self.exempt_registry_hit?(root, file_path)
      # information/antibody/exempt_registry.heki lives beside `root` (the
      # aggregates dir), not inside it -- same relative layout the Rust
      # version resolved via resolve_aggregates_dir's parent. Decoded inline
      # (magic + zlib-deflate + JSON, matching Heki::Snapshot's own private
      # #read_snapshot) rather than reaching into a private instance method.
      registry_path = File.join(File.dirname(root.to_s), "information", "antibody", "exempt_registry.heki")
      return false unless File.exist?(registry_path)

      data = File.binread(registry_path)
      return false if data.bytesize < 8 || data[0, 4] != "HEKI"

      records = JSON.parse(Zlib::Inflate.inflate(data[8..]))
      records.key?(file_path)
    rescue StandardError
      false
    end
  end
end
