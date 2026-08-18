require "net/http"
require "uri"
require_relative "signer"
require_relative "../../../runtime/registry"

module Hecksagain
  module Adapters
    class R2
      # THE TRANSPORT — GET/PUT against R2's S3-compatible REST API
      # (https://{account}.r2.cloudflarestorage.com/{bucket}/{key}), one
      # stateless HTTPS call per operation, same "no persistent handle to
      # hold open" shape D1::Connection already keeps for D1's own REST
      # endpoint. Every request is signed fresh with `Signer.sign` —
      # SigV4 credentials are time-boxed to the request's own `x-amz-date`,
      # there is nothing here to cache across calls.
      #
      # `http:` is an injected seam, not production configuration — real
      # callers never pass it and get `Net::HTTP` talking to Cloudflare.
      # `spec/adapters/driven/r2_spec.rb` passes a `.call(method, uri,
      # headers, body)`-shaped double so the round-trip spec can exercise
      # real SigV4 signing and the real Heki wire codec without a live
      # network call.
      class Connection
        REGION  = "auto" # R2's own fixed SigV4 region name for its S3-compatible API
        SERVICE = "s3"

        def initialize(account:, bucket:, access_key:, secret_key:, http: nil)
          @host       = "#{account}.r2.cloudflarestorage.com"
          @bucket     = bucket
          @access_key = access_key
          @secret_key = secret_key
          @http       = http || method(:net_http_request)
        end

        # Returns the object body as raw bytes, or nil if it does not
        # exist (a fresh aggregate's first read) — mirrors heki_r2.rs's
        # `read_record` returning an empty Store for a missing object.
        def get(key)
          status, body = request(:get, key, "")
          return nil if status == "404"
          raise Runtime::WiringError, "R2 GET #{key.inspect} failed (HTTP #{status}): #{body}" unless status == "200"

          body
        end

        def put(key, body)
          status, response_body = request(:put, key, body)
          return true if %w[200 201].include?(status)

          raise Runtime::WiringError, "R2 PUT #{key.inspect} failed (HTTP #{status}): #{response_body}"
        end

        private

        def request(method, key, body)
          path = "/#{@bucket}/#{key}"
          headers = Signer.sign(
            method: method, path: path, body: body,
            headers: { "Host" => @host, "X-Amz-Content-Sha256" => Digest::SHA256.hexdigest(body || "") },
            access_key: @access_key, secret_key: @secret_key, region: REGION, service: SERVICE
          )
          @http.call(method, URI("https://#{@host}#{path}"), headers, body)
        end

        # The real transport — swapped out in specs via `http:` above.
        def net_http_request(method, uri, headers, body)
          request_class = method == :put ? Net::HTTP::Put : Net::HTTP::Get
          request = request_class.new(uri)
          headers.each { |name, value| request[name] = value }
          request.body = body if method == :put

          response = Net::HTTP.start(uri.host, uri.port, use_ssl: true) { |http| http.request(request) }
          [response.code, response.body]
        end
      end
    end
  end
end
