//! [antibody-exempt: rust/src/runtime/llm_providers/test.rs — kernel-floor
//!  LLM dispatcher port (Phase 2 i221) ; parallels existing kernel-floor
//!  runtime markers on adapter_io.rs / shell_dispatcher.rs / adapter_llm.rs.
//!  Retires when the LLM dispatch becomes bluebook-driven via :llm
//!  hecksagon adapters resolved at runtime + a dedicated dispatcher
//!  bluebook (filed as follow-up).]
//!
//! TestProvider — fixture-backed LLM provider keyed by SHA-256(prompt).
//! Mirrors `ruby/hecks/runtime/llm_providers/test_provider.rb`.
//!
//! Two miss modes :
//!   - lenient (default) : unknown prompts return a stable synthetic
//!     placeholder so downstream pipelines still flow ; useful while
//!     fixtures are still being captured.
//!   - strict (env HECKS_LLM_FIXTURE_STRICT=1 or `strict: true`) :
//!     unknown prompts surface a `LlmError::FixtureMissing { digest }`
//!     so CI fails fast.
//!
//! Fixture loading from disk is supported via `from_fixtures_file` —
//! two on-disk formats are auto-detected by content :
//!
//!   1. **Flat TSV** (CI-friendly, machine-generated) :
//!
//!        sha256-hex<TAB>response_text\n
//!
//!      Lines starting with `#` and blank lines are skipped.
//!
//!   2. **Ruby DSL `Hecks.fixtures`** (human-authored, the same shape
//!      miette's `body/dream/dream.fixtures` uses for documentation) :
//!
//!        Hecks.fixtures "BodyDream" do
//!          aggregate "Dream" do
//!            fixture "FrenchImage_Numbers",
//!              input:    "...substituted prompt...",
//!              response: "...French response..."
//!          end
//!        end
//!
//!      Each row's `input` attribute is hashed via SHA-256 ; `response`
//!      is the lookup value. Rows without both `input` and `response`
//!      are skipped. The aggregate label is metadata only — every
//!      fixture row across every aggregate goes into one prompt-keyed
//!      lookup map (the dispatcher matches by SHA, not by aggregate).
//!
//! Either way, the hex digest is the SHA-256 of the **substituted**
//! prompt (after `{{placeholder}}` tokens are filled), which is exactly
//! what the dispatcher hands to `invoke`. The Ruby DSL form lets
//! authors document the canned responses as readable French sentences
//! ; the flat TSV is what test harnesses regenerate when they need a
//! specific seed/state combination.

use super::{LlmError, LlmProvider, LlmProviderResult};
use std::collections::HashMap;
use std::path::Path;

pub struct TestProvider {
    fixtures: HashMap<String, String>,
    strict: bool,
}

impl TestProvider {
    /// Build a provider with a literal sha256→response map. Keys may be
    /// supplied as either the SHA-256 hex digest of the prompt OR as the
    /// raw prompt string ; raw strings are hashed on insertion so call
    /// sites can register fixtures without re-deriving the digest.
    pub fn new(fixtures: HashMap<String, String>) -> Self {
        let mut normalized: HashMap<String, String> = HashMap::new();
        for (k, v) in fixtures {
            let key = if k.len() == 64 && k.chars().all(|c| c.is_ascii_hexdigit()) {
                k
            } else {
                Self::hash_for(&k)
            };
            normalized.insert(key, v);
        }
        let strict = std::env::var("HECKS_LLM_FIXTURE_STRICT")
            .ok().as_deref() == Some("1");
        Self { fixtures: normalized, strict }
    }

