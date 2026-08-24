# Hecks::Adapters::Email — the execution port's Gmail implementation.
#
# Vendored addition, not (yet) upstream hecks. Answers
# Tools::EmailTool (bound `Tools::EmailTool.executed_by("Email")`), the
# impure edge behind every Gmail command the door carries. See
# ports/execution.rb for why this port exists at all, and i757 (hecks
# inbox) for why this adapter exists : the corpus's tools.hecksagon
# named a `:mcp`/`:web_tool` "adapter" form for EmailTool that was never
# actually resolved by anything in the live runtime — a dispatch
# recorded its event and did NOTHING. Proven live 2026-08-14 : a
# dispatch of EmailTool.GetAttachment reported AttachmentFetched and
# the file never got written ; EmailTool.CreateDraft reported
# DraftCreated and no draft ever appeared.
#
# THE FIX IS THE SAME SHAPE Shell/Filesystem/Search already use — a
# real `executed_by` bind to a real Ruby adapter class — NOT a new
# effect-port/pending-instruction mechanism. The reason this is
# possible for Gmail specifically, where a bare Ruby process has no
# access to a Claude session's own connected MCP tools : Miette's own
# custom-registered Google Cloud OAuth app already holds a real,
# sufficiently-scoped (gmail.readonly, gmail.modify, gmail.send) token
# at ~/.config/miette/google-oauth-token.json, proven working for
# attachment fetches since 2026-05-18 (the transitional
# fetch_gmail_attachment.py script this supersedes). Every EmailTool
# operation is a plain HTTPS call against the real Gmail REST API with
# that token — no agent bridge needed, same as Shell shelling out or
# Filesystem touching disk.
#
# Usage (never called directly — the dispatcher reaches it through the
# port):
#   Hecks::Adapters::Email.execute("GetAttachment",
#     message_id: "...", attachment_id: "...", save_path: "/tmp/x.png")
#   # => { tool: "gmail", output: "...", exit_code: 0, ok: true }

require "net/http"
require "uri"
require "json"
require "base64"
require "fileutils"

module Hecks
  module Adapters
    module Email
      TOOL = "gmail".freeze
      TOKEN_PATH = File.expand_path("~/.config/miette/google-oauth-token.json").freeze
      API_ROOT = "https://gmail.googleapis.com/gmail/v1/users/me".freeze

      module_function

      def execute(operation, args)
        case operation
        when "SearchThreads"  then search_threads(args)
        when "GetThread"      then get_thread(args)
        when "CreateDraft"    then create_draft(args)
        when "ListDrafts"     then list_drafts(args)
        when "GetAttachment"  then get_attachment(args)
        else refuse(operation)
        end
      rescue StandardError => error
        # Same discipline Shell's own #run follows : a failure to reach
        # Gmail at all is RECORDED, never raised — the Cascade record is
        # the audit trail, and raising here would lose both the attempt
        # and the reason it never completed.
        { tool: TOOL, output: "#{error.class}: #{error.message}", exit_code: -1, ok: false }
      end

      def search_threads(args)
        ok_result(get("/threads?q=#{URI.encode_www_form_component(args[:query].to_s)}"))
      end

      def get_thread(args)
        ok_result(get("/threads/#{URI.encode_www_form_component(args[:thread_id].to_s)}?format=full"))
      end

      def list_drafts(_args)
        ok_result(get("/drafts"))
      end

      # No thread-reply attribute exists on Tools::EmailTool.CreateDraft's
      # own bluebook shape today (id/to/subject/body/description only) —
      # this creates a top-level draft, matching what the domain actually
      # declares. Threading a reply onto an existing message is a real,
      # separate bluebook gap (add a reply_to_message_id attribute), not
      # fixed here.
      def create_draft(args)
        raw = build_mime(to: args[:to].to_s, subject: args[:subject].to_s, body: args[:body].to_s)
        ok_result(post("/drafts", { message: { raw: raw } }))
      end

      def get_attachment(args)
        payload = get("/messages/#{URI.encode_www_form_component(args[:message_id].to_s)}" \
                       "/attachments/#{URI.encode_www_form_component(args[:attachment_id].to_s)}")
        data = payload.fetch("data")
        bytes = Base64.urlsafe_decode64(data + ("=" * ((4 - (data.length % 4)) % 4)))
        path = args[:save_path].to_s
        FileUtils.mkdir_p(File.dirname(path))
        File.binwrite(path, bytes)
        ok_result({ ok: true, bytes: bytes.bytesize, save_path: path })
      end

      def refuse(operation)
        { tool: TOOL, output: "unknown EmailTool operation #{operation.inspect}", exit_code: -1, ok: false }
      end

      def ok_result(json)
        { tool: TOOL, output: JSON.generate(json), exit_code: 0, ok: true }
      end

      # ── Gmail REST, OAuth token refresh — the same mechanism
      # fetch_gmail_attachment.py already proved live, ported to Ruby so
      # every EmailTool operation shares it, not just attachments. ──

      def get(path)  = request(Net::HTTP::Get, path)
      def post(path, body) = request(Net::HTTP::Post, path, body)

      def request(klass, path, body = nil)
        uri = URI("#{API_ROOT}#{path}")
        res = perform(klass, uri, body, access_token)
        res = perform(klass, uri, body, refresh_token!) if res.code.to_i == 401 || res.code.to_i == 403
        raise "Gmail API #{res.code}: #{res.body}" unless res.code.to_i.between?(200, 299)

        JSON.parse(res.body)
      end

      def perform(klass, uri, body, token)
        http = Net::HTTP.new(uri.host, uri.port)
        http.use_ssl = true
        req = klass.new(uri)
        req["Authorization"] = "Bearer #{token}"
        if body
          req["Content-Type"] = "application/json"
          req.body = JSON.generate(body)
        end
        http.request(req)
      end

      def access_token
        credentials["token"] || credentials["access_token"]
      end

      def credentials
        @credentials ||= JSON.parse(File.read(TOKEN_PATH))
      end

      def refresh_token!
        creds = credentials
        uri = URI(creds["token_uri"] || "https://oauth2.googleapis.com/token")
        res = Net::HTTP.post_form(uri, {
                                     "grant_type" => "refresh_token",
                                     "refresh_token" => creds["refresh_token"],
                                     "client_id" => creds["client_id"].to_s,
                                     "client_secret" => creds["client_secret"].to_s,
                                   })
        new_token = JSON.parse(res.body).fetch("access_token")
        creds["token"] = new_token
        File.write(TOKEN_PATH, JSON.generate(creds))
        @credentials = creds
        new_token
      end

      def build_mime(to:, subject:, body:)
        message = "To: #{to}\r\nSubject: #{subject}\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n#{body}"
        Base64.urlsafe_encode64(message)
      end
    end
  end
end
