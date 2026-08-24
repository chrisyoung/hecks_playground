require "openssl"
require "digest"

module Hecks
  module Adapters
    class R2
      # Hand-rolled AWS Signature Version 4 — R2's S3-compatible API needs
      # real SigV4 auth, not a bearer token (unlike D1::Connection, this
      # class's own nearest sibling in this directory, which only needed
      # `Authorization: Bearer <token>`). No new gem for this: `aws-sdk-s3`
      # pulls in the whole AWS SDK chain (aws-sdk-core, aws-partitions,
      # jmespath, aws-sigv4 itself) for a bucket this adapter only ever
      # GETs/PUTs two keys against per aggregate. SigV4 is a documented,
      # deterministic algorithm — four HMAC-SHA256 rounds over a canonical
      # request — exactly the kind of small vendored crypto D1::Connection
      # already set the precedent for hand-rolling rather than reaching for
      # a heavy client library.
      #
      # Pure and header-driven on purpose: `sign` takes exactly the headers
      # the caller wants signed and returns them back with `Authorization`
      # added, with no R2/S3-specific assumptions baked in. That is what
      # lets `spec/adapters/driven/r2_spec.rb` check this module directly
      # against AWS's own published SigV4 test vectors (the
      # "aws-sig-v4-test-suite" `get-vanilla` / `post-vanilla` /
      # `post-x-www-form-urlencoded` cases) — proof the core algorithm is
      # right, independent of anything R2-shaped. `Connection` (connection.rb,
      # this same directory) is the R2-specific caller: it decides which
      # headers R2 needs signed (host, x-amz-date, x-amz-content-sha256)
      # and builds the bucket/key path.
      module Signer
        module_function

        ALGORITHM = "AWS4-HMAC-SHA256"

        # `headers` — a Hash of header-name => value, exactly as they will
        # be sent (case as given; canonicalization lowercases internally).
        # Returns `headers` merged with an `"Authorization"` entry.
        def sign(method:, path:, headers:, body:, access_key:, secret_key:, region:, service:, time: Time.now.utc)
          amz_date = time.strftime("%Y%m%dT%H%M%SZ")
          date     = time.strftime("%Y%m%d")

          creq = canonical_request(method: method, path: path, headers: headers, body: body)
          scope = "#{date}/#{region}/#{service}/aws4_request"
          sts = string_to_sign(amz_date: amz_date, scope: scope, canonical_request: creq[:text])

          signature = OpenSSL::HMAC.hexdigest("SHA256", signing_key(secret_key, date, region, service), sts)

          authorization = "#{ALGORITHM} Credential=#{access_key}/#{scope}, " \
                           "SignedHeaders=#{creq[:signed_headers]}, Signature=#{signature}"

          headers.merge("Authorization" => authorization)
        end

        # Split out for `spec/adapters/driven/r2_spec.rb` to assert the
        # exact multi-line text AWS's own test vectors publish, before
        # trusting the signature that text produces.
        def canonical_request(method:, path:, headers:, body:)
          names = headers.keys.map(&:downcase).sort
          lower = headers.transform_keys(&:downcase)
          canonical_headers = names.map { |name| "#{name}:#{lower[name].to_s.strip}\n" }.join
          signed_headers = names.join(";")
          payload_hash = Digest::SHA256.hexdigest(body || "")

          text = [
            method.to_s.upcase,
            uri_encode_path(path),
            "", # no query string — this adapter never sends one
            canonical_headers,
            signed_headers,
            payload_hash
          ].join("\n")

          { text: text, signed_headers: signed_headers, payload_hash: payload_hash }
        end

        def string_to_sign(amz_date:, scope:, canonical_request:)
          [ALGORITHM, amz_date, scope, Digest::SHA256.hexdigest(canonical_request)].join("\n")
        end

        def signing_key(secret_key, date, region, service)
          k_date    = OpenSSL::HMAC.digest("SHA256", "AWS4#{secret_key}", date)
          k_region  = OpenSSL::HMAC.digest("SHA256", k_date, region)
          k_service = OpenSSL::HMAC.digest("SHA256", k_region, service)
          OpenSSL::HMAC.digest("SHA256", k_service, "aws4_request")
        end

        # S3 (and R2, the same wire dialect) deliberately does NOT
        # normalize dot-segments out of the path the way the generic
        # SigV4 algorithm does elsewhere — an object key is free to
        # contain "." or ".." literally. Every byte is percent-encoded
        # except the unreserved set; "/" is preserved as a segment
        # separator, never itself encoded.
        def uri_encode_path(path)
          path.to_s.split("/", -1).map { |segment| uri_encode_component(segment) }.join("/")
        end

        def uri_encode_component(segment)
          segment.b.each_byte.map { |byte| unreserved?(byte) ? byte.chr : format("%%%02X", byte) }.join
        end

        def unreserved?(byte)
          char = byte.chr
          char.match?(/[A-Za-z0-9\-_.~]/)
        end
      end
    end
  end
end
