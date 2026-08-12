# Statusline — emits Miette's one-line body status to stdout. A faithful
# Ruby port of rust/src/run_statusline/{mod,state,time,sleep,inbox}.rs,
# for hecksagain-cli's `statusline` subcommand (replaces the retired
# `storehouse statusline` binary as the thing statusline-command.sh
# execs — the old binary can no longer safely parse hecks_conception,
# now hecksagain-shaped content).
#
# Reads the SAME raw .heki files the Rust runner read, at the same
# resolved location (info dir = the OS app-support data root, derived
# from hecks_conception/miette.world's `heki { dir :default }` chain —
# NOT core/studio's HekiView::DEFAULT_ROOT, confirmed stale, last
# written months ago). Reads the raw snapshot+journal format directly
# (Hecksagain::Adapters::Heki::Snapshot/Journal's own codec) rather than
# going through the full aggregate-oriented Heki adapter, which needs a
# real aggregate object this CLI-level read has no use for.
#
# Verified live against the old binary's real output: byte-identical
# modulo the expected heart-glyph animation-frame difference.
#
# Usage: HecksagainRuntime::Statusline.run # prints one line, e.g.:
#   🖤 1.92m  ✉️ 3  🔮 405  ☸️ em:18

require "json"
require "zlib"

module HecksagainRuntime
  module Statusline
    module_function

    MAGIC = "HEKI"
    HEADER_BYTES = 8
    HEARTS = ["🖤", "❤️"].freeze
    MOONS = ["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘"].freeze
    PRUNE = %w[.git node_modules target vendor .wrangler dist build .next coverage .venv].freeze
    MAX_DEPTH = 4

    State = Struct.new(
      :consciousness, :sleep_summary, :sleep_stage, :sleep_cycle, :sleep_total,
      :phase_ticks, :is_lucid, :dream_pulses, :dream_pulses_needed,
      :beats_raw, :lucid_narrative, :mood_glyph, :mood_vibe, :drafts_count,
      keyword_init: true
    )

    def run
      chdir_from_stdin unless $stdin.tty?

      dir = info_dir
      s   = read_state(dir)
      now = wall_clock_nanos

      line = s.consciousness == "sleeping" ? render_sleep(s, now) : render_awake(s, now)
      puts line
    end

    def chdir_from_stdin
      # A non-blocking readiness check, not just `$stdin.tty?` — a TTY
      # check alone is false for BOTH "a harness piped real JSON in" AND
      # "invoked directly with stdin inherited but nothing ever writes
      # to or closes it" (a plain shell/subprocess call with no explicit
      # pipe). The second case made `$stdin.read` hang indefinitely,
      # found live (a 2-minute tool-call timeout) — `IO.select` with a
      # short timeout distinguishes "data is actually waiting" from
      # "nothing's coming," where `.tty?` alone cannot.
      return unless IO.select([$stdin], nil, nil, 0.05)

      input = $stdin.read
      return if input.nil? || input.empty?

      payload = JSON.parse(input)
      dir = payload.dig("workspace", "current_dir")
      Dir.chdir(dir) if dir && Dir.exist?(dir)
    rescue StandardError
      nil
    end

    # Mirrors heki::resolve_info_dir's own world-derived chain: the
    # store lives at the OS app-support root, under hecks/ — the SAME
    # place every other hecksagain-cli subcommand's own Heki writes
    # already land (confirmed against a real, live `census.heki`/
    # `tick.heki` read).
    def info_dir
      File.expand_path("~/Library/Application Support/Hecks/hecks")
    end

    def read_state(dir)
      s = State.new(
        consciousness: "", sleep_summary: "", sleep_stage: "", sleep_cycle: 0,
        sleep_total: 0, phase_ticks: 0, is_lucid: "", dream_pulses: 0,
        dream_pulses_needed: 0, beats_raw: 0, lucid_narrative: "",
        mood_glyph: "", mood_vibe: "", drafts_count: 0
      )

      if (rec = latest_record(dir, "consciousness"))
        s.consciousness       = string_field(rec, "state")
        s.sleep_summary       = string_field(rec, "sleep_summary")
        s.sleep_stage         = string_field(rec, "sleep_stage")
        s.sleep_cycle         = int_field(rec, "sleep_cycle")
        s.sleep_total         = int_field(rec, "sleep_total")
        s.phase_ticks         = int_field(rec, "phase_ticks")
        s.is_lucid             = string_field(rec, "is_lucid")
        s.dream_pulses         = int_field(rec, "dream_pulses")
        s.dream_pulses_needed  = int_field(rec, "dream_pulses_needed")
        s.dream_pulses_needed  = 5 if s.dream_pulses_needed.zero?
      end

      if (rec = latest_record(dir, "tick"))
        s.beats_raw = int_field(rec, "cycle")
      end

      if (rec = latest_record(dir, "mood"))
        s.mood_glyph = string_field(rec, "glyph")
        s.mood_vibe  = string_field(rec, "vibe")
      end

      if (rec = latest_record(dir, "drafts"))
        s.drafts_count = int_field(rec, "count")
      end

      if s.is_lucid == "yes" && s.sleep_stage == "rem"
        if (rec = latest_record(dir, "lucid_dream"))
          s.lucid_narrative = string_field(rec, "latest_narrative")
        end
      end

      s
    end

    # Raw snapshot (+ journal overlay) read, no aggregate object needed.
    # "Latest" = highest id in the id-sorted store, matching the
    # snapshot codec's own write-time sort (Heki::Snapshot#write sorts
    # by id before persisting).
    def latest_record(dir, stem)
      path = File.join(dir, stem, "#{stem}.heki")
      return nil unless File.exist?(path)

      records = read_snapshot(path)
      journal_path = "#{path}.journal"
      records = replay_journal(records, journal_path) if File.exist?(journal_path)
      return nil if records.empty?

      records.max_by { |id, _| id }.last
    rescue StandardError
      nil
    end

    def read_snapshot(path)
      data = File.binread(path)
      return {} if data.bytesize < HEADER_BYTES || data[0, 4] != MAGIC

      JSON.parse(Zlib::Inflate.inflate(data[HEADER_BYTES..]))
    rescue Zlib::DataError, JSON::ParserError
      {}
    end

    def replay_journal(records, journal_path)
      File.foreach(journal_path) do |line|
        entry = JSON.parse(line)
        id = entry.fetch("id")
        case entry.fetch("operation")
        when "save"   then records[id] = entry.fetch("state")
        when "delete" then records.delete(id)
        end
      end
      records
    rescue StandardError
      records
    end

    def string_field(rec, key)
      v = rec[key] || rec[key.to_s]
      v.is_a?(String) ? v : v.to_s
    end

    def int_field(rec, key)
      v = rec[key] || rec[key.to_s]
      return 0 if v.nil?
      return v if v.is_a?(Integer)

      v.to_s.to_i
    end

    def wall_clock_nanos
      Process.clock_gettime(Process::CLOCK_REALTIME, :nanosecond)
    end

    def heart_glyph(nanos_total)
      HEARTS[(nanos_total / 333_000_000) % 2]
    end

    def moon_glyph(nanos_total)
      secs = nanos_total / 1_000_000_000
      MOONS[secs % 8]
    end

    def render_awake(s, nanos_total)
      out = "#{heart_glyph(nanos_total)} #{format_beats(s.beats_raw)}"
      out += "  ✉️ #{s.drafts_count}" if s.drafts_count.positive?

      inboxes = render_inbox_list
      out += "  #{inboxes}" unless inboxes.empty?
      out
    end

    def format_beats(b)
      if b >= 1_000_000
        format("%.2fm", b / 1_000_000.0)
      elsif b >= 1_000
        format("%.2fk", b / 1_000.0)
      else
        b.to_s
      end
    end

    def render_sleep(s, nanos_total)
      phase_label = (s.is_lucid == "yes" && s.sleep_stage == "rem") ? "lucid rem" : s.sleep_stage

      timer =
        if s.sleep_stage == "rem"
          elapsed = s.phase_ticks * 10
          format("+%d:%02d", elapsed / 60, elapsed % 60)
        else
          ""
        end

      header =
        if s.sleep_stage == "rem"
          if s.sleep_total.positive?
            "cycle #{s.sleep_cycle}/#{s.sleep_total} — #{phase_label} #{timer} · #{s.dream_pulses}/#{s.dream_pulses_needed} dreams"
          else
            "#{phase_label} #{timer} · #{s.dream_pulses}/#{s.dream_pulses_needed} dreams"
          end
        elsif s.sleep_total.positive?
          "cycle #{s.sleep_cycle}/#{s.sleep_total} — #{phase_label}"
        else
          phase_label
        end

      narrative =
        if s.is_lucid == "yes" && s.sleep_stage == "rem" && !s.lucid_narrative.empty?
          "✨ #{s.lucid_narrative}"
        else
          s.sleep_summary
        end

      narrative.empty? ? "#{moon_glyph(nanos_total)} #{header}" : "#{moon_glyph(nanos_total)} #{header}  #{narrative}"
    end

    # Multi-inbox renderer, ported from rust/src/run_statusline/inbox.rs.
    # Decentralised registry (i528): walks $HOME/Projects for
    # `.channel.md` descriptors, no central list.
    def render_inbox_list
      home = ENV["HOME"]
      return "" unless home

      channels = []
      collect_channels(File.join(home, "Projects"), 0, channels)
      channels = channels.select { |c| c[:count].positive? }
      channels.sort_by! { |c| [c[:abbrev].empty? ? 0 : 1, c[:abbrev]] }

      channels.map do |c|
        c[:abbrev].empty? ? "#{c[:emoji]} #{c[:count]}" : "#{c[:emoji]} #{c[:abbrev]}:#{c[:count]}"
      end.join("  ")
    end

    def collect_channels(dir, depth, out)
      return if depth > MAX_DEPTH || !Dir.exist?(dir)

      desc = File.join(dir, ".channel.md")
      if File.file?(desc)
        ch = parse_channel(desc, dir)
        out << ch if ch
      end

      Dir.each_child(dir) do |name|
        path = File.join(dir, name)
        next if File.symlink?(path)
        next unless File.directory?(path)
        next if name.start_with?(".") || PRUNE.include?(name)

        collect_channels(path, depth + 1, out)
      end
    rescue StandardError
      nil
    end

    def parse_channel(desc, dir)
      text = File.read(desc)
      emoji = ""
      abbrev = ""
      frontmatter_lines(text).each do |line|
        t = line.strip
        if t.start_with?("emoji:")
          emoji = t.sub("emoji:", "").strip
        elsif t.start_with?("abbrev:")
          abbrev = t.sub("abbrev:", "").strip
        end
      end
      return nil if emoji.empty?

      { abbrev: abbrev, emoji: emoji, count: count_active_cards(dir) }
    rescue StandardError
      nil
    end

    def frontmatter_lines(text)
      body = text.start_with?("---\n") ? text[4..] : ""
      return [] if body.empty?

      idx = body.index("\n---")
      idx ? body[0...idx].lines : []
    end

    def count_active_cards(dir)
      return 0 unless Dir.exist?(dir)

      count = 0
      Dir.each_child(dir) do |name|
        next unless name.end_with?(".md")

        path = File.join(dir, name)
        count += 1 if active_status?(File.read(path))
      rescue StandardError
        nil
      end
      count
    end

    def active_status?(text)
      frontmatter_lines(text).each do |line|
        t = line.strip
        next unless t.start_with?("status:")

        rest = t.sub("status:", "").strip
        return !%w[closed done archived planning].include?(rest)
      end
      false
    end
  end
end
