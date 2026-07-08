//! Link discovery — extract and absolutize `<a href>` from a fetched page.
//!
//! Like extraction, this runs in the page via JS (the driver's `evaluate`)
//! rather than re-parsing HTML in Rust. We then resolve every discovered href
//! against the page's final URL and keep only http(s) absolute links.

use serde_json::Value;
use url::Url;

/// JS snippet that returns every anchor href on the current page as a JSON
/// array of strings. Run via the driver's `evaluate`.
pub const ANCHOR_JS: &str = "(function(){\
     return Array.prototype.slice.call(document.querySelectorAll('a[href]'))\
       .map(function(a){ return a.href; })\
       .filter(function(h){ return !!h; });\
   })()";

/// Turn the raw JS result into a list of absolute, http(s)-only URLs, resolved
/// against the page we found them on. Drops mailto/javascript/fragments.
pub fn absolutize(base_url: &str, raw: Value) -> Vec<String> {
    let Some(base) = Url::parse(base_url).ok() else {
        return Vec::new();
    };
    let hrefs: Vec<String> = match raw {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        Value::String(s) => vec![s],
        _ => return Vec::new(),
    };

    let mut out = Vec::new();
    for href in hrefs {
        let Ok(mut abs) = base.join(&href) else {
            continue;
        };
        if !matches!(abs.scheme(), "http" | "https") {
            continue;
        }
        // Strip the fragment before dedup/comparison so "page#sec" and "page"
        // are treated as the same target.
        abs.set_fragment(None);
        // Drop self-referential links (the page we found this link on).
        if abs.as_str() == base_url.trim_end_matches('#') || abs.as_str() == base_url {
            continue;
        }
        out.push(abs.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_relative_and_drops_nonhttp() {
        let raw = json!([
            "/about",
            "https://other.example/x",
            "mailto:a@b.com",
            "#top"
        ]);
        let out = absolutize("https://example.com/page", raw);
        assert_eq!(
            out,
            vec!["https://example.com/about", "https://other.example/x"]
        );
    }

    #[test]
    fn drops_self_fragment() {
        let raw = json!(["https://example.com/page#sec"]);
        let out = absolutize("https://example.com/page", raw);
        assert!(out.is_empty());
    }

    #[test]
    fn handles_single_string() {
        let raw = json!("https://x.example/y");
        let out = absolutize("https://example.com/", raw);
        assert_eq!(out, vec!["https://x.example/y"]);
    }
}
