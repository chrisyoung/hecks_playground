require "sdoc"
require "rdoc/task"

RDoc::Task.new do |rdoc|
  rdoc.generator = "sdoc"
  rdoc.main = "README.md"
  rdoc.title = "Hecks — Hexagonal DDD Framework for Ruby"
  rdoc.markup = "markdown"
  rdoc.rdoc_dir = "doc"
  rdoc.options << "--copy-files" << "hecks_logo.png"
  rdoc.rdoc_files.include("README.md", "FEATURES.md", "*/lib/**/*.rb", "docs/**/*.md")
end
