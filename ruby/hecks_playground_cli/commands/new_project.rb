HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Cli::CliInternals,
  base_dir: File.expand_path("..", __dir__)
)

HecksPlayground::CLI.handle(:new_project) do |inv|
  name = inv.args.first
  pascal = HecksPlayground::Utils.sanitize_constant(name)
  dir = name

  if File.exist?(dir)
    say "Directory #{dir} already exists", :red
    next
  end

  world_result = if options[:"no-world-goals"]
    { concerns: [], extensions: [], stub: false }
  else
    HecksPlayground::WorldConcernsPrompt.new(say_method: method(:say)).run
  end

  app_template = lambda do
    <<~RUBY
      require "hecks_playground"

      app = HecksPlayground.boot(__dir__)
    RUBY
  end

  hecksagon_template = lambda do
    <<~HECKSAGON
      HecksPlayground.hecksagon "#{pascal}" do
        capabilities :crud
      end
    HECKSAGON
  end

  gemfile_template = lambda do
    hecks_playground_spec = ::Gem.loaded_specs["hecks_playground"]
    if hecks_playground_spec && hecks_playground_spec.full_gem_path != File.expand_path("../../../..", __FILE__)
      gem_line = 'gem "hecks_playground"'
    else
      hecks_playground_root = File.expand_path("../../../..", __FILE__)
      gem_line = "gem \"hecks_playground\", path: \"#{hecks_playground_root}\""
    end

    <<~RUBY
      source "https://rubygems.org"
      #{gem_line}
    RUBY
  end

  spec_helper_template = lambda do
    <<~RUBY
      require "hecks_playground"
      app = HecksPlayground.boot(File.join(__dir__, ".."))
      app.capability(:crud)

      RSpec.configure do |config|
        config.order = :random
      end
    RUBY
  end

  gitignore_template = lambda do
    <<~TEXT
      *.gem
      *_domain/
    TEXT
  end

  rspec_template = lambda do
    <<~TEXT
      --format documentation
      --color
      --require spec_helper
    TEXT
  end

  FileUtils.mkdir_p(File.join(dir, "spec"))

  write_or_diff(
    File.join(dir, "#{name}.bluebook"),
    domain_template(pascal,
      world_concerns: world_result[:concerns],
      extensions:     world_result[:extensions],
      stub:           world_result[:stub])
  )
  write_or_diff(File.join(dir, "#{name}.hecksagon"), hecksagon_template.call)
  write_or_diff(File.join(dir, "#{name}.rb"), app_template.call)
  write_or_diff(File.join(dir, "Gemfile"), gemfile_template.call)
  write_or_diff(File.join(dir, "spec", "spec_helper.rb"), spec_helper_template.call)
  write_or_diff(File.join(dir, ".gitignore"), gitignore_template.call)
  write_or_diff(File.join(dir, ".rspec"), rspec_template.call)

  if world_result[:concerns].any? || world_result[:stub]
    say ""
    say "Domain created. World concerns declared.", :green
  else
    say "Created #{dir}/", :green
  end
  say "  #{name}.bluebook"
  say "  #{name}.hecksagon"
  say "  #{name}.rb"
  say "  Gemfile"
  say "  spec/spec_helper.rb"
  say "  .gitignore"
  say "  .rspec"
  say ""
  say "Get started:"
  say "  cd #{dir}"
  say "  bundle install"
  say "  ruby #{name}.rb"
end