    /// Build a provider whose fixtures are loaded from a fixtures file.
    /// Auto-detects the format by content :
    ///   - Lines starting with `Hecks.fixtures` → Ruby DSL form (parses
    ///     each `fixture` row's `input:` + `response:` attributes,
    ///     hashes input via SHA-256, stores response as the value).
    ///   - Otherwise → flat TSV form (`sha256-hex<TAB>response`).
    /// Missing file → empty fixtures (the lenient default kicks in on
    /// first invoke). Malformed lines are skipped silently.
    pub fn from_fixtures_file<P: AsRef<Path>>(path: P) -> Self {
        let mut fixtures: HashMap<String, String> = HashMap::new();
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if Self::looks_like_ruby_dsl(&contents) {
                fixtures = Self::parse_ruby_dsl_fixtures(&contents);
            } else {
                for line in contents.lines() {
                    let l = line.trim_end_matches('\r');
                    if l.is_empty() || l.starts_with('#') { continue; }
                    if let Some((digest, body)) = l.split_once('\t') {
                        let d = digest.trim().to_string();
                        if d.len() == 64 && d.chars().all(|c| c.is_ascii_hexdigit()) {
                            fixtures.insert(d, body.to_string());
                        }
                    }
                }
            }
        }
        let strict = std::env::var("HECKS_LLM_FIXTURE_STRICT")
            .ok().as_deref() == Some("1");
        Self { fixtures, strict }
    }

    /// Heuristic — content peek for the Ruby DSL outer block. Cheap
    /// scan of the first few non-blank, non-comment lines ; the DSL
    /// header is always `Hecks.fixtures "<Domain>" do`.
    fn looks_like_ruby_dsl(contents: &str) -> bool {
        for line in contents.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') { continue; }
            return t.starts_with("Hecks.fixtures");
        }
        false
    }

    /// Walk a `Hecks.fixtures` file via the existing fixtures parser,
    /// pull the `input` + `response` attributes off each row, and build
    /// a SHA-256-keyed lookup map. Rows missing either attribute are
    /// skipped (the row is documentation only). The aggregate label is
    /// metadata — every row across every aggregate folds into one
    /// prompt-keyed map (the dispatcher matches by SHA, not aggregate).
    fn parse_ruby_dsl_fixtures(contents: &str) -> HashMap<String, String> {
        let mut out: HashMap<String, String> = HashMap::new();
        let parsed = crate::fixtures_parser::parse(contents);
        for fixture in &parsed.fixtures {
            let mut input: Option<&str> = None;
            let mut response: Option<&str> = None;
            for (k, v) in &fixture.attributes {
                match k.as_str() {
                    "input"    => input = Some(v.as_str()),
                    "response" => response = Some(v.as_str()),
                    _ => {}
                }
            }
            if let (Some(prompt), Some(reply)) = (input, response) {
                out.insert(Self::hash_for(prompt), reply.to_string());
            }
        }
        out
    }

    /// Force strict mode (used by tests).
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    pub fn fixture_count(&self) -> usize {
        self.fixtures.len()
    }

    /// SHA-256 hex digest of `prompt` — the canonical fixture key.
    /// Pure Rust implementation (32 bytes of state, 64-byte block) so
    /// the crate adds no `sha2` dep.
    pub fn hash_for(prompt: &str) -> String {
        let bytes = sha256(prompt.as_bytes());
        let mut hex = String::with_capacity(64);
        for b in &bytes {
            hex.push_str(&format!("{:02x}", b));
        }
        hex
    }
}

impl LlmProvider for TestProvider {
    fn invoke(
        &self,
        prompt: &str,
        model: &str,
        _max_tokens: u64,
    ) -> Result<LlmProviderResult, LlmError> {
        let digest = Self::hash_for(prompt);
        let response_text = match self.fixtures.get(&digest) {
            Some(text) => text.clone(),
            None => {
                if self.strict {
                    return Err(LlmError::FixtureMissing { digest });
                }
                format!("[test-provider:unknown-prompt sha256={}]", digest)
            }
        };
        Ok(LlmProviderResult {
            response_text: response_text.clone(),
            tokens_in: estimate_tokens(prompt),
            tokens_out: estimate_tokens(&response_text),
            model_used: model.to_string(),
        })
    }
}

use super::test_digest::{estimate_tokens, sha256};
