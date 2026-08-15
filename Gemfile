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
gem "hecksagain", git: "https://github.com/chrisyoung/hecks-hecksagain", branch: "main", ref: "a984054"

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
