# hecksagain_runtime.rb — the storehouse-mcp-facing Ruby runtime layer over
# hecksagain. Replaces rust/cli's job (dispatch/query/state/dump/
# describe/validate/macrophage) without preserving its shape — each entry
# point here is designed around what hecksagain's own Loader/Dispatcher/
# Router/Handle/Exporter already return, not around the old CLI's stdout
# contract. See Part 5 of
# /Users/christopheryoung/.claude/plans/okay-so-could-we-elegant-goose.md.
#
# hecksagain-cutover PRD, slice 2.1: `hecksagain` now resolves through the
# Bundler `git:` dependency pinned in the top-level Gemfile (hecks-hecksagain,
# ref-pinned), not `vendor/hecksagain/lib` — that copy stays on disk as a
# fallback for the rest of the wave but is no longer on any live load path.
#
# Usage (one boot per process, cold-spawn correctness-first per the plan --
# the warm daemon is a follow-up once this is proven, not a prerequisite):
#   BUNDLE_GEMFILE=Gemfile bundle exec ruby -Ihecksagain_runtime/lib \
#     -r hecksagain_runtime -e 'puts HecksagainRuntime.dispatch(root, verb, args).to_json'

require "hecks"
require "json"
require "fileutils"
require "digest"
require_relative "behaviors_runner"
require_relative "validate_sweep"
require_relative "hecksagain_runtime/attr_decode"

# Vendored addition, not (yet) upstream hecksagain (migration plan task
# 8): `hecksagain/presentation` is DELIBERATELY not required by `require
# "hecksagain"` itself -- presentation.rb's own header says so explicitly
# ("a project that never boots this file never pays for `rack`, the same
# lazy-dependency discipline the Gemfile's own comment already holds pg/
# oauth2/aws-sdk-lambda to"). Respected here, not overridden : this is a
# GENERIC validate/dispatch/catalog wrapper serving many different
# corpora, so unlike a real hand-authored project boot script (which
# knows in advance whether it needs Presentation), this one doesn't --
# an opportunistic require, rescued if `rack` genuinely isn't installed
# for a caller that never needed it, rather than a hard `require` that
# would force every corpus through this wrapper to carry the dependency.
# Found live: pizzas' own bluebook/web.bluebook (the canonical example
# ITSELF) uses `Hecks::Presentation.configure "Web" do ... end`.
begin
  require "hecks/presentation"
rescue LoadError
  nil
end

