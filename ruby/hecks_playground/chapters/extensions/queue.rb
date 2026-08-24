# = HecksPlayground::Chapters::Extensions::QueueChapter
#
# Self-describing sub-chapter for queue extension internals:
# RabbitMQ adapter for event publishing.
#
#   HecksPlayground::Chapters::Extensions::QueueChapter.define(builder)
#
module HecksPlayground
  module Chapters
    module Extensions
      # HecksPlayground::Chapters::Extensions::QueueChapter
      #
      # Bluebook sub-chapter for queue extension internals: RabbitMQ adapter for event publishing.
      #
      module QueueChapter
        def self.define(b)
          b.aggregate "RabbitMqAdapter", "RabbitMQ queue adapter that publishes events by routing key" do
            command("Publish") { attribute :routing_key, String; attribute :payload, String }
          end
        end
      end
    end
  end
end
