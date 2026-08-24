# HecksPlayground::BluebookModel::Names::StateName
#
# Value object wrapping a lifecycle state name (e.g. "draft", "published").
#
#   name = StateName.wrap("draft")
#   name == "draft"  # => true
#
module HecksPlayground
  module BluebookModel
    module Names
      class StateName < BaseName; end
    end
  end
end
