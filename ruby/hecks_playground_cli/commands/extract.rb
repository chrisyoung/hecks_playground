HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Cli::CliInternals,
  base_dir: File.expand_path("..", __dir__)
)

# HecksPlayground CLI — extract command
#
# Auto-detects a project's type (Rails with schema, or models-only)
# and generates a HecksPlayground domain DSL file from the source.
#
#   hecks_playground extract /path/to/rails/app
#   hecks_playground extract /path/to/app --preview --name Blog
#
HecksPlayground::CLI.handle(:extract) do |inv|
  path = inv.args.first
  unless path
    puts "Usage: hecks_playground extract /path/to/project"
    puts "       hecks_playground extract /path/to/project --preview --name Blog"
    next
  end

  unless File.directory?(path)
    puts "Error: #{path} is not a directory"
    next
  end

  dsl = HecksPlayground::Import.from_directory(path, domain_name: options[:name])
  puts dsl

  unless options[:preview]
    output = options[:output] || "Bluebook"
    File.write(output, dsl)
    puts "\nWritten to #{output}"
  end
end
