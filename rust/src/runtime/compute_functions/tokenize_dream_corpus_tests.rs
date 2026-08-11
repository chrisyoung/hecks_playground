//! tokenize_dream_corpus_tests — the dream-corpus tokenizer suite :
//! window filtering, token ranking, stopword keepability.
//!
//! Cask extracted VERBATIM from compute_functions/tokenize_dream_corpus.rs
//! (cask-runtime) ; body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/compute_functions/
//!  tokenize_dream_corpus_tests.rs — kernel-floor compute tests, relocated
//!  verbatim from tokenize_dream_corpus.rs blanket.]

use super::tokenize_dream_corpus::*;
use crate::heki;
use std::collections::HashMap;

#[test]
fn tokenize_drops_stopwords_and_short_words() {
    let toks = tokenize("the ocean dissolving in a library");
    assert_eq!(toks, vec!["ocean", "dissolving", "library"]);
}

#[test]
fn tokenize_handles_french_stopwords() {
    let toks = tokenize("je suis dans le monde");
    assert_eq!(toks, vec!["monde"]);
}

#[test]
fn empty_data_dir_returns_empty_string() {
    let attrs = HashMap::new();
    assert_eq!(tokenize_dream_corpus(None, &attrs, None), "");
}

#[test]
fn missing_dream_state_heki_returns_empty_string() {
    let attrs = HashMap::new();
    let tmp = std::env::temp_dir().join("tokenize_dream_corpus_missing");
    let _ = std::fs::create_dir_all(&tmp);
    assert_eq!(tokenize_dream_corpus(None, &attrs, tmp.to_str()), "");
}

#[test]
fn record_window_open_bounds_admits_everything() {
    let mut r: heki::Record = HashMap::new();
    r.insert("updated_at".into(), serde_json::json!("2026-05-03T00:00:00Z"));
    assert!(record_in_window(&r, "", ""));
}

#[test]
fn record_window_lower_bound_rejects_older() {
    let mut r: heki::Record = HashMap::new();
    r.insert("updated_at".into(), serde_json::json!("2026-05-01T00:00:00Z"));
    assert!(!record_in_window(&r, "2026-05-02T00:00:00Z", ""));
}

#[test]
fn record_window_upper_bound_rejects_newer() {
    let mut r: heki::Record = HashMap::new();
    r.insert("updated_at".into(), serde_json::json!("2026-05-05T00:00:00Z"));
    assert!(!record_in_window(&r, "", "2026-05-04T00:00:00Z"));
}

#[test]
fn extract_dream_images_array_form() {
    let mut r: heki::Record = HashMap::new();
    r.insert("dream_images".into(), serde_json::json!(["a", "b"]));
    assert_eq!(extract_dream_images(&r), vec!["a", "b"]);
}

#[test]
fn extract_dream_images_string_form() {
    let mut r: heki::Record = HashMap::new();
    r.insert("dream_images".into(), serde_json::json!("a single image"));
    assert_eq!(extract_dream_images(&r), vec!["a single image"]);
}
