# Read the version string directly rather than `require_relative`-ing
# ruby/hecks_playground/version.rb — that file opens `module HecksPlayground`, and defining
# that constant at Gemfile-eval time collides with hecksagain's own
# "HecksPlayground" domain (Bundler evals every gemspec on `bundle install`/
# `bundler/setup`, so this ran before `require "hecksagain"` ever got a
# chance, silently breaking "no domain \"HecksPlayground\" loaded" dispatch for
# any bluebook FQN starting with a bare `HecksPlayground::` prefix). The old
# ruby/hecks_playground_cli files that still reference `HecksPlayground::VERSION` directly
# are unaffected — they define/require the module themselves if ever
# invoked standalone; this file only needed the version STRING.
hecks_playground_version = File.read(File.expand_path("ruby/hecks_playground/version.rb", __dir__))[/VERSION\s*=\s*"([^"]+)"/, 1]

Gem::Specification.new do |spec|
  spec.name          = "hecks_playground"
  spec.version       = hecks_playground_version
  spec.authors       = ["Christopher Young"]
  spec.summary       = "Hexagonal DDD framework for Ruby"
  spec.description   = "Domain compiler: DSL, IR, runtime, generators, CLI, workshop, and AI tools"
  spec.homepage      = "https://github.com/chrisyoung/hecks_playground"
  spec.license       = "MIT"

  spec.files         = Dir["ruby/**/*.{rb,hec,bluebook,html,js,css}"] +
                       Dir["hecks_playground/**/*.{bluebook,hecksagon,world}"] +
                       ["README.md", "FEATURES.md", "hecks_playground_logo.png"]
  spec.require_paths = ["ruby"]
  spec.bindir        = "bin"
  spec.executables   = ["hecks_playground", "hecks_playground_claude", "appeal"]

  spec.required_ruby_version = ">= 3.0"

  # Zero runtime dependencies. HecksPlayground ships kernel-only — domains compile
  # and run on stdlib alone (json, date, ostruct). Persistence, AI,
  # natural-language assists are opt-in : install the driver you want.
  #
  # Optional gems each feature looks for at runtime (begin/rescue LoadError
  # at the call site, friendly install hint when missing) :
  #
  #   sequel     — SQL persistence (HecksPersist + :sqlite / :postgres /
  #                :mysql extensions). Pulls in `sqlite3`, `pg`, or
  #                `mysql2` for the driver layer.
  #   rwordnet   — fuller verb detection in the CommandNaming validator.
  #                Without it, validator falls back to the custom-verb
  #                list configured per project.
  #
  # If you only use heki/R2 storage (the bin-buddy / StoreHouse arc),
  # you need none of these.
end
