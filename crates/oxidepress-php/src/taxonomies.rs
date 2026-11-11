// Modified by Vabian124 on 2026-11-11 to let plugins register a taxonomy.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Taxonomies a PHP plugin may register.
//!
//! Built-in names such as `category` and `nav_menu` stay on their own screens.
//! A plugin taxonomy is a short token because `wp_term_taxonomy.taxonomy` is varchar(32).

use serde_json::Value;

use crate::posts::content_type_allowed;
use crate::worker_token;

const RESERVED: &[&str] = &[
    "category",
    "post_tag",
    "nav_menu",
    "link_category",
    "post_format",
];

/// One taxonomy reported by `register_taxonomy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredTaxonomy {
    pub name: String,
    pub label: String,
    pub singular: String,
    pub object_types: Vec<String>,
    pub public: bool,
    pub hierarchical: bool,
    pub show_ui: bool,
    pub show_in_menu: bool,
}

/// Whether a plugin may register this taxonomy name.
pub fn taxonomy_name_allowed(name: &str) -> bool {
    worker_token(name, 32) && !RESERVED.contains(&name)
}

pub fn taxonomy_from_value(value: &Value) -> Option<RegisteredTaxonomy> {
    let name = text(value, "name");
    if !taxonomy_name_allowed(&name) {
        return None;
    }
    let object_types = value
        .get("object_types")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|name| content_type_allowed(name))
                .take(8)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if object_types.is_empty() {
        return None;
    }
    let label = plain(&text(value, "label"), 80);
    let label = if label.is_empty() {
        name.clone()
    } else {
        label
    };
    let singular = plain(&text(value, "singular"), 80);
    let singular = if singular.is_empty() {
        label.clone()
    } else {
        singular
    };
    Some(RegisteredTaxonomy {
        name,
        label,
        singular,
        object_types,
        public: flag(value, "public"),
        hierarchical: flag(value, "hierarchical"),
        show_ui: flag(value, "show_ui"),
        show_in_menu: flag(value, "show_in_menu"),
    })
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn plain(raw: &str, max: usize) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_control() {
            continue;
        }
        out.push(ch);
        if out.len() >= max {
            break;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reserved_taxonomies_are_refused() {
        assert!(taxonomy_name_allowed("venue"));
        assert!(!taxonomy_name_allowed("category"));
        assert!(!taxonomy_name_allowed("nav_menu"));
        assert!(!taxonomy_name_allowed("post_format"));
        let venue = json!({
            "name": "venue",
            "label": "Venues",
            "singular": "Venue",
            "object_types": ["event"],
            "public": true,
            "hierarchical": true,
            "show_ui": true,
            "show_in_menu": true,
        });
        let parsed = taxonomy_from_value(&venue).unwrap();
        assert_eq!(parsed.name, "venue");
        assert_eq!(parsed.object_types, vec!["event"]);
        assert!(parsed.hierarchical);
        assert!(taxonomy_from_value(&json!({
            "name": "nav_menu",
            "object_types": ["post"],
        }))
        .is_none());
        assert!(taxonomy_from_value(&json!({
            "name": "venue",
            "object_types": ["revision"],
        }))
        .is_none());
    }
}
