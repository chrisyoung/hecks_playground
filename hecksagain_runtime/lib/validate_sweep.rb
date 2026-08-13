# HecksagainRuntime::ValidateSweep
#
# i745's own finding: `hecksagain-cli validate hecks_conception` flattens
# the WHOLE corpus into one combined boot and reports ONE verdict --
# `{ok:true,"valid":true}` even while 36 of the corpus's own 58 sub-roots
# were individually invalid (mostly the family/port gap, since fixed --
# Phase 1a). A single combined boot hides per-root defects two ways: a
# crash anywhere makes the WHOLE thing false with no "which root", and a
# name collision between two roots' declarations can make a broken root
# silently PASS (confirmed live this session: hecks_conception's own
# heki.adapter was missing `field :dir` and only ever "worked" because a
# DIFFERENT bug crashed it before it could load and shadow the vendored
# library's more complete copy -- the combined boot never caught it).
#
# Sweeping every domain root INDIVIDUALLY, isolated, is what actually
# surfaces this class of bug -- same reasoning as behaviors_runner.rb's
# own per-test isolation. Mirrors that file's own files_swept discipline:
# a result that doesn't say how much it looked at is the failure mode this
# whole arc exists to end.
module HecksagainRuntime
  module ValidateSweep
    module_function

    # Every directory under `root` that is itself a domain (carries a
    # `bluebook/` subdir), sorted for deterministic output. Same discovery
    # rule `find <root> -type d -name bluebook` uses, in Ruby.
    def domain_roots_under(root)
      Dir.glob(File.join(root, "**", "bluebook"))
         .select { |d| File.directory?(d) }
         .map { |d| File.dirname(d) }
         .sort
    end

    # `root` itself may or may not be a single domain (has its own
    # `bluebook/` subdir directly). Swept EITHER WAY: a corpus's own
    # top-level content (if any) is a root too, not a special case skipped
    # by the recursive glob missing depth-0.
    def run(root)
      roots = domain_roots_under(root)
      roots << root if File.directory?(File.join(root, "bluebook")) && !roots.include?(root)
      roots.sort!

      # A sweep that found NOTHING is not a clean pass -- `invalid: 0` on
      # zero roots swept reads as "all good" to any caller checking only
      # that field, which is the exact false-positive-on-nothing-found shape
      # this whole sweep exists to end (a bad `root` argument would
      # otherwise silently report success). Refuse instead of claiming a
      # verdict on work that never happened.
      return { ok: false, roots_swept: 0, error: "no domain roots (no bluebook/ dir) found under #{root}" } if roots.empty?

      results = roots.map { |r| { root: r }.merge(validate_one(r)) }
      valid_count = results.count { |r| r[:valid] }

      {
        ok: true,
        roots_swept: roots.size,
        valid: valid_count,
        invalid: roots.size - valid_count,
        results: results
      }
    end

    def validate_one(root)
      Hecks.boot(HecksagainRuntime.stage_flat_corpus(root), install_facade: false)
      { valid: true }
    rescue StandardError => e
      { valid: false, error: e.message, error_class: e.class.name }
    end
  end
end
