//! Extraction — turn a fetched page into structured records.
//!
//! Rather than re-parse HTML in Rust (the page is JS-rendered; a static parser
//! would fight the DOM shirabe already built), extraction is expressed as a
//! declarative **schema** of CSS-selector → field mappings, compiled to a
//! JavaScript snippet and executed inside the page via the driver's `evaluate`.
//! This leans on shirabe's strongest capability and keeps the crawler free of
//! any HTML parsing dependency.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How to pull a single field out of a matched element.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FieldSource {
    /// `element.textContent`
    Text,
    /// A specific attribute, e.g. `href`, `src`.
    Attr { name: String },
    /// `element.innerHTML`
    Html,
}

/// One column of an extraction schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSpec {
    pub name: String,
    pub selector: String,
    #[serde(flatten)]
    pub source: FieldSource,
}

/// A full extraction schema: select container rows, then map fields per row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionSchema {
    /// CSS selector for the repeating container (one per record). If omitted,
    /// the whole document is treated as a single record and each field's
    /// selector is queried against the document.
    pub container: Option<String>,
    pub fields: Vec<FieldSpec>,
}

impl ExtractionSchema {
    /// Compile this schema into a JS expression that, when evaluated in a page,
    /// returns an array of plain JS objects (`[{name: value, ...}]`).
    ///
    /// The generated snippet uses only standard DOM APIs so it runs under any
    /// browser backend the driver picks.
    pub fn to_js(&self) -> String {
        let container = self
            .container
            .as_deref()
            .map(|c| format!("document.querySelectorAll({})", js_str(c)))
            .unwrap_or_else(|| "[document]".to_string());

        let field_collectors: Vec<String> = self
            .fields
            .iter()
            .map(|f| {
                let getter = match &f.source {
                    FieldSource::Text => "(e||{}).textContent || ''".to_string(),
                    FieldSource::Html => "(e||{}).innerHTML || ''".to_string(),
                    FieldSource::Attr { name } => {
                        format!("(e||{{}}).getAttribute({}) || ''", js_str(name))
                    }
                };
                format!(
                    "name: {js}, value: (function(root){{\
                     var el = (root||document).querySelector({sel});\
                     return {getter}.trim();\
                     }})(this)",
                    js = js_str(&f.name),
                    sel = js_str(&f.selector),
                    getter = getter,
                )
            })
            .collect();

        let body = if self.container.is_some() {
            format!(
                "Array.prototype.map.call({root}, function(el){{\
                 return {{ {cols} }}.value !== undefined ? {{ {cols} }} : null;\
                 }}).filter(function(x){{ return x !== null; }})",
                root = container,
                cols = field_collectors.join(", "),
            )
        } else {
            format!("[ {{ {cols} }} ]", cols = field_collectors.join(", "))
        };

        format!("(function(){{ return {body}; }})()")
    }
}

/// A single extracted record (an arbitrary JSON object).
pub type Record = Value;

/// Extract records from rendered HTML using a JS schema evaluated via a
/// closure. Kept here as a pure type alias + helper so the worker can plug in
/// whatever evaluator the driver exposes.
pub fn parse_eval_result(raw: Value) -> Vec<Record> {
    match raw {
        Value::Array(items) => items.into_iter().filter(|v| !v.is_null()).collect(),
        single if !single.is_null() => vec![single],
        _ => Vec::new(),
    }
}

/// Quote a Rust string as a JavaScript single-quoted string literal with
/// backslash escaping. Kept dependency-free.
fn js_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn js_str_escapes_quotes() {
        assert_eq!(js_str("a'b"), "'a\\'b'");
        assert_eq!(js_str("a\\b"), "'a\\\\b'");
    }

    #[test]
    fn schema_with_container_compiles() {
        let schema = ExtractionSchema {
            container: Some("div.item".into()),
            fields: vec![
                FieldSpec {
                    name: "title".into(),
                    selector: "h2".into(),
                    source: FieldSource::Text,
                },
                FieldSpec {
                    name: "link".into(),
                    selector: "a".into(),
                    source: FieldSource::Attr {
                        name: "href".into(),
                    },
                },
            ],
        };
        let js = schema.to_js();
        assert!(js.contains("querySelectorAll('div.item')"));
        assert!(js.contains("getAttribute('href')"));
    }

    #[test]
    fn parse_eval_result_filters_nulls() {
        let raw = json!([{"title": "a"}, null, {"title": "b"}]);
        assert_eq!(parse_eval_result(raw).len(), 2);
    }

    #[test]
    fn parse_eval_result_single_object() {
        let raw = json!({"title": "solo"});
        assert_eq!(parse_eval_result(raw).len(), 1);
    }
}
