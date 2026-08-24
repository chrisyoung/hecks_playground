# = HecksMongodb
#
# MongoDB persistence adapter for HecksPlayground domains. Provides document-based
# repository adapters using the mongo Ruby driver. Each aggregate maps
# to a MongoDB collection.
#
# == Usage
#
#   app = HecksPlayground.boot(__dir__, adapter: :mongodb)
#   app = HecksPlayground.boot(__dir__, adapter: { type: :mongodb, uri: "mongodb://localhost:27017/mydb" })
#
require "hecks_playground_mongodb/mongo_boot"
require "hecks_playground_mongodb/mongo_adapter_generator"

HecksPlayground.register_adapter(:mongodb)
