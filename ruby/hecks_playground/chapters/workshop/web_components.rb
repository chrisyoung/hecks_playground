# HecksPlayground::Chapters::Workshop::WebComponentsParagraph
#
# Paragraph covering WebRunner children: evaluator, command parser,
# and state serializer.
#
#   HecksPlayground::Chapters::Workshop::WebComponentsParagraph.define(builder)
#
module HecksPlayground
  module Chapters
    module Workshop
      module WebComponentsParagraph
        def self.define(b)
          b.aggregate "Evaluator" do
            description "Safe eval-free command execution delegating to CommandParser"
            command "Evaluate" do
              attribute :input, String
            end
          end

          b.aggregate "CommandParser" do
            description "Safe command dispatcher using BlueBook Grammar for the web workshop"
            command "Execute" do
              attribute :input, String
            end
          end

          b.aggregate "StateSerializer" do
            description "Serializes workshop state into JSON for the browser console"
            command "Serialize"
          end
        end
      end
    end
  end
end
