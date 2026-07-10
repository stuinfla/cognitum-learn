//! Regression guard for GitHub issue #1: every CDN asset referenced by the
//! embedded UI pages must be pinned to an exact `X.Y.Z` version.
//!
//! On 2026-06-27 npm published @babel/standalone 8.0.3; the unpinned
//! `https://unpkg.com/@babel/standalone/babel.min.js` URL silently switched
//! majors and the JSX output became `import`-based, blanking `learn ui` for
//! every existing install with no binary change. Exact pins make CDN-side
//! releases inert.

static UI_HTML: &str = include_str!("../ui/index.html");
static UI_VISUAL_HTML: &str = include_str!("../ui/index-visual.html");

const CDN_HOSTS: [&str; 3] = ["unpkg.com", "jsdelivr.net", "cdn.tailwindcss.com"];

/// Extract every `src="…"` / `href="…"` attribute value that points at a
/// known CDN host.
fn cdn_urls(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for attr in ["src=\"", "href=\""] {
        let mut rest = html;
        while let Some(i) = rest.find(attr) {
            rest = &rest[i + attr.len()..];
            if let Some(end) = rest.find('"') {
                let url = &rest[..end];
                if CDN_HOSTS.iter().any(|h| url.contains(h)) {
                    urls.push(url.to_string());
                }
                rest = &rest[end..];
            }
        }
    }
    urls
}

/// True when `s` starts with an exact `X.Y.Z` semver (digits only), taking
/// the portion before any `/` path suffix.
fn is_exact_semver(s: &str) -> bool {
    let version = s.split('/').next().unwrap_or("");
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// True when `url` carries an exact version pin.
///
/// - Tailwind Play CDN pins as a path segment: `https://cdn.tailwindcss.com/3.4.16`
/// - unpkg / jsdelivr pin as `pkg@X.Y.Z`; scoped packages (`@babel/…`) have a
///   leading `@`, so the version sigil is always the LAST `@` in the URL.
fn exactly_pinned(url: &str) -> bool {
    if let Some(rest) = url.strip_prefix("https://cdn.tailwindcss.com") {
        return is_exact_semver(rest.trim_start_matches('/'));
    }
    match url.rfind('@') {
        Some(i) => is_exact_semver(&url[i + 1..]),
        None => false,
    }
}

#[test]
fn all_cdn_assets_are_exactly_pinned() {
    for (name, html) in [
        ("index.html", UI_HTML),
        ("index-visual.html", UI_VISUAL_HTML),
    ] {
        for url in cdn_urls(html) {
            assert!(
                exactly_pinned(&url),
                "{name}: CDN asset is not pinned to an exact X.Y.Z version: {url}\n\
                 Unpinned CDN deps break every deployed install when the CDN \
                 publishes a new major (see issue #1)."
            );
        }
    }
}

/// CDN `<script>`s from unpkg/jsdelivr must carry Subresource Integrity so a
/// compromised CDN cannot inject different bytes than the ones we verified.
///
/// The Tailwind Play CDN is the deliberate EXCEPTION: it serves no
/// `Access-Control-Allow-Origin` header, and SRI requires a `crossorigin`
/// fetch — adding integrity there makes the browser reject the script and
/// silently drops all Tailwind styling (verified live in Chrome). So the
/// tailwind tag must stay version-pinned but SRI-free.
#[test]
fn cdn_scripts_have_integrity_where_cors_allows() {
    for (name, html) in [
        ("index.html", UI_HTML),
        ("index-visual.html", UI_VISUAL_HTML),
    ] {
        for tag in html.split('<').filter(|t| t.starts_with("script ")) {
            let head = tag.split('>').next().unwrap_or(tag);
            if tag.contains("unpkg.com") || tag.contains("jsdelivr.net") {
                assert!(
                    tag.contains("integrity=\"sha384-"),
                    "{name}: CDN script tag lacks an integrity hash: <{head}"
                );
                assert!(
                    tag.contains("crossorigin"),
                    "{name}: CDN script tag with integrity needs crossorigin: <{head}"
                );
            } else if tag.contains("cdn.tailwindcss.com") {
                assert!(
                    !tag.contains("integrity=") && !tag.contains("crossorigin"),
                    "{name}: tailwind Play CDN must NOT carry integrity/crossorigin — \
                     it serves no CORS headers, so the browser blocks the script: <{head}"
                );
            }
        }
    }
}

/// Guard the guard: if the dashboard ever stops using CDN assets entirely
/// (e.g. precompiled bundle), this test should be revisited rather than
/// silently matching nothing.
#[test]
fn dashboard_still_references_cdn_assets() {
    assert!(
        !cdn_urls(UI_HTML).is_empty(),
        "expected index.html to reference CDN assets; if the UI moved to a \
         precompiled bundle, update or remove cdn_pinning.rs"
    );
}

#[test]
fn exactly_pinned_accepts_and_rejects_correctly() {
    // Pinned forms
    assert!(exactly_pinned(
        "https://unpkg.com/@babel/standalone@7.29.7/babel.min.js"
    ));
    assert!(exactly_pinned(
        "https://unpkg.com/react@18.3.1/umd/react.development.js"
    ));
    assert!(exactly_pinned("https://cdn.tailwindcss.com/3.4.16"));
    // Unpinned / range forms
    assert!(!exactly_pinned(
        "https://unpkg.com/@babel/standalone/babel.min.js"
    ));
    assert!(!exactly_pinned(
        "https://unpkg.com/react@18/umd/react.development.js"
    ));
    assert!(!exactly_pinned(
        "https://cdn.jsdelivr.net/npm/marked@13/marked.min.js"
    ));
    assert!(!exactly_pinned("https://cdn.tailwindcss.com"));
}
