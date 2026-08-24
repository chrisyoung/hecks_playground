require "zlib"

module Hecks
  module Adapters
    class Heki
      # The binary snapshot codec: magic-header framing around
      # deflate-compressed, id-sorted JSON. Reading refuses loudly on a
      # short blob, a bad magic, or a body neither zlib nor JSON will own.
      #
      # `encode`/`decode` are the pure wire-format half, deliberately
      # free of any `@path`/File reference — `adapters/driven/r2.rb`
      # calls these same two module methods directly (over HTTP GET/PUT
      # against a Cloudflare R2 object) so the bytes it writes are
      # byte-for-byte what a local Heki store would hold, matching
      # `rust/src/heki_r2.rs`'s own encode/decode on the fork's Rust
      # side by design. `read_snapshot`/`write` below stay Heki's own
      # filesystem wrapping of that codec — unchanged behavior.
      module Snapshot
        def self.encode(records)
          sorted = records.sort_by { |id, _| id }.to_h
          json   = JSON.generate(sorted)
          MAGIC + [sorted.size].pack("N") + Zlib::Deflate.deflate(json, Zlib::BEST_COMPRESSION)
        end

        def self.decode(data)
          raise Malformed, "too short" if data.bytesize < HEADER_BYTES
          raise Malformed, "bad magic" unless data[0, 4] == MAGIC

          JSON.parse(Zlib::Inflate.inflate(data[HEADER_BYTES..]))
        rescue Zlib::DataError => e
          raise Malformed, "zlib error: #{e.message}"
        rescue JSON::ParserError => e
          raise Malformed, "json error: #{e.message}"
        end

        private

        def read_snapshot
          return {} unless File.exist?(@path)

          Snapshot.decode(File.binread(@path))
        rescue Malformed => e
          raise Malformed, "#{@path}: #{e.message}"
        end

        def write(records)
          File.binwrite(@path, Snapshot.encode(records))
        end
      end
    end
  end
end
