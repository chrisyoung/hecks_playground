# HecksPlayground::Generator
#
# Base class for all HecksPlayground code generators. Provides NamingHelpers
# and the generate contract. Subclasses implement #generate to return
# generated source (String) or structured data (Hash).
#
#   class MyGenerator < HecksPlayground::Generator
#     def generate
#       "class #{bluebook_constant_name(@model.name)}; end"
#     end
#   end
#

module HecksPlayground
  class Generator
    include HecksPlayground::Conventions::NamingHelpers

    def generate
      raise NotImplementedError, "#{self.class}#generate not implemented"
    end
  end
end
