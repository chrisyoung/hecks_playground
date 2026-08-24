# ActiveHecks::InitGenerator
#
# Rails generator that sets up a HecksPlayground domain gem in a Rails app.
# Detects the *_domain directory, creates a HecksPlayground.configure initializer,
# adds app/models/HECKS_PLAYGROUND_README.md explaining the setup, and injects
# hecks_playground/test_helper into the test/spec helper file.
#
# The domain gem must already be built and in the Gemfile.
#
#   rails generate active_hecks_playground:init
#
require "rails/generators"

module ActiveHecks
  class InitGenerator < ::Rails::Generators::Base
    desc "Set up a HecksPlayground domain gem in your Rails app"

    def detect_domain_gem
      @gem_dir = Dir.glob(::Rails.root.join("*_domain")).first
      unless @gem_dir
        say "No domain gem found (looking for *_domain/ directory)", :red
        say "Build one first with `hecks_playground build` and add it to your Gemfile."
        raise SystemExit
      end

      @gem_name = File.basename(@gem_dir)
      @domain_module = @gem_name.split("_").map(&:capitalize).join.sub(/Domain$/, "") + "Domain"
      say "Found domain gem: #{@gem_name}", :green
    end

    def create_initializer
      initializer_path = ::Rails.root.join("config/initializers/hecks_playground.rb")
      if File.exist?(initializer_path)
        say "config/initializers/hecks_playground.rb already exists", :yellow
        return
      end

      create_file "config/initializers/hecks_playground.rb", <<~RUBY
        HecksPlayground.configure do
          # Optional: control which extension gems are required at boot.
          # gems only: [:audit, :logging]   # require only these
          # gems except: [:pii]             # skip from the default AUTO list
          domain "#{@gem_name}"
          adapter :memory
        end
      RUBY
    end

    def create_hecks_playground_readme
      create_file "app/models/HECKS_PLAYGROUND_README.md", <<~MD
        # Domain Models

        This app uses [HecksPlayground](https://github.com/hecks_playground) for domain modeling.
        There are no ActiveRecord model files here — domain objects come from
        the `#{@gem_name}` gem.

        ## Usage

        ```ruby
        Pizza.create(name: "Margherita", description: "Classic")
        Pizza.find(id)
        Pizza.all
        Pizza.count
        Pizza.delete(id)

        pizza.toppings.create(name: "Mozzarella", amount: 2)
        pizza.toppings.each { |t| puts t.name }
        ```

        ## Changing the Domain

        The domain is defined in a standalone HecksPlayground project. To modify it:

        1. Go to the HecksPlayground project where `domain.rb` lives
        2. Run `hecks_playground console` to edit interactively
        3. Run `hecks_playground build` to generate a new version of the gem
        4. Update the gem version in this app's Gemfile
        5. `bundle update #{@gem_name}`
        6. Generate and run migrations:
           ```
           rails generate active_hecks_playground:migration
           rake hecks_playground:db:migrate
           ```

        ## Where's the Code?

        - Domain classes: `#{@gem_name}/lib/` (the gem)
        - Initializer: `config/initializers/hecks_playground.rb`
        - This file: `app/models/HECKS_PLAYGROUND_README.md`
      MD
    end

    def setup_test_helper
      helper_path = ::Rails.root.join("spec/rails_helper.rb")
      helper_path = ::Rails.root.join("spec/spec_helper.rb") unless File.exist?(helper_path)
      helper_path = ::Rails.root.join("test/test_helper.rb") unless File.exist?(helper_path)

      if File.exist?(helper_path)
        content = File.read(helper_path)
        unless content.include?("hecks_playground/test_helper")
          inject_into_file helper_path, after: /require.*rails.*helper.*\n|require.*spec.*helper.*\n|require.*test.*helper.*\n/i do
            "require \"hecks_playground/test_helper\"\n"
          end
          say "Added hecks_playground/test_helper to #{helper_path.relative_path_from(::Rails.root)}", :green
        end
      end
    end

    def print_summary
      say ""
      say "HecksPlayground initialized!", :green
      say "  config/initializers/hecks_playground.rb   — boots the domain"
      say "  app/models/HECKS_PLAYGROUND_README.md     — explains the setup"
      say ""
      say "Domain objects are ready: Pizza.create, Pizza.find, etc."
      say "Tests reset automatically between examples — no setup needed."
    end
  end
end
