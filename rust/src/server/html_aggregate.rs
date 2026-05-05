// [antibody-exempt: rust/src/server/html_aggregate.rs — kernel-floor
//  per-aggregate HTML page for the multi-domain server. Click an
//  aggregate in the left nav → URL `/domains/<Domain>/aggregates/<Agg>`
//  → this renderer emits a focused page for that one aggregate, filtered
//  out of the noise of the parent domain page. Same Trikaya-floor
//  justification as the rest of rust/src/server/.]

//! HTML aggregate page — focused detail view for a single aggregate
//!
//! Shows the aggregate's bluebook representation : header, attributes,
//! value objects, references, lifecycle (states + transitions), every
//! command as a runnable button-and-form section, queries, records.
//!
//! Usage:
//!   let page = generate_aggregate_page("BinBuddy", "Subscription", rt, runtimes);

use crate::runtime::Runtime;
use std::cell::RefCell;
use std::collections::HashMap;
use super::html_shared::{wrap_page, sidebar_links, display_name, module_icon, esc};

/// Generate the per-aggregate detail page. Returns 404-shaped placeholder
/// if the named aggregate isn't in the domain.
pub fn generate_aggregate_page(
    domain_name: &str,
    agg_name: &str,
    rt: &RefCell<Runtime>,
    runtimes: &HashMap<String, RefCell<Runtime>>,
) -> String {
    let domains: Vec<(String, usize)> = {
        let mut d: Vec<_> = runtimes.iter()
            .map(|(n, r)| (n.clone(), r.borrow().domain.aggregates.len()))
            .collect();
        d.sort_by(|a, b| a.0.cmp(&b.0));
        d
    };
    let rt = rt.borrow();
    let sidebar_aggregates: Vec<String> = rt.domain.aggregates.iter()
        .map(|a| a.name.clone())
        .collect();
    let sidebar = sidebar_links(&domains, Some(domain_name), &sidebar_aggregates, Some(agg_name));

    let agg = match rt.domain.aggregates.iter().find(|a| a.name == agg_name) {
        Some(a) => a,
        None => {
            let main = format!(
                r#"<div class="p-8 rounded-lg border border-dashed border-surface-4 text-center">
  <p class="text-lg text-gray-400">Aggregate <code class="text-brand">{}</code> not found in domain <code class="text-brand">{}</code>.</p>
</div>"#,
                esc(agg_name), esc(domain_name),
            );
            return wrap_page(&display_name(agg_name), &sidebar, &main);
        }
    };

    let mut main = String::new();
    main.push_str(&header_section(domain_name, agg));
    main.push_str(&records_section(agg, &rt));
    // attributes_section + value_objects_section dropped — records table
    // shows attribute values ; VO fields surface inline in command forms.
    main.push_str(&references_section(agg));
    main.push_str(&lifecycle_section(agg));
    main.push_str(&commands_section(domain_name, agg, &rt));
    main.push_str(&queries_section(domain_name, agg));

    wrap_page(&display_name(&agg.name), &sidebar, &main)
}

/// Attribute names hidden from the records table — sensitive fields
/// (password hashes, tokens, secrets) and bookkeeping that adds noise.
/// The convention : if your attribute name contains any of these
/// substrings, it stays out of the user-visible table.
const HIDDEN_ATTRS: &[&str] = &[
    "password", "secret", "token", "api_key", "hash",
];

fn is_hidden(attr_name: &str) -> bool {
    let lower = attr_name.to_lowercase();
    HIDDEN_ATTRS.iter().any(|h| lower.contains(h))
}

