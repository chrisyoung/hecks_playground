// [antibody-exempt: rust/src/server/html_login.rs — kernel-floor sign-in
//  page + post-auth dashboard for the multi-domain server. The HTML is
//  intentionally minimal : email + password, brand-amethyst accent, no
//  framework chrome on the login surface so the sign-in feels like its
//  own moment. Same Trikaya-floor justification as the rest of
//  rust/src/server/.]

//! HTML login + dashboard for the auth walking-skeleton.
//!
//! `generate_login_page` renders the unauthenticated home — sign-in form
//! posting to /sessions. `generate_dashboard` renders the post-auth home
//! showing the available domain (one for bin-buddy) plus a sign-out link.
//!
//! Brand : Embryonaut amethyst #5B3A8E. Cream surface,
//! understated typography, no animation.

use crate::runtime::Runtime;
use std::cell::RefCell;
use std::collections::HashMap;
use super::html_shared::{display_name, esc};

/// First-run bootstrap form. Rendered when zero Account records exist
/// — creates the owner account. POSTs to /sessions/bootstrap which
/// dispatches Account.SignUp with role=owner, sets the cookie, and
/// redirects to /. After this lands, the route is closed.
pub fn generate_bootstrap_page(flash: Option<&str>) -> String {
    let flash_html = flash
        .map(|m| format!(
            r#"<div class="rounded-lg bg-red-900/30 border border-red-800/40 text-red-300 text-sm px-4 py-3 mb-4">{}</div>"#,
            esc(m),
        ))
        .unwrap_or_default();

    format!(
        r#"<!DOCTYPE html>
<html lang="en" class="h-full">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Set up Bin-Buddy</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link href="https://fonts.googleapis.com/css2?family=Roboto+Slab:wght@500;700&family=Cabin:wght@400;500;700&display=swap" rel="stylesheet">
  <style>
    body, h1, h2 {{ font-family: 'Cabin', sans-serif; }}
    .brand-display {{ font-family: 'Roboto Slab', serif; letter-spacing: -0.01em; }}
    body {{ background-color: #faf8f4; color: #1a1a1a; }}
    .accent {{ color: #5B3A8E; }}
    .accent-bg {{ background-color: #5B3A8E; }}
    .accent-bg:hover {{ background-color: #4d2f78; }}
    .card {{ background: #fff; border: 1px solid #e6e1d7; }}
    .input {{ background: #fff; border: 1px solid #d8d3c7; }}
    .input:focus {{ border-color: #5B3A8E; outline: 2px solid rgba(91,58,142,0.15); outline-offset: 0; }}
  </style>
</head>
<body class="h-full flex items-center justify-center px-4">
  <main class="w-full max-w-sm">
    <div class="text-center mb-8">
      <h1 class="brand-display text-4xl font-bold accent">Bin-Buddy</h1>
      <p class="text-sm text-gray-500 mt-1">First run — create the owner account.</p>
    </div>
    <div class="card rounded-xl p-6 shadow-sm">
      {flash_html}
      <form method="POST" action="/sessions/bootstrap" class="space-y-3">
        <div>
          <label class="block text-xs font-medium text-gray-700 mb-1" for="email">Email</label>
          <input class="input rounded-md w-full px-3 py-2 text-sm" id="email" type="email" name="email" autocomplete="username" autofocus required>
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-700 mb-1" for="password">Password</label>
          <input class="input rounded-md w-full px-3 py-2 text-sm" id="password" type="password" name="password" autocomplete="new-password" minlength="6" required>
          <p class="text-xs text-gray-500 mt-1">You're the owner — this is the account you'll sign in with from now on.</p>
        </div>
        <button class="accent-bg text-white rounded-md w-full py-2 text-sm font-medium transition" type="submit">Create owner account</button>
      </form>
    </div>
    <p class="text-xs text-gray-500 text-center mt-4">Embryonaut · Bin-Buddy walking-skeleton</p>
  </main>
</body>
</html>"#,
        flash_html = flash_html,
    )
}

/// Customer self-signup form. Email + password + name + phone. POSTs
/// to /signup which dispatches Account.SignUp (role=customer) and
/// Customer.RegisterCustomer back-to-back, then sets the cookie and
/// lands the user on /. Driver and admin signup are restricted paths
/// (separate flows ; not this one).
pub fn generate_signup_page(flash: Option<&str>) -> String {
    let flash_html = flash
        .map(|m| format!(
            r#"<div class="rounded-lg bg-red-50 border border-red-200 text-red-700 text-sm px-4 py-3 mb-4">{}</div>"#,
            esc(m),
        ))
        .unwrap_or_default();

    format!(
        r#"<!DOCTYPE html>
<html lang="en" class="h-full">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Sign up — Bin-Buddy</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link href="https://fonts.googleapis.com/css2?family=Roboto+Slab:wght@500;700&family=Cabin:wght@400;500;700&display=swap" rel="stylesheet">
  <style>
    body, h1, h2 {{ font-family: 'Cabin', sans-serif; }}
    .brand-display {{ font-family: 'Roboto Slab', serif; letter-spacing: -0.01em; }}
    body {{ background-color: #faf8f4; color: #1a1a1a; }}
    .accent {{ color: #5B3A8E; }}
    .accent-bg {{ background-color: #5B3A8E; }}
    .accent-bg:hover {{ background-color: #4d2f78; }}
    .card {{ background: #fff; border: 1px solid #e6e1d7; }}
    .input {{ background: #fff; border: 1px solid #d8d3c7; }}
    .input:focus {{ border-color: #5B3A8E; outline: 2px solid rgba(91,58,142,0.15); outline-offset: 0; }}
  </style>
</head>
<body class="h-full flex items-center justify-center px-4 py-8">
  <main class="w-full max-w-md">
    <div class="text-center mb-8">
      <h1 class="brand-display text-4xl font-bold accent">Bin-Buddy</h1>
      <p class="text-sm text-gray-500 mt-1">Sign up — let us take it from here.</p>
    </div>
    <div class="card rounded-xl p-6 shadow-sm">
      {flash_html}
      <form method="POST" action="/signup" class="space-y-3">
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-xs font-medium text-gray-700 mb-1" for="first_name">First name</label>
            <input class="input rounded-md w-full px-3 py-2 text-sm" id="first_name" type="text" name="first_name" autocomplete="given-name" autofocus required>
          </div>
          <div>
            <label class="block text-xs font-medium text-gray-700 mb-1" for="last_name">Last name</label>
            <input class="input rounded-md w-full px-3 py-2 text-sm" id="last_name" type="text" name="last_name" autocomplete="family-name" required>
          </div>
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-700 mb-1" for="email">Email</label>
          <input class="input rounded-md w-full px-3 py-2 text-sm" id="email" type="email" name="email" autocomplete="email" required>
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-700 mb-1" for="phone">Phone</label>
          <input class="input rounded-md w-full px-3 py-2 text-sm" id="phone" type="tel" name="phone" autocomplete="tel">
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-700 mb-1" for="password">Password</label>
          <input class="input rounded-md w-full px-3 py-2 text-sm" id="password" type="password" name="password" autocomplete="new-password" minlength="6" required>
        </div>
        <button class="accent-bg text-white rounded-md w-full py-2 text-sm font-medium transition" type="submit">Sign up</button>
      </form>
    </div>
    <p class="text-xs text-gray-500 text-center mt-4">
      Already a customer? <a href="/" class="accent hover:underline">Sign in</a>
    </p>
  </main>
</body>
</html>"#,
        flash_html = flash_html,
    )
}

/// Sign-in form. `flash` is an optional error message to render
/// above the form (e.g. "Email or password didn't match").
pub fn generate_login_page(flash: Option<&str>) -> String {
    let flash_html = flash
        .map(|m| format!(
            r#"<div class="rounded-lg bg-red-50 border border-red-200 text-red-700 text-sm px-4 py-3 mb-4">{}</div>"#,
            esc(m),
        ))
        .unwrap_or_default();

    format!(
        r#"<!DOCTYPE html>
<html lang="en" class="h-full">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Sign in</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link href="https://fonts.googleapis.com/css2?family=Roboto+Slab:wght@500;700&family=Cabin:wght@400;500;700&display=swap" rel="stylesheet">
  <style>
    body, h1, h2 {{ font-family: 'Cabin', sans-serif; }}
    .brand-display {{ font-family: 'Roboto Slab', serif; letter-spacing: -0.01em; }}
    body {{ background-color: #faf8f4; color: #1a1a1a; }}
    .accent {{ color: #5B3A8E; }}
    .accent-bg {{ background-color: #5B3A8E; }}
    .accent-bg:hover {{ background-color: #4d2f78; }}
    .card {{ background: #fff; border: 1px solid #e6e1d7; }}
    .input {{ background: #fff; border: 1px solid #d8d3c7; }}
    .input:focus {{ border-color: #5B3A8E; outline: 2px solid rgba(91,58,142,0.15); outline-offset: 0; }}
  </style>
</head>
<body class="h-full flex items-center justify-center px-4">
  <main class="w-full max-w-sm">
    <div class="text-center mb-8">
      <h1 class="brand-display text-4xl font-bold accent">Bin-Buddy</h1>
      <p class="text-sm text-gray-500 mt-1">Trash in, on time. Always.</p>
    </div>
    <div class="card rounded-xl p-6 shadow-sm">
      {flash_html}
      <form method="POST" action="/sessions" class="space-y-3">
        <div>
          <label class="block text-xs font-medium text-gray-700 mb-1" for="email">Email</label>
          <input class="input rounded-md w-full px-3 py-2 text-sm" id="email" type="email" name="email" autocomplete="username" autofocus required>
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-700 mb-1" for="password">Password</label>
          <input class="input rounded-md w-full px-3 py-2 text-sm" id="password" type="password" name="password" autocomplete="current-password" required>
        </div>
        <button class="accent-bg text-white rounded-md w-full py-2 text-sm font-medium transition" type="submit">Sign in</button>
      </form>
    </div>
    <p class="text-xs text-gray-500 text-center mt-4">
      New customer? <a href="/signup" class="accent hover:underline">Sign up</a>
    </p>
  </main>
</body>
</html>"#,
        flash_html = flash_html,
    )
}

/// Post-auth landing : a tight summary of what the signed-in user
/// can reach. For tonight's walking-skeleton the dashboard is just a
/// list of loaded domains + sign-out.
pub fn generate_dashboard(
    email: &str,
    runtimes: &HashMap<String, RefCell<Runtime>>,
) -> String {
    let mut domain_cards = String::new();
    let mut names: Vec<&String> = runtimes.keys().collect();
    names.sort();
    for name in names {
        let rt = runtimes[name].borrow();
        let agg_count = rt.domain.aggregates.len();
        let cmd_count: usize = rt.domain.aggregates.iter().map(|a| a.commands.len()).sum();
        let vision = rt.domain.vision.as_deref().unwrap_or("");
        domain_cards.push_str(&format!(
            r#"<a href="/domains/{name}" class="card rounded-xl p-5 block hover:shadow-md transition">
  <h3 class="brand-display text-xl font-semibold accent">{label}</h3>
  <p class="text-sm text-gray-600 mt-1">{vision}</p>
  <p class="text-xs text-gray-500 mt-3">{agg_count} aggregates · {cmd_count} commands</p>
</a>"#,
            name = esc(name),
            label = esc(&display_name(name)),
            vision = esc(vision),
            agg_count = agg_count,
            cmd_count = cmd_count,
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en" class="h-full">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Bin-Buddy</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link href="https://fonts.googleapis.com/css2?family=Roboto+Slab:wght@500;700&family=Cabin:wght@400;500;700&display=swap" rel="stylesheet">
  <style>
    body, h1, h2, h3 {{ font-family: 'Cabin', sans-serif; }}
    .brand-display {{ font-family: 'Roboto Slab', serif; letter-spacing: -0.01em; }}
    body {{ background-color: #faf8f4; color: #1a1a1a; }}
    .accent {{ color: #5B3A8E; }}
    .accent-bg {{ background-color: #5B3A8E; }}
    .card {{ background: #fff; border: 1px solid #e6e1d7; }}
  </style>
</head>
<body class="h-full">
  <header class="border-b border-gray-200 bg-white">
    <div class="max-w-5xl mx-auto px-6 py-4 flex items-center justify-between">
      <h1 class="brand-display text-2xl font-bold accent">Bin-Buddy</h1>
      <div class="text-xs text-gray-500 flex items-center gap-3">
        <span>{email}</span>
        <a href="/sign-out" class="text-gray-500 hover:accent transition">Sign out</a>
      </div>
    </div>
  </header>
  <main class="max-w-5xl mx-auto px-6 py-10">
    <h2 class="text-lg font-semibold mb-4">Your domains</h2>
    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      {domain_cards}
    </div>
  </main>
</body>
</html>"#,
        email = esc(email),
        domain_cards = domain_cards,
    )
}
