// [antibody-exempt: rust/src/server/html_sidebar.rs — kernel-floor HTML
//  sidebar renderer for the multi-domain server. Same Trikaya-floor
//  justification as the rest of rust/src/server/. Edits for the i241
//  primary-bluebook walk : aggregates of the active domain rendered
//  as indented sub-links pointing at /domains/<Name>/aggregates/<Agg>.
//  active_aggregate parameter highlights the current aggregate.]

//! Sidebar navigation — grouped domain links and utility helpers
//!
//! Generates the sidebar HTML with domains grouped under category
//! headers (Operations, Products, Sales, Compliance). When the user
//! is on a specific domain page, that domain's aggregates render as
//! indented sub-links beneath the active entry — the "walk into the
//! domain to get the aggregates" UX from bin-pal / bin-buddy. Each
//! sub-link points at `/domains/<Domain>/aggregates/<AggName>` ; the
//! page renderer emits a focused per-aggregate view at that URL.
//!
//! Usage:
//!   let links = sidebar_links(&domains, Some("BinBuddy"), &agg_names, Some("Subscription"));

/// Generate sidebar nav links. `aggregates` lists the active domain's
/// aggregates ; pass an empty slice on the index page or when the
/// active domain has none. `active_aggregate` highlights the current
/// aggregate when on a per-aggregate page.
pub fn sidebar_links(
    domains: &[(String, usize)],
    active: Option<&str>,
    aggregates: &[String],
    active_aggregate: Option<&str>,
) -> String {
    let groups: &[(&str, &[&str])] = &[
        ("Operations", &["manufacturing", "inventory", "supply_chain", "distribution", "quality"]),
        ("Products", &["catalog", "formulation", "formulation_lab", "pricing"]),
        ("Sales", &["brand_strategy", "customer_personas", "storefront"]),
        ("Compliance", &["compliance", "regulatory_compliance", "claims", "demand"]),
    ];
    let mut out = String::new();
    let mut placed = std::collections::HashSet::new();
    for (header, members) in groups {
        let group_domains: Vec<_> = members.iter()
            .filter_map(|m| domains.iter().find(|(n, _)| n == m))
            .collect();
        if group_domains.is_empty() { continue; }
        out.push_str(&format!(
            r#"<div class="mt-4 mb-1 px-3 text-xs font-bold uppercase tracking-wider text-gray-600">{}</div>"#,
            header
        ));
        for (name, _count) in &group_domains {
            placed.insert(name.clone());
            out.push_str(&sidebar_link(name, active));
            if active == Some(name) {
                out.push_str(&aggregate_sublinks(name, aggregates, active_aggregate));
            }
        }
    }
    // Ungrouped domains
    let ungrouped: Vec<_> = domains.iter().filter(|(n, _)| !placed.contains(n)).collect();
    if !ungrouped.is_empty() && !placed.is_empty() {
        out.push_str(r#"<div class="mt-4 mb-1 px-3 text-xs font-bold uppercase tracking-wider text-gray-600">Other</div>"#);
    }
    for (name, _) in ungrouped {
        out.push_str(&sidebar_link(name, active));
        if active == Some(name) {
            out.push_str(&aggregate_sublinks(name, aggregates, active_aggregate));
        }
    }
    out
}

/// Render aggregates under the active domain as indented links pointing
/// to `/domains/<Domain>/aggregates/<AggName>`. The active aggregate
/// (when set) is highlighted with the brand accent.
fn aggregate_sublinks(
    domain: &str,
    aggregates: &[String],
    active: Option<&str>,
) -> String {
    if aggregates.is_empty() { return String::new(); }
    let mut s = String::new();
    s.push_str(r#"<div class="ml-3 mt-1 mb-1 border-l border-surface-3 pl-2">"#);
    for name in aggregates {
        let label = super::html_shared::display_name(name);
        let icon = super::html_shared::module_icon(name);
        let active_class = if active == Some(name.as_str()) {
            "bg-brand/10 text-brand font-medium"
        } else {
            "text-gray-500 hover:bg-surface-2 hover:text-gray-200"
        };
        s.push_str(&format!(
            r#"<a href="/domains/{domain}/aggregates/{name}" class="block px-2 py-1 rounded text-xs {active_class} transition cursor-pointer" title="Open {label}">
  {icon} {label}
</a>"#,
        ));
    }
    s.push_str("</div>");
    s
}

fn sidebar_link(name: &str, active: Option<&str>) -> String {
    let active_class = if active == Some(name) {
        "bg-brand/15 text-brand border-l-2 border-brand font-semibold"
    } else {
        "text-gray-400 hover:bg-surface-2 hover:text-white"
    };
    let icon = super::html_shared::domain_icon(name);
    let label = super::html_shared::display_name(name);
    format!(
        r#"<a href="/domains/{name}" data-domain-aggregate="{name}" title="Open {label}" class="block px-3 py-1.5 rounded-lg text-sm {active_class} transition cursor-pointer">
  {icon} {label}
</a>"#,
    )
}
