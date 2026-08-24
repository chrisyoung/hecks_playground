source "https://rubygems.org"

gemspec

gem "websocket"

# VENDOR-SOURCED, not `git:` -- and this is now the REAL source, not a
# fallback. hecksagain lives in a PRIVATE fork
# (github.com/chrisyoung/hecks-hecksagain), and `ci.yml` (pure Ruby specs,
# no Rust) has no credentials to clone it: every CI run failed outright at
# `bundle install`, before a single spec ran. A `path:` source needs no
# token, no network, and no private-repo access at all -- the content is
# already in this repo, tracked, reviewed in the same PR as the code that
# depends on it.
#
# `vendor/hecksagain` is the SAME content the `git:` pin resolved to --
# re-vendored from the fork's `1cce2d1` (see vendor/hecksagain/
# VENDORED_FROM.md for the exact commit, the 3-way-merge discipline, and
# what changed since the last sync). Re-vendor, don't hand-edit: bump by
# copying the fork's `lib/` + `hecksagain.gemspec` again and updating
# VENDORED_FROM.md, exactly as the note describes. The old `ref:` bumping
# ritual the history below records is what this replaces.
#
# The wave the earlier comment said `vendor/hecksagain` was waiting on
# ("stays in place as a fallback until the rest of the wave verifies")
# HAS now verified: the 64-root `HecksagainRuntime.validate` sweep over
# hecks_playground_conception reproduces the git-pin baseline exactly (59 valid /
# 4 invalid / 63 swept), and the for_each fan-out + verb-prefix fixes
# (i787, f5557d80b) prove out live against the vendored gem.
#
# ---- history of the `git:`-pinned era this replaces ----
# hecksagain-cutover PRD, slice 2.1 replaced the hand-maintained
# hecks_playground/vendor/hecksagain file-copy vendoring with a `ref:`-pinned git
# source (no real Bundler entry existed before that --
# hecksagain_runtime/bin/hecksagain-cli hardcoded
# `$LOAD_PATH.unshift ".../vendor/hecksagain/lib"` instead).
#
# Bumped 2026-08-16: `48d2210` was the fork's OWN pre-rebase tip -- every
# dispatch through this Gemfile had been running that frozen point since
# the day it was pinned, missing an entire squash-and-reapply rebase onto
# upstream/hecksagain's current main (ADR 0025/0026 grammar), a same-day
# upstream catch-up, new fork-only features (execution port + Shell/
# Filesystem/Search/Email adapters wired live, driven-handler dispatch,
# the .behaviors authoring DSL), and three bug fixes since merged into
# real upstream hecksagain (#327 Rendering.describe object-pointer leak,
# #328 World#for_binding cross-adapter settings leak, #329 increment/
# decrement/multiply on an absent VO-typed attribute). `933d1dd` was the
# fork's tip when this pin first bumped; `fe39d44` adds one more fork-only
# fix found immediately after: `AdapterBuilder#handler` was never wired
# (the Adapter struct carried the field, the Stripe example used the DSL
# word, but no builder method ever set it -- every `.adapter` file in this
# corpus using `handler "..."` raised NoMethodError at parse time). Bumping
# ALSO required migrating this corpus off two DSL forms real upstream
# retired before `48d2210`'s freeze: the two-arg `aggregate "X", "desc" do`
# form (-> `aggregate "X" do` + `description "desc"` inside the block, 225
# call sites/133 files) and command-level `description "..."` (-> `goal
# "..."` per ADR 0025, 830 call sites/135 files) -- both migrated via
# precise one-off Ruby scripts (see project history), not hand edits.
# Bumped 2026-08-17 (i780/i781): `74ea7da` adds record_effect_outbound, the
# runtime producer for the spawn/charged_by effect port's `Bind#on/success/
# failure` DSL-capture, which had been real but never consumed. First real
# caller is this corpus's :exec -> spawned_by re-expression (fibroblast,
# git, cargo, filesystem, tools, inbox, process_health, session, plan,
# macrophage). Reviewed and merged straight to the fork's main (Chris,
# 2026-08-17) -- feat/spawn-effect-port is deleted, this is the real pin
# now, not provisional.
# ---- end history ----
gem "hecks", path: "vendor/hecks"

group :development, :test do
  gem "rake"
  gem "rspec", "~> 3.0"
  gem "webrick"
  gem "railties", "~> 8.0"
  gem "activemodel", ">= 6.0", "< 10.0"
  gem "sqlite3", ">= 1.4", "< 3.0"
  gem "rdoc", ">= 6.4", "< 6.7"
  gem "sdoc"
  gem "mongo"
  gem "reek"
  gem "flay"
  gem "flog"
  gem "debride"
  gem "fasterer"
  gem "bundler-audit"
end
