# HecksPlayground::BluebookModel::Names::EventName
#
# Value object wrapping an event name string (e.g. "CreatedPizza").
#
#   name = EventName.wrap("CreatedPizza")
#   name == "CreatedPizza"  # => true
#
module HecksPlayground
  module BluebookModel
    module Names
      class EventName < BaseName; end
    end
  end
end
