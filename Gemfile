source "https://rubygems.org"

gemspec

gem "websocket"

# hecksagain-cutover PRD, slice 2.1: replaces the hand-maintained
# hecks/vendor/hecksagain file-copy vendoring (no real Bundler entry
# existed before this -- hecksagain_runtime/bin/hecksagain-cli hardcoded
# `$LOAD_PATH.unshift ".../vendor/hecksagain/lib"` instead). Pinned via
# `ref:` (not a bare `branch:`) so this is load-bearing and reproducible,
# not floating -- the whole point of this decision per the PRD. Bump the
# ref explicitly (new commit + `bundle install`) to pick up fork changes;
# `vendor/hecksagain` stays in place as a fallback until the rest of the
# wave verifies (do not delete yet).
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
gem "hecksagain", git: "https://github.com/chrisyoung/hecks-hecksagain", branch: "main", ref: "fe39d44"

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