module HecksagainRuntime
  # STAGING, not a permanent restructure. hecksagain's Loader boots exactly
  # ONE flat directory into ONE Registry (Adapters::Folder#load_domain --
  # Dir[directory/*.bluebook], non-recursive) -- multiple domains only
  # compose together (cross-domain references, like GovernedDoor.LookupDoor
  # scanning Tools.bluebook's commands) when their files live in the SAME
  # directory. hecks_playground_conception's deeply-nested aggregates/<category>/<name>/
  # bluebook/ layout was never going to compose that way.
  #
  # DECISION (documented, not hidden): rather than a second massive
  # directory-restructuring migration on top of the syntax rewrite, this
  # stages a flattened COPY on every boot rather than physically
  # reorganizing the source tree. Costs a copy on every cold boot (mitigated
  # by the warm-daemon design, which boots once and stays resident); avoids
  # a whole second "where does every file live now" project. Revisit if
  # hecksagain gains genuine multi-directory composition, or if the
  # corpus's own layout converges toward hecksagain's flat convention for
  # other reasons.
  #
  # Two genuine filename collisions found in hecks_playground_conception's real corpus
  # (two different domains independently named "Inbox"/"Bluebook",
  # confirmed as distinct aggregates, not duplicates) -- disambiguated by
  # prefixing the relative directory path, not silently dropped.
  # Scoped to canon only -- aggregates/ (the domain tree), storehouse/ (the
  # framework kernel), adapters/ (persistence/heki/ollama/r2 wiring).
  # Deliberately excludes catalog/ (deferred: still load-bearing for the
  # OLD Rust test suite, see migration plan Part 4 "catalog/" note) and
  # anything else at hecks_playground_conception's root that isn't canon.
  CANON_SUBDIRS = %w[aggregates storehouse adapters].freeze

  # Vendored addition, not (yet) upstream hecksagain (migration plan task
  # 4/5): miette's own root has a DIFFERENT top-level shape than
  # hecks_playground_conception's (body/library/self/mind/discipline/framework/
  # world/surface/storehouse/catalog/..., not aggregates/storehouse/
  # adapters) -- a second, PARALLEL canon-subdir list rather than
  # generalizing CANON_SUBDIRS to "any directory with bluebook content
  # recursively", which would silently swallow hecks_playground_conception's own
  # deliberately-excluded catalog/ too.
  # Scoped to directories the corpus's own inventory confirms carry real
  # bluebook content -- deploy/tooling/tools/inbox/docs/information/
  # spikes/tests/data confirmed empty of it (inbox/ especially: 728+
  # files of Miette's own working memory, never bluebook syntax -- an
  # unscoped glob risking a parse attempt on prose is exactly what this
  # list avoids).
  #
  # 2026-08-14 (hecks_playground->hecksagain cutover PRD, slice 3.1): framework/,
  # world/, and surface/ were already miette top-level dirs BEFORE this
  # slice but were left off this list under the (now stale) claim they
  # were "confirmed empty" of bluebook content -- framework/adapters/ and
  # surface/bluebook/ already carried real content at the time. This
  # slice's hecks_playground_conception import made the gap much larger by landing
  # eight more real-content top-level dirs (storehouse/catalog/demo/plan/
  # correspondence/language/test_automation/drafting) that were silently
  # invisible to any bare-root dispatch/query/catalog/list call --
  # confirmed by comparing hecksagain-cli list against hecks_playground_conception's
  # root (returns ~200 aggregates, everything below included) versus
  # against miette's new root pre-fix (ArgumentError on the five-dir
  # flatten, and even had it succeeded, none of the eight new dirs'
  # aggregates would have been in the result at all).
  MIETTE_SUBDIRS = %w[
    body library self mind discipline framework world surface
    storehouse catalog demo plan correspondence language test_automation drafting
  ].freeze

  def self.stage_flat_corpus(root)
        # SCOPED CALLERS PASS THROUGH UNCHANGED: a caller pointing at one
        # domain's own directory (hecksagain's native single-directory
        # convention -- a bluebook/ subdir or bare .bluebook files
        # directly present) doesn't need the whole-corpus flatten, and
        # forcing it through would couple every scoped dispatch to every
        # OTHER file in the corpus being boot-clean too. Only a true
        # multi-domain root (hecks_playground_conception: aggregates/storehouse/
        # adapters ; miette: body/library/self/mind/discipline) gets
        # flattened.
        # `.all?`, not `.any?` -- vendored fix, not (yet) upstream
        # hecksagain (migration plan task 8): `.any?` false-matched
        # bin-buddy (a scattered-shape project, below) purely because it
        # happens to have its OWN top-level `aggregates/` directory --
        # "aggregates" alone is a common, generic name, not a reliable
        # hecks_playground_conception signature ; the COMBINATION of aggregates +
        # storehouse + adapters all present is. Once matched, the wrong
        # glob (requiring a "bluebook/" segment bin-buddy's own layout
        # never has) staged ZERO files -- not an error, a silent EMPTY
        # flatten that made `validate`/`list_aggregates` trivially,
        # falsely agree with each other (0 aggregates, 0 wheres to fail
        # on) while never having loaded any real content at all.
        # Vendored addition, not (yet) upstream hecksagain (migration plan
        # task 8): a project whose top-level bluebook file composes the
        # rest via plain Ruby `require_relative` (opt-website/vindiction:
        # `require_relative "aggregates/owner/owner"`) must NEVER be
        # flattened -- the recursive-copy-with-prefixed-names staging
        # approach destroys the exact relative paths require_relative
        # depends on, so a project using its OWN native Ruby composition
        # is passed through UNCHANGED (same "scoped callers pass through"
        # rule as a single-domain directory) and left to Hecks.boot's own
        # non-recursive `Dir[root/*.bluebook]` + Ruby's require system --
        # which only works once a matching `.rb` shim sits beside each
        # `require_relative`d `.bluebook` file (each project's own repo,
        # not this runtime -- Kernel.load(path_to_the_real_bluebook)).
        return root if uses_require_relative_composition?(root)

        # Vendored addition, not (yet) upstream hecksagain (migration
        # plan task 8/burning-man-prep investigation, 2026-08-11): the
        # "SCOPED CALLERS PASS THROUGH UNCHANGED" comment two paragraphs
        # up already CLAIMED this case ("a bluebook/ subdir... doesn't
        # need the whole-corpus flatten") but no code ever actually
        # implemented it -- a project whose own top-level layout is
        # ALREADY hecksagain's native single-directory convention
        # (`<root>/bluebook/*.bluebook`, nothing deeper) fell through
        # to `scattered_project?` below instead, which is true for
        # EVERY such project (a non-recursive glob of `root` itself
        # always finds 0 direct matches when the real files sit one
        # level down in `bluebook/`, so `recursive.size > direct.size`
        # trivially) -- so it got staged into a throwaway `Dir.tmpdir`
        # copy anyway, needlessly.
        #
        # Harmless for validate/dispatch RESULTS (content is byte-
        # identical either way) but NOT harmless for PERSISTENCE: once
        # staged, `Hecks.boot` computes its own `root` as `File.dirname`
        # of the booted bluebook directory (runtime/loader.rb) -- which
        # for a staged corpus is the STAGE directory, not the real
        # project root -- so a `.world`'s `persisted_by("Heki") { dir
        # "data" }` (a path relative to that root) resolves against
        # `/private/var/folders/.../hecksagain_runtime_stage_<hash>/
        # data/`, not `<root>/data/`. Confirmed live: burning-man-prep's
        # own `data/*.heki` files never changed across an entire real
        # dispatch session (Camper.Admit, Item.Add, the
        # PlaceNewItemOnOwnersPersonalList saga, ConsoleSettings writes)
        # despite every dispatch reporting success and reading its own
        # prior writes back correctly -- because every one of those
        # calls was reading and writing the SAME deterministic (digest-
        # of-root-path) tmp copy, never the project's real `data/`
        # directory. Every prior "verified via real dispatch" claim in
        # this migration for a project shaped this way (pizzas has no
        # local Heki store so this was invisible there ; embryonaut-
        # foundersapp's own eleventh-pass `era_check.rb` fix was patching
        # a downstream SYMPTOM of this exact same over-staging, not a
        # separate bug) never actually exercised real, durable,
        # cross-process persistence for this shape of project.
        #
        # `File.join(root, "bluebook")` (not `root` alone) mirrors
        # exactly what `Adapters::Folder#bluebook_directory` already
        # does internally when handed `root` directly -- this just
        # skips the round-trip through a needless staged copy to reach
        # the same directory `Hecks.boot(root)` would already resolve
        # to on its own. Deliberately narrow: only fires when `root`
        # itself carries NO `.bluebook` file directly (so it can't
        # collide with a project that legitimately has both a top-level
        # `.bluebook` file AND an unrelated `bluebook/` folder) and the
        # `bluebook/` subdirectory's own files are flat (no nested
        # `**` needed) -- exactly hecksagain's own documented single-
        # domain convention, nothing broader.
        nested_bluebook_dir = File.join(root, "bluebook")
        if Dir.glob(File.join(root, "*.bluebook")).empty? &&
           Dir.glob(File.join(nested_bluebook_dir, "*.bluebook")).any?
          return root
        end

        canon_globs =
          if CANON_SUBDIRS.all? { |d| File.directory?(File.join(root, d)) }
            # Slice 1.3 (driving-adapter-grammar port) fix, real and
            # confirmed live: hecks_playground_conception's own documented convention
            # for `driving on`/`driven on` adapters is a SIBLING
            # `hecksagons/` folder next to `bluebook/`
            # (aggregates/framework/tools/hecksagons/{shell_adapter,
            # sprint14_smoke_fanout,cron_adapter,interval_adapter,
            # agent_tool}.hecksagon -- tools.behaviors's own comment names
            # it "the conventional location") -- NOT a file nested inside
            # `bluebook/` itself. The glob below only ever matched the
            # latter, so every file in `hecksagons/` was silently absent
            # from EVERY full-corpus boot (validate, catalog, a
            # :cross_cascade behaviors test) -- confirmed by staging the
            # real corpus and finding zero matches for any of the five
            # filenames above. Added as its own glob per canon subdir
            # rather than folded into the existing pattern, so a directory
            # that legitimately has both stays covered by both.
            CANON_SUBDIRS.flat_map do |d|
              [File.join(root, d, "**", "bluebook", "*.{bluebook,hecksagon,world,fixtures,behaviors}"),
               File.join(root, d, "**", "hecksagons", "*.hecksagon")]
            end
          elsif MIETTE_SUBDIRS.all? { |d| File.directory?(File.join(root, d)) }
            MIETTE_SUBDIRS.map { |d| File.join(root, d, "**", "bluebook", "*.{bluebook,hecksagon,world,fixtures,behaviors}") }
          elsif scattered_project?(root)
            # Vendored addition, not (yet) upstream hecksagain (migration
            # plan task 8): a THIRD project shape, distinct from both
            # canon-subdir lists above -- bin-buddy/deciderate/mietteai/
            # opt-website/vindiction/pigeoncoop/daily-musing and likely
            # most of the other-20-projects wave declare their aggregates
            # DIRECTLY under a named directory (`aggregates/service_task/
            # service_task.bluebook`), never inside a "bluebook/"
            # subdirectory segment at all -- neither CANON_SUBDIRS'
            # pattern nor MIETTE_SUBDIRS' matches. Without this,
            # `Hecks.boot(root)` falls back to `bluebook_directory`
            # returning `root` itself and non-recursively globs only the
            # ROOT'S OWN top-level file (bin-buddy.bluebook -- a
            # documentation/index file that says "the actual aggregates
            # live in per-aggregate splits under aggregates/<name>/
            # <name>.bluebook", never loading them) -- a FALSE POSITIVE
            # "valid:true" that validated a near-empty index file, not
            # the real content. No "bluebook/" segment requirement, since
            # this project shape has none ; excludes vendor/node_modules/
            # .git/spec/test the same way a real project's own tooling
            # would.
            [File.join(root, "**", "*.{bluebook,hecksagon,world,fixtures,behaviors}")]
          end
        return root unless canon_globs

        digest = Digest::SHA256.hexdigest(File.expand_path(root))[0, 12]
        stage_root = File.join(Dir.tmpdir, "hecksagain_runtime_stage_#{digest}")
        source_files = canon_globs.flat_map { |g| Dir.glob(g) }
                                   .reject { |f| f =~ %r{/(vendor|node_modules|\.git|spec|test)/} }
        source_mtimes = source_files.map { |f| File.mtime(f) }
        newest_source = source_mtimes.max
        stage_dir = File.join(stage_root, "bluebook")
        marker = File.join(stage_root, ".staged_at")

        # Vendored fix, not (yet) upstream hecksagain (migration plan
        # task 8): the staleness check used to compare ONLY the newest
        # source mtime against the marker -- correct for edits and
        # additions, but blind to DELETIONS. Deleting a source file
        # (hecks_playground_nursury's blog/hecks_playground/ duplicate, this migration's own
        # dedup fix) never makes any REMAINING file newer than an
        # already-fresh marker, so the stale cache -- still holding the
        # deleted file's copy -- kept being reused, and `validate` kept
        # reporting the exact "Blog:Post already exists" error the fix
        # had already resolved on disk, until the tmp dir was deleted by
        # hand. `source_files.size` catches deletions (and, as a
        # side effect, any other count-changing edit a pure max-mtime
        # comparison can't see) without hashing every file's full
        # content on every call. TODO upstream via bin/evolve (migration
        # plan task 7).
        file_count_marker = File.join(stage_root, ".staged_count")
        current_count      = source_files.size.to_s
        count_changed      = !File.exist?(file_count_marker) || File.read(file_count_marker) != current_count

        # Found live (2026-08-13): count_changed alone is blind to a
        # DIFFERENT bug than the deletion case above -- which BRANCH
        # produced canon_globs (CANON_SUBDIRS / MIETTE_SUBDIRS /
        # scattered_project?) can itself flip between calls if a
        # directory this root depends on (hecks_playground_conception/adapters,
        # /storehouse) transiently vanishes from another process racing
        # this one, then reappears. If the transient scattered-glob pass
        # happens to stage the SAME file count as a later, correctly-
        # shaped canon-glob pass, count_changed is false and the WRONG
        # (broader, unscoped) file set silently survives for every
        # subsequent call -- confirmed live : a scattered-glob stage
        # pulled in framework/behavior_kinds/*.hecksagon (files with no
        # "bluebook/" path segment, using a Hecks.behavior_kind DSL
        # entrypoint vendor/hecksagain never defines), and every dispatch
        # against the cached stage raised NoMethodError until the tmp dir
        # was deleted by hand. Fingerprinting the glob PATTERNS
        # themselves (not just what they matched) catches a branch flip
        # even when file counts coincide.
        shape_marker     = File.join(stage_root, ".staged_shape")
        current_shape     = Digest::SHA256.hexdigest(canon_globs.sort.join("\n"))
        shape_changed     = !File.exist?(shape_marker) || File.read(shape_marker) != current_shape

        if !File.exist?(marker) || count_changed || shape_changed || (newest_source && File.mtime(marker) < newest_source)
          FileUtils.rm_rf(stage_root)
          FileUtils.mkdir_p(stage_dir)
          seen = Hash.new(0)
          # BUG FIXED during migration: this loop used to glob the whole
          # root recursively (catching catalog/'s deferred old-syntax
          # files too) instead of source_files (already canon-scoped
          # above) -- found live via appeal.bluebook persisting in every
          # "fresh" stage despite the staleness check itself being
          # correctly scoped.
          #
          # DEPENDENCY-ORDERED, not plain alphabetical: hecksagain's
          # Adapters::Folder#load_domain does `Dir[directory/*.bluebook]`,
          # and EACH `Hecks.bluebook "X" do...end` block validates itself
          # IMMEDIATELY on load (MetaValidator.call runs per file, not
          # after the whole multi-file domain assembles) -- so a file
          # referencing an aggregate declared in a LATER-sorted sibling
          # file fails structurally, even though the two files are
          # genuinely the same domain and would compose fine if ordered
          # right. Found live: conductor/claim.bluebook (references
          # Worker) sorts before conductor/worker.bluebook alphabetically.
          # Fix: within each domain-name's own file group (not globally --
          # different domains don't need cross-ordering), topologically
          # sort by which aggregate names each file's own
          # `reference_to`/`belongs_to`/`has_many`/`has_one` calls point
          # at vs. which aggregate names the file itself declares.
          # ADDITIONAL FIX: copy order alone doesn't work -- hecksagain's
          # Adapters::Folder#load_each does `Dir[pattern].sort` before
          # loading, re-sorting ALPHABETICALLY regardless of copy order.
          # The desired order has to be encoded IN the filename itself
          # (a zero-padded numeric prefix), or the dependency sort above
          # is silently discarded.
          # BUG FIXED during migration (task 4): the cleanup below used to
          # remove `File.join(stage_dir, base)` (the bare, un-prefixed
          # basename) -- but the first pass NEVER wrote that filename; it
          # always writes `"%03d_%s" % [i, base]` (the numeric-index
          # prefix from stage_flat_corpus's own dependency-order encoding,
          # unconditionally, even for a base seen only once so far). So
          # the `rm_f` silently matched nothing, the stale first-pass copy
          # survived on disk, and a second-pass disambiguated copy landed
          # ALONGSIDE it -- two staged copies of the same source file,
          # both `Hecks.load`ed, each independently declaring the same
          # aggregate ("Declare creates an Aggregate that already
          # exists"). Found live: two files both named bluebook.bluebook
          # (aggregates/framework/bluebook/ and aggregates/language/
          # grammar/bluebook/), the corpus's first REAL basename collision
          # to reach a full-tree boot. Fixed by tracking the exact
          # first-pass filename per base, so cleanup removes what was
          # actually written, not a name that was never on disk.
          # `first_pass_names[base]` collects EVERY filename actually
          # written for that base during the first pass, not just the
          # first occurrence -- the earlier fix's `||=` only remembered
          # ONE name per base, but when seen[base] first crosses 2, THAT
          # SAME occurrence's own first-pass write already used
          # `disambiguated_name` (still numeric-prefixed) -- a SECOND
          # first-pass file the old single-value tracking never recorded,
          # so it survived the second pass's cleanup too (found live:
          # 013_aggregates_language_grammar__bluebook.bluebook sat next to
          # the correctly-cleaned 012_bluebook.bluebook, still a
          # duplicate). Removing the FULL list this base ever produced in
          # pass one is the only way pass two starts from a clean slate.
          first_pass_names = Hash.new { |h, k| h[k] = [] }
          ordered_files(source_files).each_with_index do |f, i|
            base = File.basename(f)
            seen[base] += 1
            name = seen[base] > 1 ? disambiguated_name(f, root) : base
            prefixed = format("%03d_%s", i, name)
            first_pass_names[base] << prefixed
            FileUtils.cp(f, File.join(stage_dir, prefixed))
          end
          # Second pass: files whose base collided need their FIRST
          # occurrence renamed too (seen[base] was 1 on the first pass, so
          # it wasn't disambiguated) -- redo any base with seen[base] > 1
          # from scratch, disambiguating every occurrence. Scoped to
          # source_files too, not a fresh unscoped glob.
          seen.select { |_, count| count > 1 }.each_key do |base|
            first_pass_names[base].each { |name| FileUtils.rm_f(File.join(stage_dir, name)) }
            source_files.select { |f| File.basename(f) == base }.sort.each do |f|
              FileUtils.cp(f, File.join(stage_dir, disambiguated_name(f, root)))
            end
          end
          FileUtils.touch(marker)
          File.write(file_count_marker, current_count)
          File.write(shape_marker, current_shape)
        end
        stage_root
  end

  # Vendored addition, not (yet) upstream hecksagain (migration plan task
  # 8) -- see the "scattered_project?" branch comment in stage_flat_corpus
  # for why this exists. Signal: the root has REAL bluebook content sitting
  # deeper than a non-recursive `Hecks.boot(root)` would ever find (more
  # matches recursively than directly in root), and neither of the other
  # two known project shapes applies. Excludes vendor/node_modules/.git/
  # spec/test the same way source_files itself does, so an empty match
  # there (a project with ONLY a vendored hecksagain copy recursively
  # findable, no real content of its own) doesn't falsely trigger a flatten.
  def self.scattered_project?(root)
    direct = Dir.glob(File.join(root, "*.bluebook"))
    recursive = Dir.glob(File.join(root, "**", "*.bluebook"))
                    .reject { |f| f =~ %r{/(vendor|node_modules|\.git|spec|test)/} }
    recursive.size > direct.size
  end

  # Vendored addition, not (yet) upstream hecksagain (migration plan task
  # 8) -- see the return-root-unchanged branch's own comment in
  # stage_flat_corpus. Checked only against root's OWN top-level
  # .bluebook files (the ones a non-recursive `Hecks.boot(root)` would
  # actually load and execute) -- a require_relative buried inside a
  # DEEPER file wouldn't change how root itself should be staged.
  def self.uses_require_relative_composition?(root)
    Dir.glob(File.join(root, "*.bluebook")).any? { |f| File.read(f).include?("require_relative") }
  end

  # Groups source_files by their own declared `Hecks.bluebook "X"` domain
  # name, topologically sorts each group by aggregate reference (a file
  # declaring an aggregate another file's `reference_to`/`belongs_to`/
  # `has_many`/`has_one` points at must load first), concatenates groups
  # back together (inter-domain order left alphabetical -- this fix
  # targets the found failure mode specifically: same-domain, cross-file).
  # Falls back to alphabetical within a group on any cycle (a real cycle
  # is a content problem no ordering fixes -- surfaces as a boot error to
  # investigate, not silently masked by a wrong-but-quiet order).
  def self.ordered_files(files)
    by_domain = files.group_by { |f| domain_name_of(f) }
    by_domain.sort.flat_map { |_, group| topo_sort(group) }
  end

  def self.domain_name_of(file)
    text = File.read(file)
    (text[/^Hecks\.bluebook\s+"([^"]+)"/, 1] || File.basename(file))
  end

  def self.topo_sort(group)
    return group if group.size <= 1

    declares = {} # file => Set of aggregate names it declares
    needs    = {} # file => Set of aggregate names it references
    group.each do |f|
      text = File.read(f)
      declares[f] = text.scan(/^\s*aggregate\s+"(\w+)"/).flatten.to_set
      needs[f]    = text.scan(/^\s*(?:reference_to|belongs_to|has_many|has_one)\s+(\w+)/).flatten.to_set
    end

    remaining = group.dup
    ordered   = []
    loop do
      break if remaining.empty?

      ready = remaining.select do |f|
        needed = needs[f] - declares[f] # a file may self-reference; ignore
        other_declared = remaining.reject { |g| g == f }.flat_map { |g| declares[g].to_a }.to_set
        (needed & other_declared).empty?
      end
      # Cycle (or unresolvable) fallback: take the rest in original order
      # rather than looping forever or silently dropping files.
      ready = [remaining.first] if ready.empty?

      ordered.concat(ready.sort)
      remaining -= ready
    end
    ordered
  end

  def self.disambiguated_name(file, root)
    rel = File.dirname(File.dirname(file)).sub("#{File.expand_path(root)}/", "").sub("#{root}/", "")
    "#{rel.gsub('/', '_')}__#{File.basename(file)}"
  end
  # One call, one result. Boots the domain, dispatches, returns a clean
  # {ok, state, events} hash -- no cascade/timeline text to scrape (see
  # dispatch_render.mjs's old design, retired by this rewrite).
  def self.dispatch(root, verb, args = {})
    runtime = Hecks.boot(stage_flat_corpus(root), install_facade: false)
    args    = AttrDecode.decode_args(runtime, verb, args, kind: :command)
    result  = runtime.dispatch(verb, **symbolize(args))
    {
      ok: true,
      state: result.state,
      events: result.events.map { |e| { name: e.name, aggregate: e.aggregate, id: e.id, payload: e.payload } },
      # THE EXECUTION PORT'S REPLY — present only for a command whose
      # aggregate binds an `executed_by` adapter (Tools::ShellTool /
      # FileTool / SearchTool). Carried explicitly rather than left to be
      # read off the events: a re-entered Cascade.RecordResult's events do
      # not land in this call's own `announced`, so without this a caller
      # would watch a shell command genuinely run and still see nothing
      # come back.
      #
      # Added conditionally rather than via `.compact` — compact would ALSO
      # drop `state` on the port-operation path (nothing is hydrated there,
      # so state is legitimately nil), silently changing the response shape
      # of every dispatch to fix the one key that needed it.
    }.tap { |payload| payload[:reply] = result.reply if result.reply }
  rescue StandardError => e
    { ok: false, error: e.message, error_class: e.class.name }
  end

  def self.query(root, verb, args = {})
    runtime = Hecks.boot(stage_flat_corpus(root), install_facade: false)
    args    = AttrDecode.decode_args(runtime, verb, args, kind: :query)
    rows    = runtime.query(verb, **symbolize(args))
    { ok: true, rows: rows }
  rescue StandardError => e
    { ok: false, error: e.message, error_class: e.class.name }
  end

  # hecksagain has no single-record-by-id tool (bin/stores dumps every
  # aggregate's current head records). This is the few-lines version Part 5
  # recommended over writing a new subsystem.
  def self.state(root, aggregate_fqn, id)
    runtime = Hecks.boot(stage_flat_corpus(root), install_facade: false)
    domain_name, aggregate_name = aggregate_fqn.split("::")
    bluebook = runtime.registry.bluebooks.fetch(domain_name)
    ir       = bluebook.aggregates.find { |a| a.name == aggregate_name } or
      return { ok: false, error: "no such aggregate #{aggregate_fqn}" }
    record = runtime.registry.repository(domain_name, ir).find(id)
    { ok: !record.nil?, aggregate: aggregate_fqn, id: id, state: record&.state }
  rescue StandardError => e
    { ok: false, error: e.message, error_class: e.class.name }
  end

  # Replaces `storehouse validate`. hecksagain's own DSL builders raise
  # inline and Registry#verify! checks wiring at boot -- a clean Hecks.boot
  # already IS validity (Part 1's validator finding). No separate pass.
  #
  # PER-ROOT for a multi-domain corpus (i745 -- parser-removal plan, Phase
  # 1b): a single combined boot over the WHOLE corpus reported valid:true
  # while 36 of hecks_playground_conception's own 58 sub-roots were individually
  # invalid -- a crash anywhere makes the whole thing false with no "which
  # root", and a name collision between two roots' declarations can make a
  # broken root silently PASS (confirmed live this session: heki.adapter
  # was missing field :dir and only "worked" because a DIFFERENT bug
  # crashed it before it could load and shadow a better copy). See
  # ValidateSweep's own header for the full reasoning.
  #
  # ALWAYS sweeps, single domain or many -- a single-domain root reduces to
  # "1 root swept, valid or not", which is the same answer in a strictly
  # more informative shape ({roots_swept, valid, invalid, results}, not a
  # bare boolean). Kept ONE path deliberately: a root-count comparison to
  # special-case the single-domain shape back to the old bare-boolean form
  # turned out to compare an expanded path against a relative one and never
  # actually match (caught by cli_smoke.mjs's own assertion needing an
  # update, not by this comment) -- simpler to commit to one honest shape
  # everywhere than carry a comparison that silently never took its branch.
  def self.validate(root)
    ValidateSweep.run(root)
  end

  # Replaces storehouse__catalog / describe_aggregate / list_aggregates.
  # hecksagain's own Bluebook#to_h shape (ir_version, classification,
  # read_models, canonical_form) is taken as-is -- a documented breaking
  # shape change from dump.rs's shape, not a preserved-compatibility
  # adapter (Part 5's explicit call).
  def self.catalog(root)
    runtime = Hecks.boot(stage_flat_corpus(root), install_facade: false)
    { ok: true, bluebooks: runtime.registry.bluebooks.transform_values(&:to_h) }
  rescue StandardError => e
    { ok: false, error: e.message, error_class: e.class.name }
  end

  # Vendored addition, not (yet) upstream hecksagain (storehouse-mcp
  # rewrite, migration plan Part 5 "correction" pass): a miss (unknown
  # domain OR unknown aggregate within a known domain) now carries the
  # full list of available Domain::Aggregate FQNs across the whole root,
  # same data catalog/list_aggregates already compute, so a caller with a
  # misspelled name gets the correction inline instead of a bare "no such
  # aggregate" -- matches the old Rust-CLI-era describe_aggregate.mjs's own
  # documented miss behaviour, restored under the new corpus-root contract.
  def self.describe_aggregate(root, aggregate_fqn)
    domain_name, aggregate_name = aggregate_fqn.split("::")
    runtime = Hecks.boot(stage_flat_corpus(root), install_facade: false)
    available = runtime.registry.bluebooks.values.flat_map do |b|
      b.aggregates.map { |a| "#{b.name}::#{a.name}" }
    end

    bluebook = runtime.registry.bluebooks[domain_name]
    agg      = bluebook&.aggregates&.find { |a| a.name == aggregate_name }
    return { ok: false, error: "no such aggregate #{aggregate_fqn}", available: available } unless agg

    { ok: true, aggregate: agg.to_h }
  rescue StandardError => e
    { ok: false, error: e.message, error_class: e.class.name }
  end

  def self.list_aggregates(root)
    runtime = Hecks.boot(stage_flat_corpus(root), install_facade: false)
    { ok: true, aggregates: runtime.registry.bluebooks.values.flat_map { |b| b.aggregates.map(&:name) } }
  rescue StandardError => e
    { ok: false, error: e.message, error_class: e.class.name }
  end

  # i746 step 7 -- replaces bin/adapter-host's old shell-out to a
  # nonexistent "storehouse dump-hecksagon" subcommand. Boots root, finds
  # the named adapter, returns its declared `handler` path (or nil if the
  # .adapter file never declared one -- bin/adapter-host's own "fail loud"
  # check handles that, not this method).
  def self.adapter_handler(root, adapter_name)
    runtime = Hecks.boot(stage_flat_corpus(root), install_facade: false)
    adapter = runtime.registry.adapters[adapter_name]
    return { ok: false, error: "no such adapter #{adapter_name.inspect}", available: runtime.registry.adapters.keys } unless adapter

    { ok: true, adapter: adapter_name, handler: adapter.handler }
  rescue StandardError => e
    { ok: false, error: e.message, error_class: e.class.name }
  end

  def self.symbolize(h)
    (h || {}).transform_keys { |k| k.to_s.to_sym }
  end
end
