# HecksPlayground::Features
#
# Vertical slice architecture. Extracts vertical slices from domain
# reactive chains, validates slice boundaries, and generates diagrams.
#
# Implementation files loaded from the FeaturesParagraph chapter definition.
# Uses narrow base_dir to avoid parent-child filter excluding these files.
#
HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Bluebook::FeaturesParagraph,
  base_dir: File.expand_path("features", __dir__)
)

HecksPlayground::BluebookModel::Structure::Domain.include(HecksPlayground::Features::DomainMixin)

# Backward compat
HecksFeatures = HecksPlayground::Features unless defined?(HecksFeatures)
