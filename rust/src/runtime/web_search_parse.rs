//! web_search_parse — the DuckDuckGo HTML-lite parsing layer of the
//! :web_tool family : SearchResult, parse_duckduckgo_html,
//! format_search_results, and the string leaves (extract_attr, strip_tags,
//! clean_ddg_url, percent_decode, hex_val, url_encode). Pure functions ;
//! the HTTP runners stay in web_tool_dispatcher.rs.
//!
//! Cask extracted VERBATIM from runtime/web_tool_dispatcher.rs
//! (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/web_search_parse.rs — kernel-floor
//!  web-tool parsing, relocated verbatim from web_tool_dispatcher.rs
//!  blanket.]

/// A parsed DuckDuckGo result block — (title, url, snippet).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SearchResult {
    pub(super) title: String,
    pub(super) url: String,
    pub(super) snippet: String,
}

/// Walk DuckDuckGo's HTML-lite response, extract up to `limit`
/// result blocks. The shape is small enough that a regex-style
/// substring walk is robust ; introducing a real HTML parser is
/// follow-on work if/when the shape complexity grows.
pub(super) fn parse_duckduckgo_html(html: &str, limit: usize) -> Vec<SearchResult> {
    let mut results: Vec<SearchResult> = Vec::new();
    let mut cursor = 0;
    while results.len() < limit {
        // Each result block opens with `class="result__a"` on an
        // anchor element. We find the anchor, then walk its href
        // attribute, then its text content for the title, then look
        // for the next snippet anchor / div with class result__snippet.
        let block_start = match html[cursor..].find("class=\"result__a\"") {
            Some(i) => cursor + i,
            None => break,
        };
        // Locate the anchor's opening '<a' before the class attr.
        let anchor_open = match html[..block_start].rfind("<a ") {
            Some(i) => i,
            None => {
                cursor = block_start + 1;
                continue;
            }
        };
        // Extract href="..."
        let anchor_end = match html[anchor_open..].find('>') {
            Some(i) => anchor_open + i,
            None => break,
        };
        let anchor_attrs = &html[anchor_open..anchor_end];
        let url = match extract_attr(anchor_attrs, "href") {
            Some(u) => u,
            None => {
                cursor = anchor_end + 1;
                continue;
            }
        };
        // Title is the text between '>' and '</a>'.
        let title_start = anchor_end + 1;
        let title_end = match html[title_start..].find("</a>") {
            Some(i) => title_start + i,
            None => break,
        };
        let title = strip_tags(&html[title_start..title_end]).trim().to_string();
        if title.is_empty() {
            cursor = title_end + 4;
            continue;
        }
        // Look for a snippet right after — class="result__snippet"
        let snippet = match html[title_end..].find("class=\"result__snippet\"") {
            Some(snip_idx) => {
                let abs = title_end + snip_idx;
                if let Some(snip_open_end) = html[abs..].find('>') {
                    let snip_text_start = abs + snip_open_end + 1;
                    if let Some(snip_text_end) = html[snip_text_start..].find("</a>") {
                        strip_tags(&html[snip_text_start..snip_text_start + snip_text_end])
                            .trim()
                            .to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
            None => String::new(),
        };

        results.push(SearchResult {
            title,
            url: clean_ddg_url(&url),
            snippet,
        });
        cursor = title_end + 4;
    }
    results
}

pub(super) fn format_search_results(results: &[SearchResult]) -> String {
    let mut s = String::new();
    for (i, r) in results.iter().enumerate() {
        s.push_str(&format!("{}. {}\n", i + 1, r.title));
        s.push_str(&format!("   {}\n", r.url));
        if !r.snippet.is_empty() {
            s.push_str(&format!("   {}\n", r.snippet));
        }
        s.push('\n');
    }
    s
}

/// Pull a specific attribute value out of `<a href="..." class="...">`-shaped text.
pub(super) fn extract_attr(attrs: &str, name: &str) -> Option<String> {
    let needle = format!("{}=\"", name);
    let idx = attrs.find(&needle)?;
    let start = idx + needle.len();
    let end_offset = attrs[start..].find('"')?;
    Some(attrs[start..start + end_offset].to_string())
}

/// Strip HTML tags out of a text fragment ; replaces with spaces.
pub(super) fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// DuckDuckGo wraps result URLs in a redirector — e.g.
///   //duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpage
/// Pull the real URL out of the `uddg=` parameter when present ;
/// otherwise return the URL as-is (just normalizing leading //).
pub(super) fn clean_ddg_url(url: &str) -> String {
    let candidate = if let Some(idx) = url.find("uddg=") {
        let after = &url[idx + 5..];
        let raw = after.split('&').next().unwrap_or(after);
        percent_decode(raw)
    } else {
        url.to_string()
    };
    if let Some(rest) = candidate.strip_prefix("//") {
        format!("https://{}", rest)
    } else {
        candidate
    }
}

/// Minimal percent-decoder — enough for DDG's redirector params.
pub(super) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                hex_val(bytes[i + 1]),
                hex_val(bytes[i + 2]),
            ) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(super) fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Minimal URL-encode — covers space + common unsafe chars for a
/// query string. Sufficient for DuckDuckGo's `q=` parameter.
pub(super) fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

