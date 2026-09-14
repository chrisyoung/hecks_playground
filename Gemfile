source "https://rubygems.org"

gemspec

gem "websocket"

# hecks, the DSL/runtime this repo is built on, comes from the published
# gem (rubygems.org, source: github.com/heckslabs/hecks). It used to be
# vendored under vendor/hecks from the private chrisyoung/hecks-hecksagain
# fork; that copy and its VENDORED_FROM.md history are gone. The gem ships
# lib/ only -- anything needing hecks's Rust crate or bin/ scripts uses a
# source checkout of heckslabs/hecks instead.
gem "hecks", "~> 1.3"

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
