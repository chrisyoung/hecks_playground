# Hecks::Parity::StorehouseBin
#
# Resolves the storehouse binary the parity harnesses shell out to — and BUILDS
# it first.
#
# Every harness used to do this :
#
#   STOREHOUSE = ENV.fetch("STOREHOUSE_BIN") { ".../target/release/storehouse" }
#   abort "storehouse not built" unless File.executable?(STOREHOUSE)
#
# which checks that a binary EXISTS, never that it matches the source it is
# about to be compared against. A stale binary makes the Rust side report the
# parser it had at build time while the Ruby side reports the parser on disk, so
# the diff is between two different days. It reads as real drift, and it has
# cost this project whole sessions — most recently the `identity` field, which
# the dump "did not have" for several confusing minutes because the CLI is its
# own crate and `cargo test` had only rebuilt the lib.
#
# So the build is a PREREQUISITE, not a precondition to check. Cargo is a no-op
# when nothing changed, so a fresh tree pays ~0.1s for the guarantee.
#
#   require_relative "storehouse_bin"
#   STOREHOUSE = Hecks::Parity::StorehouseBin.path
#
# HECKS_SKIP_BUILD=1 skips it, for a CI job that built the binary in an earlier
# step and passes STOREHOUSE_BIN explicitly.
module Hecks
  module Parity
    module StorehouseBin
      RUST_DIR = File.expand_path("../rust", __dir__)
      DEFAULT  = File.expand_path("../rust/target/release/storehouse", __dir__)

      class << self
        def path
          @path ||= resolve
        end

        private

        def resolve
          bin = ENV.fetch("STOREHOUSE_BIN", DEFAULT)
          build!(bin) unless ENV["HECKS_SKIP_BUILD"] == "1"
          unless File.executable?(bin)
            abort "storehouse not built and could not be built: #{bin}"
          end
          bin
        end

        # Build the profile the requested path asks for, so a harness pointed at
        # target/debug does not silently get a release binary or vice versa.
        def build!(bin)
          profile = bin.include?("/debug/") ? [] : ["--release"]
          cmd = ["cargo", "build", "-p", "storehouse-cli", *profile]
          warn "[parity] #{cmd.join(' ')} — so the binary matches the source it is compared against"
          ok = system(*cmd, chdir: RUST_DIR, out: :err)
          abort "[parity] #{cmd.join(' ')} FAILED — refusing to compare against a stale binary" unless ok
        end
      end
    end
  end
end
