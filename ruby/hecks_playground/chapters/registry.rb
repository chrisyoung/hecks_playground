# HecksPlayground::Chapters — Chapter Registration
#
# Registers all available chapters with ChapterLoader. Each entry
# declares what to require and how to wire the chapter into the
# framework. Chapters are loaded selectively via HecksPlayground.chapters.
#
#   HecksPlayground.chapters :bluebook, :runtime
#
module HecksPlayground
  ChapterLoader.register(:bluebook,
    requires: %w[bluebook]
  ) do
    Chapters.load_chapter(
      Chapters::Bluebook,
      base_dirs: %w[
        hecks_playground/domain hecks_playground/bluebook_model hecks_playground/dsl hecks_playground/generators
        hecks_playground/validation_rules hecks_playground/event_storm hecks_playground/features
        hecks_playground/extensions/docs bluebook hecks_playground_persist hecks_playground_mongodb
      ].map { |d| File.join(__dir__, "../..", d) }
    )
  end

  ChapterLoader.register(:packaging,
    requires: %w[hecks_playground/chapters/packaging]
  )

  ChapterLoader.register(:runtime,
    requires: %w[
      hecks_playground/stats hecks_playground/event_sourcing hecks_playground/runtime/boot
      hecks_playground/chapters/runtime hecks_playground/chapters/workshop hecks_playground/deprecations
      hecks_playground/runtime hecks_playground/runtime/boot_bluebook
    ]
  )

  ChapterLoader.register(:hecksagon,
    requires: %w[hecksagon]
  )

  ChapterLoader.register(:workshop,
    requires: %w[hecks_playground/chapters/workshop hecks_playground/workshop]
  )

  ChapterLoader.register(:ai,
    requires: %w[hecks_playground_ai]
  )

  # Appeal is a bluebook project — boots via HecksPlayground.boot, not as a chapter

  ChapterLoader.register(:cli,
    requires: %w[hecks_playground_cli/cli]
  )

  ChapterLoader.register(:extensions,
    requires: %w[]
  ) do
    begin; require "hecks_playground/extensions"; rescue LoadError; end
  end

  ChapterLoader.register(:targets,
    requires: %w[hecks_playground/chapters/targets]
  ) do
    begin; require "go_hecks_playground"; rescue LoadError; end
    begin; require "node_hecks_playground"; rescue LoadError; end
    begin; require "hecks_playground_static"; rescue LoadError; end
  end

  ChapterLoader.register(:persist,
    requires: %w[hecks_playground_persist]
  )

  ChapterLoader.register(:multidomain,
    requires: %w[hecks_playground_multidomain]
  )

  ChapterLoader.register(:rails,
    requires: %w[]
  ) do
    require "active_hecks_playground/railtie" if defined?(::Rails::Railtie)
  end

  ChapterLoader.register(:features,
    requires: %w[hecks_playground/features]
  )
end
