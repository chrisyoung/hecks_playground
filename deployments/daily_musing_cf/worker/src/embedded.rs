// AUTO-GENERATED — daily-musing bluebooks embedded into the Worker WASM.
// To regenerate :
//   python3 worker/scripts/regenerate-embedded.py
//
// Embedded files : 3
// Primary        : daily_musing.bluebook
// `.fixtures` files are never embedded (fixtures→policies, 2026-07-26) —
// demo records establish via `on "BootCompleted"` policies at CompleteBoot.

pub const PRIMARY_BASENAME: &str = "daily_musing.bluebook";
pub const BLUEBOOK_COUNT:   usize = 3;

pub static EMBEDDED_BLUEBOOKS: &[(&str, &str)] = &[
    ("daily_musing.bluebook", r#####"HecksPlayground.bluebook "DailyMusing", version: "2026.05.23.2" do
  vision "An author posts blog entries to the Daily Musing so readers can read them."
  category "blog"

  aggregate "BlogEntry", "A single post in the Daily Musing — its title, body, author, and published state" do
    # The natural key establishment upserts on (fixtures→policies,
    # 2026-07-26) : re-Posting a title updates that entry, never duplicates.
    identified_by :title
    attribute :title, Title
    attribute :body, Body
    attribute :author, AuthorName
    attribute :published_at, PublishedAt

    value_object "Title" do
      attribute :value, String
    end
    value_object "Body" do
      attribute :value, String
    end
    value_object "AuthorName" do
      attribute :value, String
    end
    value_object "PublishedAt" do
      attribute :value, String
    end

    command "Post" do
      role "Author"
      description "Publish a blog entry to the Daily Musing so readers can read it"
      attribute :title, Title
      attribute :body, Body
      attribute :author, AuthorName
      attribute :published_at, PublishedAt
      then_set :title, to: :title
      then_set :body, to: :body
      then_set :author, to: :author
      then_set :published_at, to: :published_at
      emits "EntryPosted"
    end

    # No from-gate : establishment re-asserts by re-Posting the same title
    # (idempotent upsert on :title), so Post must land "published" from any
    # state — a from: "draft" gate would turn re-establishment into a
    # LifecycleViolation instead of an upsert.
    lifecycle :status, default: "draft" do
      transition "Post" => "published"
    end

    query "DailyMusing" do
      description "Readers' view — every published entry in the Daily Musing"
      where status: "published"
    end
  end

  # The demo musings as ESTABLISHMENT (fixtures→policies, 2026-07-26) —
  # `on "BootCompleted"` seeds the two published entries the retired
  # daily_musing.fixtures carried. The Worker's generated lib.rs dispatches
  # CompleteBoot on every per-request boot (its boot IS a boot), so a fresh
  # memory-only runtime always has a readable Daily Musing ; R2 state still
  # layers on top afterwards and wins by id. Bite :
  # rust/tests/daily_musing_establishment_test.rs.
  policy "EstablishWelcomeMusing" do
    on "BootCompleted"
    trigger "BlogEntry.Post"
    with "title", "Welcome to the Daily Musing"
    with "body", "This blog runs on a single HecksPlayground bluebook compiled to WebAssembly and served from a Cloudflare Worker — no container, no server. The domain IS the contract."
    with "author", "Miette"
    with "published_at", "2026-05-23T08:00:00Z"
  end

  policy "EstablishMorningLightMusing" do
    on "BootCompleted"
    trigger "BlogEntry.Post"
    with "title", "Morning Light"
    with "body", "A quiet musing on starting the day with a clear bluebook and a warm runtime."
    with "author", "Chris"
    with "published_at", "2026-05-23T09:30:00Z"
  end
end
"#####),
    ("boot.bluebook", r#####"HecksPlayground.bluebook "Boot" do
  vision "The Worker's per-request boot IS a boot — it completes like one. CompleteBoot emits BootCompleted so every `on \"BootCompleted\"` establishment policy self-seeds (fixtures→policies, 2026-07-26 : the demo musings ride this, not fixture seeding)."
  category "boot"

  aggregate "BootRun" do
    identified_by :phase
    attribute :phase, Phase, default: "pending"

    value_object "Phase" do
      attribute :value, String
    end

    command "CompleteBoot" do
      role "System"
      description "Complete the per-request boot — BootCompleted fans out to every establishment policy"
      emits "BootCompleted"
    end
  end
end
"#####),
    ("daily_musing.hecksagon", r#####"HecksPlayground.hecksagon "DailyMusing" do
  adapter :memory
end
"#####),
];
