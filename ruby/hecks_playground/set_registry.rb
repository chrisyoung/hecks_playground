# HecksPlayground::SetRegistry
#
# Array-backed registry for unique items. Prevents duplicates on register.
#
#   adapters = HecksPlayground::SetRegistry.new(%i[memory sqlite])
#   adapters.register(:postgres)
#   adapters.include?(:postgres)  # => true
#   adapters.all                  # => [:memory, :sqlite, :postgres]
#
module HecksPlayground
  # HecksPlayground::SetRegistry
  #
  # Array-backed registry for unique items with duplicate prevention and Enumerable support.
  #
  class SetRegistry
    include Enumerable

    def initialize(initial = [])
      @items = initial.dup
    end

    def register(item)
      coerced = item.respond_to?(:to_sym) ? item.to_sym : item
      @items << coerced unless @items.include?(coerced)
    end

    def include?(item)
      coerced = item.respond_to?(:to_sym) ? item.to_sym : item
      @items.include?(coerced)
    end

    def each(&block)
      @items.each(&block)
    end

    def all
      @items.dup
    end

    def empty?
      @items.empty?
    end
  end
end
