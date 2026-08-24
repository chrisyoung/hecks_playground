# HecksPlayground::Capabilities::Readme
#
# Generates HECKS_PLAYGROUND_README.md in the project directory on boot.
# Documents the domain's aggregates, commands, capabilities,
# world config, and routing decisions. Regenerated every boot
# so it's always in sync with the Bluebook.
#
#   HecksPlayground.hecksagon "MyApp" do
#     capabilities :readme
#   end
#
# Generates: HECKS_PLAYGROUND_README.md in Dir.pwd
#
require_relative "dsl"
require_relative "readme/generator"

module HecksPlayground
  module Capabilities
    # HecksPlayground::Capabilities::Readme
    #
    # Auto-generates project README from domain IR + capability metadata.
    #
    module Readme
      def self.apply(runtime)
        generator = Generator.new(runtime)
        output_dir = runtime.respond_to?(:root) ? runtime.root : Dir.pwd
        path = File.join(output_dir, "HECKS_PLAYGROUND_README.md")
        File.write(path, generator.generate)
        $stderr.puts "[Readme] Generated #{path}"
      end
    end
  end
end

HecksPlayground.capability :readme do
  description "Auto-generate HECKS_PLAYGROUND_README.md from domain IR on boot"
  direction :driven
  on_apply do |runtime|
    HecksPlayground::Capabilities::Readme.apply(runtime)
  end
end