/// Live records — every persisted instance of this aggregate, with one
/// column per declared attribute (sensitive fields hidden). Reads from
/// `Runtime::all` (in-memory + .heki-loaded).
fn records_section(agg: &crate::ir::Aggregate, rt: &Runtime) -> String {
    let records = rt.all(&agg.name);
    if records.is_empty() {
        return format!(
            r#"<div class="bg-surface-2 rounded-xl border border-surface-3 p-8 mb-6 text-center">
  <h2 class="text-sm font-bold uppercase tracking-wider text-gray-400 mb-3">Records</h2>
  <p class="text-sm text-gray-500">No <code class="text-brand">{}</code> records yet — run a command below to create one.</p>
</div>"#,
            esc(&display_name(&agg.name)),
        );
    }
    let visible: Vec<&crate::ir::Attribute> = agg.attributes.iter()
        .filter(|a| !is_hidden(&a.name))
        .collect();

    let mut s = format!(
        r#"<div class="bg-surface-2 rounded-xl border border-surface-3 p-5 mb-6">
  <h2 class="text-sm font-bold uppercase tracking-wider text-gray-400 mb-3">Records ({count})</h2>
  <div class="overflow-x-auto">
    <table class="w-full text-sm">
      <thead><tr class="text-xs text-gray-500 border-b border-surface-3">"#,
        count = records.len(),
    );
    for attr in &visible {
        s.push_str(&format!(
            r#"<th class="text-left py-2 pr-4">{}</th>"#,
            esc(&display_name(&attr.name)),
        ));
    }
    s.push_str("</tr></thead><tbody>");
    for state in records {
        s.push_str(r#"<tr class="border-b border-surface-3/50">"#);
        for attr in &visible {
            let display = format_value(state.get(&attr.name));
            s.push_str(&format!(
                r#"<td class="py-2 pr-4 text-gray-400">{}</td>"#,
                esc(&display),
            ));
        }
        s.push_str("</tr>");
    }
    s.push_str("</tbody></table></div></div>");
    s
}

/// Render one command-input element. Convention :
///   - `*_id` → datalist picker over the target aggregate's records
///   - everything else → plain input typed by the attribute's declared type
///
/// Auto-timestamps (`*_at`) are filtered out before this function runs ;
/// they render separately as hidden inputs.
fn render_command_input(
    attr: &crate::ir::Attribute,
    _agg: &crate::ir::Aggregate,
    rt: &Runtime,
) -> String {
    let placeholder = esc(&display_name(&attr.name));
    let input_type = match attr.attr_type.to_lowercase().as_str() {
        "float" | "integer" | "int" => "number",
        _ => "text",
    };
    let step = if attr.attr_type.to_lowercase() == "float" { r#" step="any""# } else { "" };

    // ID picker — attribute name ends in `_id` ; target aggregate is
    // the PascalCase of the prefix. If the runtime has it, render a
    // datalist of (id, label) pairs. Otherwise fall through to text.
    if let Some(target_name) = infer_id_target(&attr.name) {
        let records = rt.all(&target_name);
        if !records.is_empty() {
            let list_id = format!("picker-{}-{}", target_name, attr.name);
            let mut html = format!(
                r#"<div>
  <input list="{list_id}" name="{name}" type="text" placeholder="{placeholder}" class="bg-surface-0 border border-surface-4 rounded px-3 py-1.5 text-sm text-gray-100 focus:border-brand focus:outline-none w-full">
  <datalist id="{list_id}">"#,
                list_id = esc(&list_id),
                name = esc(&attr.name),
                placeholder = placeholder,
            );
            for state in records {
                let label = pick_label(state);
                if label.is_empty() || label == state.id {
                    html.push_str(&format!(
                        r#"<option value="{}">"#, esc(&state.id),
                    ));
                } else {
                    html.push_str(&format!(
                        r#"<option value="{}">{}</option>"#, esc(&state.id), esc(&label),
                    ));
                }
            }
            html.push_str("</datalist></div>");
            return html;
        }
    }

    // Plain input.
    format!(
        r#"<input name="{name}" type="{input_type}"{step} placeholder="{placeholder}" class="bg-surface-0 border border-surface-4 rounded px-3 py-1.5 text-sm text-gray-100 focus:border-brand focus:outline-none w-full">"#,
        name = esc(&attr.name),
        input_type = input_type,
        step = step,
        placeholder = placeholder,
    )
}

/// `service_address_id` → `ServiceAddress`. Returns None if the
/// attribute name doesn't end in `_id`.
fn infer_id_target(attr_name: &str) -> Option<String> {
    let stem = attr_name.strip_suffix("_id")?;
    if stem.is_empty() { return None; }
    // PascalCase the snake_case stem.
    let mut out = String::with_capacity(stem.len());
    let mut upper_next = true;
    for c in stem.chars() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            out.push(c.to_ascii_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    Some(out)
}

/// Best-guess display label for a record in a datalist option : prefer
/// `name`, then `email`, then any short String field, else fall back
/// to the id.
fn pick_label(state: &crate::runtime::AggregateState) -> String {
    use crate::runtime::Value;
    for key in &["name", "email", "code", "label", "title", "address_line_1", "first_name"] {
        if let Some(Value::Str(s)) = state.fields.get(*key) {
            if !s.is_empty() { return s.clone(); }
        }
    }
    state.id.clone()
}

/// Render a runtime Value as a short display string for table cells.
fn format_value(v: &crate::runtime::Value) -> String {
    use crate::runtime::Value;
    match v {
        Value::Str(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::List(items) => {
            let n = items.len();
            if n == 0 { "[]".into() } else { format!("[{} item{}]", n, if n == 1 { "" } else { "s" }) }
        }
        Value::Map(_) => "{…}".into(),
        Value::Null => "—".into(),
    }
}

fn header_section(domain_name: &str, agg: &crate::ir::Aggregate) -> String {
    let icon = module_icon(&agg.name);
    let label = esc(&display_name(&agg.name));
    let desc = agg.description.as_deref().unwrap_or("");
    format!(
        r#"<div class="mb-6">
  <p class="text-xs text-gray-500 mb-1">
    <a href="/domains/{domain}" class="hover:text-brand">{domain_label}</a>
    <span class="mx-1">›</span>
    <span>{label}</span>
  </p>
  <h1 class="text-3xl font-bold text-brand">{icon} {label}</h1>
  <p class="text-gray-400 mt-2">{desc}</p>
</div>"#,
        domain = esc(domain_name),
        domain_label = esc(&display_name(domain_name)),
        icon = icon,
        label = label,
        desc = esc(desc),
    )
}

fn attributes_section(agg: &crate::ir::Aggregate) -> String {
    if agg.attributes.is_empty() { return String::new(); }
    let mut s = String::from(
        r#"<div class="bg-surface-2 rounded-xl border border-surface-3 p-5 mb-6">
  <h2 class="text-sm font-bold uppercase tracking-wider text-gray-400 mb-3">Attributes</h2>
  <table class="w-full text-sm">
    <thead><tr class="text-xs text-gray-500 border-b border-surface-3"><th class="text-left py-2">Name</th><th class="text-left py-2">Type</th><th class="text-left py-2">Default</th></tr></thead>
    <tbody>"#,
    );
    for attr in &agg.attributes {
        let default = attr.default.as_deref().unwrap_or("—");
        let type_label = if attr.list {
            format!("[ {} ]", attr.attr_type)
        } else {
            attr.attr_type.clone()
        };
        s.push_str(&format!(
            r#"<tr class="border-b border-surface-3/50">
  <td class="py-2 text-white"><code>{name}</code></td>
  <td class="py-2 text-gray-400"><code>{type_label}</code></td>
  <td class="py-2 text-gray-500"><code>{default}</code></td>
</tr>"#,
            name = esc(&attr.name),
            type_label = esc(&type_label),
            default = esc(default),
        ));
    }
    s.push_str("</tbody></table></div>");
    s
}

fn value_objects_section(agg: &crate::ir::Aggregate) -> String {
    if agg.value_objects.is_empty() { return String::new(); }
    let mut s = String::from(
        r#"<div class="mb-6">
  <h2 class="text-sm font-bold uppercase tracking-wider text-gray-400 mb-3">Value Objects</h2>
  <div class="grid grid-cols-1 md:grid-cols-2 gap-3">"#,
    );
    for vo in &agg.value_objects {
        let desc = vo.description.as_deref().unwrap_or("");
        s.push_str(&format!(
            r#"<div class="bg-surface-2 rounded-lg border border-surface-3 p-4">
  <h3 class="font-semibold text-white">{name}</h3>
  <p class="text-xs text-gray-500 mt-1 mb-2">{desc}</p>
  <ul class="text-xs text-gray-400 space-y-1">"#,
            name = esc(&vo.name),
            desc = esc(desc),
        ));
        for attr in &vo.attributes {
            s.push_str(&format!(
                r#"<li><code class="text-gray-300">{}</code> <span class="text-gray-500">{}</span></li>"#,
                esc(&attr.name),
                esc(&attr.attr_type),
            ));
        }
        s.push_str("</ul></div>");
    }
    s.push_str("</div></div>");
    s
}

fn references_section(agg: &crate::ir::Aggregate) -> String {
    if agg.references.is_empty() { return String::new(); }
    let mut s = String::from(
        r#"<div class="bg-surface-2 rounded-xl border border-surface-3 p-5 mb-6">
  <h2 class="text-sm font-bold uppercase tracking-wider text-gray-400 mb-3">References</h2>
  <ul class="space-y-2">"#,
    );
    for r in &agg.references {
        s.push_str(&format!(
            r#"<li class="text-sm"><code class="text-gray-300">{name}</code> <span class="text-gray-500">→</span> <code class="text-brand">{target}</code></li>"#,
            name = esc(&r.name),
            target = esc(&r.target),
        ));
    }
    s.push_str("</ul></div>");
    s
}

fn lifecycle_section(agg: &crate::ir::Aggregate) -> String {
    let lifecycle = match &agg.lifecycle {
        Some(lc) => lc,
        None => return String::new(),
    };
    let mut s = String::from(
        r#"<div class="bg-surface-2 rounded-xl border border-surface-3 p-5 mb-6">
  <h2 class="text-sm font-bold uppercase tracking-wider text-gray-400 mb-3">Lifecycle</h2>"#,
    );
    s.push_str(&format!(
        r#"<p class="text-xs text-gray-500 mb-3">Field <code class="text-gray-300">{field}</code>, default <code class="text-emerald-400">{default}</code></p>"#,
        field = esc(&lifecycle.field),
        default = esc(&lifecycle.default),
    ));
    if !lifecycle.transitions.is_empty() {
        s.push_str(r#"<div class="space-y-1 text-sm">"#);
        for t in &lifecycle.transitions {
            let from = t.from_state.as_deref().unwrap_or("*");
            s.push_str(&format!(
                r#"<div class="flex gap-2 items-center"><code class="text-gray-500">{from}</code><span class="text-gray-600">—{cmd}—></span><code class="text-emerald-400">{to}</code></div>"#,
                from = esc(from),
                cmd = esc(&t.command),
                to = esc(&t.to_state),
            ));
        }
        s.push_str("</div>");
    }
    s.push_str("</div>");
    s
}

/// Each command as its own runnable section : button toggles a form
/// with all the command's attributes and dispatches via wizardSubmit.
///
/// Two attribute conventions surface here :
///
///   1. **Auto-timestamps** — any attribute name ending in `_at` (e.g.
///      `submitted_at`, `created_at`, `last_signed_in_at`) renders as
///      a hidden input with `data-auto="now"`. The wizardSubmit JS
///      fills these with `new Date().toISOString()` at submit time, so
///      the user never sees them. Keep timestamps out of the user's
///      attention.
///
///   2. **ID pickers** — any attribute name ending in `_id` renders as
///      an `<input list="...">` + inline `<datalist>` populated with
///      the existing instances of the target aggregate (PascalCase of
///      the prefix, e.g. `service_address_id` → ServiceAddress). This
///      gives HTML5-native search-as-you-type ; user picks from a
///      list rather than typing an opaque id.
fn commands_section(domain: &str, agg: &crate::ir::Aggregate, rt: &Runtime) -> String {
    if agg.commands.is_empty() { return String::new(); }
    let mut s = String::from(
        r#"<div class="mb-6">
  <div class="space-y-2">"#,
    );
    for cmd in &agg.commands {
        let label = display_name(&cmd.name);
        let role = cmd.role.as_deref().unwrap_or("System");
        let desc = cmd.description.as_deref().unwrap_or("");
        s.push_str(&format!(
            r##"<details class="bg-surface-2 rounded-lg border border-surface-3 overflow-hidden">
  <summary class="px-4 py-3 cursor-pointer hover:bg-surface-3/30 transition flex items-center justify-between">
    <div>
      <span class="font-semibold text-white">{label}</span>
      <span class="text-xs text-gray-500 ml-2">as {role}</span>
    </div>
    <span class="text-xs text-gray-600">▾</span>
  </summary>
  <div class="px-4 py-3 border-t border-surface-3 bg-surface-1/30">
    <p class="text-xs text-gray-500 mb-3">{desc}</p>
    <form onsubmit="return wizardSubmit(this, '{domain}', '{cmd_name}')" class="space-y-2">"##,
            label = esc(&label),
            role = esc(role),
            desc = esc(desc),
            domain = esc(domain),
            cmd_name = esc(&cmd.name),
        ));
        // Visible attrs : not auto-timestamps. Hidden timestamps still
        // render so the form carries them, but inside the form, not in
        // the visible grid.
        let visible_attrs: Vec<&crate::ir::Attribute> = cmd.attributes.iter()
            .filter(|a| !a.name.ends_with("_at"))
            .collect();
        if visible_attrs.is_empty() {
            s.push_str(r#"<p class="text-xs text-gray-500 italic">No inputs.</p>"#);
        } else {
            let cols = if visible_attrs.len() <= 3 { "grid-cols-1" } else { "grid-cols-2" };
            s.push_str(&format!(r#"<div class="grid {} gap-2">"#, cols));
            for attr in &visible_attrs {
                s.push_str(&render_command_input(attr, agg, rt));
            }
            s.push_str("</div>");
        }
        // Hidden auto-timestamp inputs — wizardSubmit JS fills them
        // with `new Date().toISOString()` before POSTing.
        for attr in cmd.attributes.iter().filter(|a| a.name.ends_with("_at")) {
            s.push_str(&format!(
                r#"<input type="hidden" name="{name}" data-auto="now">"#,
                name = esc(&attr.name),
            ));
        }
        s.push_str(&format!(
            r#"<button type="submit" class="w-full px-4 py-2 bg-brand/10 border border-brand/30 rounded-lg text-brand text-sm font-medium hover:bg-brand/20 transition mt-2">{label}</button>
    <div class="wizard-result mt-2"></div>
  </form>
  </div>
</details>"#,
            label = esc(&label),
        ));
    }
    s.push_str("</div></div>");
    s
}

/// Each query as its own runnable section : button toggles a form
/// with the query's input attributes ; submit POSTs to /domains/<X>/query
/// and renders the result as a small table.
fn queries_section(domain: &str, agg: &crate::ir::Aggregate) -> String {
    if agg.queries.is_empty() { return String::new(); }
    let mut s = String::from(
        r#"<div class="mb-6">
  <div class="space-y-2">"#,
    );
    for q in &agg.queries {
        let label = display_name(&q.name);
        let desc = q.description.as_deref().unwrap_or("");
        s.push_str(&format!(
            r##"<details class="bg-surface-2 rounded-lg border border-surface-3 overflow-hidden">
  <summary class="px-4 py-3 cursor-pointer hover:bg-surface-3/30 flex items-center justify-between">
    <div>
      <span class="font-semibold text-white">{label}</span>
      <span class="text-xs text-gray-500 ml-2">query</span>
    </div>
    <span class="text-xs text-gray-600">▾</span>
  </summary>
  <div class="px-4 py-3 border-t border-surface-3 bg-surface-1/30">
    <p class="text-xs text-gray-500 mb-3">{desc}</p>
    <form onsubmit="return wizardQuery(this, '{domain}', '{qname}')" class="space-y-2">"##,
            label = esc(&label),
            desc = esc(desc),
            domain = esc(domain),
            qname = esc(&q.name),
        ));
        if q.attributes.is_empty() {
            s.push_str(r#"<p class="text-xs text-gray-500 italic">No inputs.</p>"#);
        } else {
            let cols = if q.attributes.len() <= 3 { "grid-cols-1" } else { "grid-cols-2" };
            s.push_str(&format!(r#"<div class="grid {} gap-2">"#, cols));
            for attr in &q.attributes {
                let placeholder = esc(&display_name(&attr.name));
                let input_type = match attr.attr_type.to_lowercase().as_str() {
                    "float" | "integer" | "int" => "number",
                    _ => "text",
                };
                let step = if attr.attr_type.to_lowercase() == "float" { r#" step="any""# } else { "" };
                s.push_str(&format!(
                    r#"<input name="{name}" type="{input_type}"{step} placeholder="{placeholder}" class="bg-surface-0 border border-surface-4 rounded px-3 py-1.5 text-sm text-gray-100 focus:border-brand focus:outline-none w-full">"#,
                    name = esc(&attr.name),
                    input_type = input_type,
                    step = step,
                    placeholder = placeholder,
                ));
            }
            s.push_str("</div>");
        }
        s.push_str(&format!(
            r#"<button type="submit" class="w-full px-4 py-2 bg-brand/10 border border-brand/30 rounded-lg text-brand text-sm font-medium hover:bg-brand/20 transition mt-2">{label}</button>
    <div class="wizard-result mt-2"></div>
  </form>
  </div>
</details>"#,
            label = esc(&label),
        ));
    }
    s.push_str("</div></div>");
    s
}
